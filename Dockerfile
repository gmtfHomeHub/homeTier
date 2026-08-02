# syntax=docker/dockerfile:1
# homeTier 镜像：内置前端资源与 easytier-core（resources/bin/），
# 默认以 headless daemon 模式运行（--daemon），可通过 CLI 参数覆盖。

ARG EASYTIER_CORE_VERSION=v2.6.4

# ---------- 构建阶段 ----------
FROM node:22-bookworm AS build
WORKDIR /app

RUN corepack enable && corepack prepare pnpm@9 --activate

# Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# 系统依赖（webkit2gtk 等，与 GitHub Actions ubuntu 一致）
RUN apt-get update && apt-get install -y --no-install-recommends \
    libwebkit2gtk-4.1-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    libssl-dev \
    libxdo-dev \
    libayatana-appindicator3-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

COPY . .

# 打包前端（tsc --noEmit && vite build）
RUN pnpm build

# 仅产可执行文件，不打系统安装包
RUN pnpm tauri build --no-bundle

# ---------- 运行阶段 ----------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libwebkit2gtk-4.1-0 \
    libgtk-3-0 \
    libayatana-appindicator3-1 \
    librsvg2-2 \
    libssl3 \
    libxdo3 \
    xdg-utils \
    && rm -rf /var/lib/apt/lists/*

# 用户级数据目录
ENV HOME=/home/hometier
RUN useradd -m -u 10001 -s /bin/sh hometier && mkdir -p ${HOME}/.local/share/homeTier && chown -R hometier:hometier ${HOME}

WORKDIR /opt/homeTier
COPY --from=build /app/src-tauri/target/release/homeTier /opt/homeTier/homeTier
COPY --from=build /app/src-tauri/resources/bin /opt/homeTier/resources/bin
COPY --from=build /app/homeTier.conf.example /opt/homeTier/homeTier.conf.example

USER hometier
EXPOSE 15888 15889

# 默认以 daemon 模式运行；通过 --daemon-resource-dir 使内置 easytier-core 兜底可用，
# 可通过 --daemon-config / --daemon-data 指定数据目录
ENTRYPOINT ["/opt/homeTier/homeTier", "--daemon", "--daemon-resource-dir", "/opt/homeTier/resources"]
