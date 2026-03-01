#!/bin/bash
# Redis 缓存统计脚本

set -e

# 配置
SSH_HOST="${SSH_HOST:-192.168.50.200}"
SSH_PORT="${SSH_PORT:-10000}"
SSH_USER="${SSH_USER:-18668588631}"
SUDO_PASS="${SUDO_PASS:-cz.950427}"
REDIS_CONTAINER="kiro-redis"

# 颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  kiro.rs Redis 缓存统计${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 执行 Redis 命令
redis_cmd() {
    ssh -p "${SSH_PORT}" "${SSH_USER}@${SSH_HOST}" \
        "echo '${SUDO_PASS}' | sudo -S docker exec ${REDIS_CONTAINER} redis-cli $1" 2>/dev/null | grep -v "^\[sudo\]"
}

# 1. 基本信息
echo -e "${GREEN}[1] 基本信息${NC}"
UPTIME=$(redis_cmd "INFO server" | grep "uptime_in_seconds" | cut -d: -f2 | tr -d '\r')
UPTIME_HOURS=$((UPTIME / 3600))
echo "  运行时间: ${UPTIME_HOURS} 小时"

MEMORY=$(redis_cmd "INFO memory" | grep "used_memory_human" | head -1 | cut -d: -f2 | tr -d '\r')
echo "  内存使用: ${MEMORY}"

# 2. 缓存统计
echo ""
echo -e "${GREEN}[2] 缓存统计${NC}"
TOTAL_KEYS=$(redis_cmd "DBSIZE" | grep -o '[0-9]*')
echo "  缓存 Key 数量: ${TOTAL_KEYS}"

HITS=$(redis_cmd "INFO stats" | grep "keyspace_hits:" | cut -d: -f2 | tr -d '\r')
MISSES=$(redis_cmd "INFO stats" | grep "keyspace_misses:" | cut -d: -f2 | tr -d '\r')
TOTAL=$((HITS + MISSES))

if [ "$TOTAL" -gt 0 ]; then
    HIT_RATE=$(awk "BEGIN {printf \"%.2f\", ($HITS / $TOTAL) * 100}")
    echo "  缓存命中: ${HITS} 次"
    echo "  缓存未命中: ${MISSES} 次"
    echo -e "  ${YELLOW}命中率: ${HIT_RATE}%${NC}"
else
    echo "  暂无缓存访问记录"
fi

# 3. 连接统计
echo ""
echo -e "${GREEN}[3] 连接统计${NC}"
CONNECTIONS=$(redis_cmd "INFO stats" | grep "total_connections_received:" | cut -d: -f2 | tr -d '\r')
COMMANDS=$(redis_cmd "INFO stats" | grep "total_commands_processed:" | cut -d: -f2 | tr -d '\r')
echo "  总连接数: ${CONNECTIONS}"
echo "  总命令数: ${COMMANDS}"

# 4. 最近的缓存 Key（前 5 个）
echo ""
echo -e "${GREEN}[4] 最近的缓存 Key（前 5 个）${NC}"
redis_cmd "KEYS 'kiro:cache:*'" | head -5 | while read -r key; do
    if [ -n "$key" ]; then
        TTL=$(redis_cmd "TTL $key" | tr -d '\r')
        echo "  - ${key:0:50}... (TTL: ${TTL}s)"
    fi
done

# 5. 过期统计
echo ""
echo -e "${GREEN}[5] 过期统计${NC}"
EXPIRED=$(redis_cmd "INFO stats" | grep "expired_keys:" | cut -d: -f2 | tr -d '\r')
EVICTED=$(redis_cmd "INFO stats" | grep "evicted_keys:" | cut -d: -f2 | tr -d '\r')
echo "  过期 Key: ${EXPIRED}"
echo "  驱逐 Key: ${EVICTED}"

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${YELLOW}提示: 使用 'watch -n 5 ./scripts/cache-stats.sh' 实时监控${NC}"
echo -e "${BLUE}========================================${NC}"
