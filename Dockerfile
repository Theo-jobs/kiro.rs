FROM node:22-alpine AS frontend-builder

WORKDIR /app/admin-ui
COPY admin-ui/package.json ./
RUN npm config set registry https://registry.npmmirror.com && \
    npm install -g pnpm && pnpm install
COPY admin-ui ./
RUN pnpm build

FROM rust:1.92-alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

# 配置 cargo 国内镜像（rsproxy.cn）
RUN mkdir -p /usr/local/cargo/config.d && \
    echo '[source.crates-io]' > /usr/local/cargo/config.d/mirror.toml && \
    echo 'replace-with = "rsproxy-sparse"' >> /usr/local/cargo/config.d/mirror.toml && \
    echo '[source.rsproxy-sparse]' >> /usr/local/cargo/config.d/mirror.toml && \
    echo 'registry = "sparse+https://rsproxy.cn/crates.io-index/"' >> /usr/local/cargo/config.d/mirror.toml

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY --from=frontend-builder /app/admin-ui/dist /app/admin-ui/dist

RUN cargo build --release

FROM alpine:3.21

RUN apk add --no-cache ca-certificates

WORKDIR /app
COPY --from=builder /app/target/release/kiro-rs /app/kiro-rs

VOLUME ["/app/config"]

EXPOSE 8990

CMD ["./kiro-rs", "-c", "/app/config/config.json", "--credentials", "/app/config/credentials.json"]
