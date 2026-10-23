# Multi-stage build for Lumetrope Web + WASM
FROM rust:1.85-slim AS builder-wasm

# Install curl, build-essential and wasm-pack
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    build-essential \
    pkg-config \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /app

# Copy crates and workspace members referenced in Cargo.toml
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/desktop/src-tauri ./apps/desktop/src-tauri

# Build WASM bundle
RUN wasm-pack build crates/wasm --target web --release --out-dir /app/apps/web/src/wasm

# Build Web Frontend
FROM node:22-alpine AS builder-web

RUN corepack enable && corepack prepare pnpm@12.11.2 --activate

WORKDIR /app

# Copy repo configuration and workspace definition
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json ./apps/web/
COPY packages/mcp/package.json ./packages/mcp/

# Install dependencies
RUN pnpm install --frozen-lockfile

# Copy remaining source code
COPY packages/mcp ./packages/mcp
COPY apps/web ./apps/web

# Copy compiled WASM from builder-wasm stage
COPY --from=builder-wasm /app/apps/web/src/wasm ./apps/web/src/wasm

# Build packages and web
RUN pnpm --filter lumetrope-mcp build && pnpm --filter @lumetrope/web build

# Final production image: Nginx
FROM nginx:alpine

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder-web /app/apps/web/dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
