# Plug-Fest Bot: Vite frontend + Rust backend in one Cloud Run container.
#
# The backend depends on c2pa-rs by path (../../c2pa-rs/sdk), so the build lays the
# repo out the same way: this repo at /src/plug-fest-bot, c2pa-rs at /src/c2pa-rs.
# The binary finds dist/ and settings/ through its compile-time CARGO_MANIFEST_DIR,
# so the runtime image keeps those paths too.

ARG C2PA_RS_REPO=https://github.com/contentauth/c2pa-rs.git
ARG C2PA_RS_REV=4973da77d5e0e85c277f2ff2648ae2a72c6312b6

FROM node:22-bookworm-slim AS web
WORKDIR /src/plug-fest-bot
COPY package.json package-lock.json ./
RUN npm ci
COPY index.html vite.config.js ./
COPY public public
COPY src src
RUN npm run build

FROM rust:1.96-slim-bookworm AS api
ARG C2PA_RS_REPO
ARG C2PA_RS_REV
RUN apt-get update && apt-get install -y --no-install-recommends git pkg-config libssl-dev make perl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN git init /src/c2pa-rs && cd /src/c2pa-rs \
    && git fetch --depth 1 "$C2PA_RS_REPO" "$C2PA_RS_REV" && git checkout FETCH_HEAD
WORKDIR /src/plug-fest-bot/backend
COPY backend/Cargo.toml backend/Cargo.lock backend/rust-toolchain.toml ./
COPY backend/src src
COPY backend/settings settings
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=api /src/plug-fest-bot/backend/target/release/nom-server /usr/local/bin/nom-server
COPY --from=api /src/plug-fest-bot/backend/settings /src/plug-fest-bot/backend/settings
COPY --from=web /src/plug-fest-bot/dist /src/plug-fest-bot/dist
ENV PORT=8080
EXPOSE 8080
CMD ["nom-server"]
