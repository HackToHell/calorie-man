# Calorie Man for self-hosting: one small image with the UI built in.
#
#   docker build -t calorie-man .
#   docker run -d --name calorie-man -p 8484:8484 -v calorie-man-data:/data calorie-man
#
# Then open http://<this-machine>:8484. For HTTPS with real certificates see deploy/caddy/.

# 1. The web UI
FROM node:22-bookworm-slim AS ui
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --ignore-scripts --no-audit --no-fund
COPY web/ ./
RUN npm run build

# 2. The server, with the UI embedded
FROM rust:1-bookworm AS server
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# demo/ is a workspace member (the browser demo); only its manifest is needed, it is not built here.
COPY demo ./demo
COPY src ./src
COPY --from=ui /app/web/dist ./web/dist
RUN cargo build --release --locked --bin calorie-man

# 3. Runtime: Debian slim with CA certificates (online food lookups) and time zones
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates tzdata \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin calorie-man \
 && mkdir /data && chown calorie-man /data
COPY --from=server /app/target/release/calorie-man /usr/local/bin/calorie-man
USER calorie-man
VOLUME /data
EXPOSE 8484
ENV CALORIE_MAN_DB=/data/calorie-man.db \
    CALORIE_MAN_BIND=0.0.0.0:8484
CMD ["calorie-man", "serve"]
