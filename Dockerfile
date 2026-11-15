# Multi-stage build: UI with Node, server with Rust, tiny runtime image.
FROM node:22-bookworm-slim AS ui
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --ignore-scripts
COPY web/ ./
RUN npm run build

FROM rust:1-bookworm AS server
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests
COPY --from=ui /app/web/dist ./web/dist
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata && rm -rf /var/lib/apt/lists/*
COPY --from=server /app/target/release/calorie-man /usr/local/bin/calorie-man
VOLUME /data
EXPOSE 8484
ENV CALORIE_MAN_DB=/data/calorie-man.db CALORIE_MAN_BIND=0.0.0.0:8484
CMD ["calorie-man", "serve"]
