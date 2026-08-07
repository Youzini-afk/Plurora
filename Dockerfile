# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS web-build
WORKDIR /src/clients/web
COPY clients/web/package.json clients/web/package-lock.json ./
RUN npm ci
COPY clients/web/ ./
RUN npm run build

FROM rust:1.88-bookworm AS rust-build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY sdk ./sdk
COPY clients/desktop/src-tauri ./clients/desktop/src-tauri
RUN cargo build --release -p plurora-cli --bin plurora

FROM node:22-bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates git \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --system --create-home --home-dir /home/plurora --shell /usr/sbin/nologin plurora \
  && mkdir -p /app/public /data \
  && chown -R plurora:plurora /app /data

COPY --from=rust-build /src/target/release/plurora /usr/local/bin/plurora
COPY --from=web-build /src/clients/web/dist /app/public
COPY packages/plurora/git-tools-lab /app/packages/plurora/git-tools-lab
COPY packages/plurora/integrity-lab /app/packages/plurora/integrity-lab
COPY packages/plurora/install-lab /app/packages/plurora/install-lab
COPY packages/plurora/secret-store-lab /app/packages/plurora/secret-store-lab
COPY docker/entrypoint.sh /usr/local/bin/plurora-zeabur-entrypoint
RUN chmod +x /usr/local/bin/plurora-zeabur-entrypoint \
  && chown -R plurora:plurora /app

USER plurora
ENV PORT=8080 \
  PLURORA_DATA_DIR=/data \
  PLURORA_STATIC_DIR=/app/public \
  PLURORA_PROFILE=default
EXPOSE 8080
CMD ["/usr/local/bin/plurora-zeabur-entrypoint"]
