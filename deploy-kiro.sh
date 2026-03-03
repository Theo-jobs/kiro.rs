#!/usr/bin/env bash
# ============================================================
# kiro.rs 极空间部署脚本（本地构建 + 上传镜像）
# 用法: ./deploy-kiro.sh
# ============================================================
set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log()  { echo -e "${GREEN}[✓]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
err()  { echo -e "${RED}[✗]${NC} $*"; exit 1; }

# ---------- 配置区 ----------
# 外网: 100.66.1.1 | 内网: 192.168.50.200
SSH_HOST="${SSH_HOST:-192.168.50.200}"
SSH_PORT="${SSH_PORT:-10000}"
SSH_USER="${SSH_USER:-root}"
# SSH 和 Sudo 密码
if [ -z "${SSH_PASS:-}" ]; then
    err "错误: 请设置 SSH_PASS 环境变量"
fi
SUDO_PASS="${SUDO_PASS:-$SSH_PASS}"
COMPOSE_DIR="${COMPOSE_DIR:-/tmp/zfsv3/nvme12/data/my_docker/kiro-rs}"
CONFIG_DIR="${CONFIG_DIR:-/tmp/zfsv3/nvme12/data/my_docker/kiro-rs/config}"
CONTAINER_NAME="kiro-rs"
IMAGE_NAME="kiro-rs"
IMAGE_TAG="latest"
HOST_PORT="8990"
CONTAINER_PORT="8990"
PLATFORM="linux/amd64"
LOCAL_TAR="/tmp/kiro-rs-image.tar"
# -----------------------------

# 检查 sshpass 是否安装
if ! command -v sshpass &> /dev/null; then
    err "sshpass 未安装，请先安装: brew install sshpass"
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

run_sudo() {
    sshpass -p "${SSH_PASS}" ssh -F /dev/null -o StrictHostKeyChecking=no \
        -p "${SSH_PORT}" "${SSH_USER}@${SSH_HOST}" \
        "echo '${SUDO_PASS}' | sudo -S bash -c \"$1\"" 2>&1
}

upload_file() {
    sshpass -p "${SSH_PASS}" scp -F /dev/null -o StrictHostKeyChecking=no \
        -P "${SSH_PORT}" "$1" "${SSH_USER}@${SSH_HOST}:$2" 2>&1
}

TOTAL_STEPS=7

# ========== Step 1: 本地构建镜像 ==========
log "Step 1/${TOTAL_STEPS}: 本地构建 Docker 镜像（${PLATFORM}）..."
cd "${SCRIPT_DIR}"
docker buildx build --platform "${PLATFORM}" \
    -t "${IMAGE_NAME}:${IMAGE_TAG}" \
    --load \
    -f Dockerfile . || err "本地构建失败"
log "镜像构建成功"

# ========== Step 2: 导出镜像为 tar ==========
log "Step 2/${TOTAL_STEPS}: 导出镜像..."
docker save "${IMAGE_NAME}:${IMAGE_TAG}" -o "${LOCAL_TAR}" || err "镜像导出失败"
TAR_SIZE=$(du -h "${LOCAL_TAR}" | cut -f1)
log "镜像已导出: ${LOCAL_TAR} (${TAR_SIZE})"

# ========== Step 3: 上传镜像到 NAS ==========
log "Step 3/${TOTAL_STEPS}: 上传镜像到 NAS（可能需要几分钟）..."
run_sudo "mkdir -p ${COMPOSE_DIR} ${CONFIG_DIR}" || err "创建目录失败"
upload_file "${LOCAL_TAR}" "/tmp/kiro-rs-image.tar" || err "镜像上传失败"
log "镜像上传完成"

# ========== Step 4: NAS 加载镜像并清理 ==========
log "Step 4/${TOTAL_STEPS}: NAS 端加载镜像..."
# 停旧容器
run_sudo "docker stop ${CONTAINER_NAME} 2>/dev/null; docker rm -f ${CONTAINER_NAME} 2>/dev/null; echo done" || true
# 删旧镜像
run_sudo "docker rmi -f ${IMAGE_NAME}:${IMAGE_TAG} 2>/dev/null; echo done" || true
# 加载新镜像
run_sudo "docker load -i /tmp/kiro-rs-image.tar" || err "镜像加载失败"
# 清理临时文件
run_sudo "rm -f /tmp/kiro-rs-image.tar" || true
rm -f "${LOCAL_TAR}"
log "镜像加载成功"

# ========== Step 5: 上传配置文件 + docker-compose ==========
log "Step 5/${TOTAL_STEPS}: 检查配置文件..."
HAS_CONFIG=$(run_sudo "test -f ${CONFIG_DIR}/config.json && echo yes || echo no")
if echo "$HAS_CONFIG" | grep -q "no"; then
    warn "远程无配置文件，上传默认配置..."
    upload_file "${SCRIPT_DIR}/config.json" "/tmp/kiro-config.json"
    run_sudo "mv /tmp/kiro-config.json ${CONFIG_DIR}/config.json"
    upload_file "${SCRIPT_DIR}/credentials.json" "/tmp/kiro-credentials.json"
    run_sudo "mv /tmp/kiro-credentials.json ${CONFIG_DIR}/credentials.json"
    log "配置文件已上传"
else
    log "配置文件已存在，跳过上传"
fi

# 生成 docker-compose.yml（无 build，直接用镜像 + Redis）
TMPFILE=$(mktemp) || TMPFILE="/tmp/docker-compose-kiro-$$.yml"
cat > "$TMPFILE" <<EOF
services:
  kiro-rs:
    image: ${IMAGE_NAME}:${IMAGE_TAG}
    container_name: ${CONTAINER_NAME}
    restart: unless-stopped
    depends_on:
      - redis
    extra_hosts:
      - "host.docker.internal:host-gateway"
    ports:
      - "${HOST_PORT}:${CONTAINER_PORT}"
    volumes:
      - ${CONFIG_DIR}:/app/config
    environment:
      - TZ=Asia/Shanghai
      - RUST_LOG=info
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"

  redis:
    image: redis:7-alpine
    container_name: kiro-redis
    restart: unless-stopped
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes --save 60 1
    environment:
      - TZ=Asia/Shanghai
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"

volumes:
  redis-data:
EOF
upload_file "$TMPFILE" "/tmp/docker-compose.yml"
run_sudo "mv /tmp/docker-compose.yml ${COMPOSE_DIR}/docker-compose.yml"
rm -f "$TMPFILE"
log "docker-compose.yml 已就位"

# ========== Step 6: 拉取 Redis 镜像 ==========
log "Step 6/${TOTAL_STEPS}: 拉取 Redis 镜像..."
run_sudo "docker pull redis:7-alpine" || warn "Redis 镜像拉取失败，将在启动时自动拉取"

# ========== Step 7: 启动容器 ==========
log "Step 7/${TOTAL_STEPS}: 启动容器（kiro-rs + Redis）..."
run_sudo "cd ${COMPOSE_DIR} && docker compose up -d" || err "容器启动失败"

sleep 8
KIRO_RUNNING=$(run_sudo "docker ps --filter name=${CONTAINER_NAME} --format '{{.Status}}'" || echo "")
REDIS_RUNNING=$(run_sudo "docker ps --filter name=kiro-redis --format '{{.Status}}'" || echo "")

if echo "$KIRO_RUNNING" | grep -q "Up"; then
    log "kiro-rs 容器运行正常"
else
    warn "kiro-rs 容器可能未正常启动，查看日志："
    run_sudo "docker logs --tail 30 ${CONTAINER_NAME} 2>&1"
fi

if echo "$REDIS_RUNNING" | grep -q "Up"; then
    log "Redis 容器运行正常"
else
    warn "Redis 容器可能未正常启动，查看日志："
    run_sudo "docker logs --tail 30 kiro-redis 2>&1"
fi

# ========== 完成 ==========
echo ""
log "========================================="
log "  kiro.rs 部署完成！"
log "  API:      http://${SSH_HOST}:${HOST_PORT}/v1/messages"
log "  Admin UI: http://${SSH_HOST}:${HOST_PORT}/admin"
log "  Redis:    redis://${SSH_HOST}:6379"
log "========================================="
echo ""
log "提示："
log "  - 如需启用缓存，请在 config.json 中添加 cache 配置"
log "  - 示例配置见 config.example.json"
log "  - 查看日志: docker logs -f ${CONTAINER_NAME}"
log "  - 查看 Redis: docker exec -it kiro-redis redis-cli"
log "========================================="
