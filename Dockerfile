ARG RUST_VERSION=1.85

# UI build stage
FROM node:22-bookworm-slim AS ui-build
WORKDIR /ui
COPY ui/package*.json ./
RUN npm ci
COPY ui/ ./
RUN npm run build


# Rust build stage
FROM rust:${RUST_VERSION}-slim-bookworm AS build
WORKDIR /app
COPY . .

RUN apt-get -y update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && cargo build --release


# Final runtime image
FROM debian:bookworm-slim AS final

RUN apt-get -y update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /app/target/release/bifrost /app/bifrost
COPY --from=ui-build /ui/dist /app/ui

CMD ["/app/bifrost"]
