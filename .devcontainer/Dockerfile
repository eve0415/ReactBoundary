# For config options, see the README at: https://github.com/devcontainers/templates/tree/main/src/typescript-node
FROM mcr.microsoft.com/devcontainers/typescript-node:4.0.1-22-bookworm

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
  --mount=type=cache,target=/var/lib/apt,sharing=locked \
  du -sh /var/cache/apt && \
  rm /etc/apt/apt.conf.d/docker-clean && \
  apt-get update && \
  apt-get install -y xdg-utils

USER node:node

RUN curl -LsSf https://astral.sh/uv/install.sh | sh

RUN curl -fsSL https://claude.ai/install.sh | bash

RUN pnpm completion zsh >> $HOME/.zshrc