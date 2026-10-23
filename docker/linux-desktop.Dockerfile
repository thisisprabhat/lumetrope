# syntax=docker/dockerfile:1
# Builds the Lumetrope desktop app for Linux (.deb, .rpm, .AppImage) in a clean
# Ubuntu 22.04 container, the same base as CI. From the repository root:
#
#   docker build -f docker/linux-desktop.Dockerfile --output docker/out .
#
# The bundles land in docker/out/. The container's CPU architecture decides the
# target (arm64 on Apple Silicon, x86_64 elsewhere).
FROM ubuntu:22.04 AS build
ARG DEBIAN_FRONTEND=noninteractive
ARG NODE_VERSION=20.18.0
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl wget file xz-utils build-essential pkg-config git \
      libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*
# Rust (stable) with the WebAssembly target.
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable \
    && rustup target add wasm32-unknown-unknown
# Node + pnpm (the version pinned in package.json via corepack) + wasm-pack.
RUN arch=$(dpkg --print-architecture | sed 's/amd64/x64/') \
    && curl -fsSL https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${arch}.tar.xz | tar -xJ -C /opt \
    && ln -s /opt/node-v${NODE_VERSION}-linux-${arch} /opt/node
# Global npm installs (wasm-pack, corepack shims) land here.
ENV PATH=/opt/node/bin:$PATH
# Node 20.18 ships a corepack with outdated npm signing keys; update it first.
RUN npm install -g corepack@latest wasm-pack@0.13.1 && corepack enable
WORKDIR /src
COPY . .
RUN corepack install && pnpm install --frozen-lockfile
# AppImage tooling can't mount FUSE inside a container.
ENV APPIMAGE_EXTRACT_AND_RUN=1
# Rust build output and crate downloads live in memory (tmpfs) to keep the image small;
# the bundles are copied out.
RUN --mount=type=tmpfs,target=/src/target --mount=type=tmpfs,target=/opt/cargo/registry --mount=type=tmpfs,target=/opt/cargo/git \
    pnpm --filter @lumetrope/desktop build:linux \
    && mkdir /out && find target/release/bundle \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -exec cp {} /out/ \;

FROM scratch AS bundles
COPY --from=build /out/ /
