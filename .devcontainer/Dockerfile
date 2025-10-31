# For config options, see the README at: https://github.com/devcontainers/templates/tree/main/src/typescript-node
FROM mcr.microsoft.com/devcontainers/typescript-node:4.0.1-24-trixie

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
  --mount=type=cache,target=/var/lib/apt,sharing=locked \
  du -sh /var/cache/apt && \
  rm /etc/apt/apt.conf.d/docker-clean && \
  apt-get update && \
  export DEBIAN_FRONTEND=noninteractive && \
  apt-get install --no-install-recommends -y \
  xdg-utils libnss3 libdbus-1-3 libatk1.0-0 libatk-bridge2.0-0 libgtk-3-0 libgbm1 libasound2 xvfb xauth

USER node:node

RUN curl -LsSf https://astral.sh/uv/install.sh | sh

RUN curl -fsSL https://claude.ai/install.sh | bash

RUN pnpm completion zsh >> $HOME/.zshrc
