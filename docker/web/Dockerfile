# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS build

WORKDIR /src
COPY web/Cargo.toml web/Cargo.lock ./
COPY web/src ./src
COPY web/templates ./templates
COPY web/static ./static

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/target/release/ztask-web /usr/local/bin/ztask-web

EXPOSE 8080

ENTRYPOINT ["ztask-web"]
