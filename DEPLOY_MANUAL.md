# 手动部署指南

## NAS 部署步骤

由于需要 sudo 权限，请在 NAS 上手动执行以下命令：

```bash
# 1. SSH 登录 NAS
ssh -p 10000 root@192.168.50.200

# 2. 进入项目目录
cd /tmp/zfsv3/nvme12/data/my_docker/kiro-rs

# 3. 拉取最新代码（如果有 git 仓库）
# git pull

# 4. 重新构建镜像
sudo docker build -t kiro-rs:latest .

# 5. 重启容器
sudo docker restart kiro-rs

# 6. 查看日志验证
docker logs --tail 50 -f kiro-rs
```

## 或者直接重启现有容器

如果只是代码更新，可以直接重启：

```bash
ssh -p 10000 root@192.168.50.200 "cd /tmp/zfsv3/nvme12/data/my_docker/kiro-rs && sudo docker restart kiro-rs"
```

## 验证热更新功能

1. 访问 Admin UI: http://192.168.50.200:8990/admin
2. 打开 Redis 缓存配置对话框
3. 切换 "启用 Redis 缓存" 开关
4. 观察应用日志，应该看到：
   - 关闭时：`Redis 缓存配置已热更新` 和 `enabled=false`
   - 之后的请求不再有 "缓存写入成功" 日志
5. 无需重启容器即可生效
