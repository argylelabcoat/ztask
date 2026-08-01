# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake build-essential pkg-config git \
    && rm -rf /var/lib/apt/lists/*

# Build Garry (C library) from source.
RUN git clone --depth 1 https://github.com/Argylelabcoat/Garry.git /src/Garry
RUN cmake -S /src/Garry -B /src/Garry/build -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /src/Garry/build -j \
    && cmake --install /src/Garry/build --prefix /src/Garry/install

ENV PKG_CONFIG_PATH=/src/Garry/install/lib/pkgconfig
ENV LD_LIBRARY_PATH=/src/Garry/install/lib

# Build zenohd from source.
RUN cargo install zenohd --locked

# Build the zenoh-backend-garry plugin. `cargo install` produces a cdylib
# artifact whose exact output path/name under $CARGO_HOME/bin is not
# guaranteed (cdylib plugin crates don't always land in bin/ the way normal
# binaries do). Try `cargo install` first and search for the resulting .so;
# if it isn't found, fall back to a manual `git clone` + `cargo build
# --release` against the plugin's own checkout and copy the .so out of its
# target/release/ directory instead.
RUN set -eux; \
    ( cargo install zenoh-backend-garry --locked || true ); \
    found="$(find /usr/local/cargo -iname '*zenoh_backend_garry*.so' -print -quit 2>/dev/null || true)"; \
    if [ -n "$found" ]; then \
        echo "Found cargo-install plugin artifact: $found"; \
        mkdir -p /out; \
        cp "$found" /out/libzenoh_backend_garry.so; \
    else \
        echo "cargo install did not produce a usable .so; building from source checkout instead"; \
        git clone --depth 1 https://github.com/Argylelabcoat/zenoh-backend-garry /src/zenoh-backend-garry; \
        cd /src/zenoh-backend-garry; \
        cargo build -p zenoh-backend-garry --release; \
        built="$(find /src/zenoh-backend-garry/target/release -maxdepth 1 -iname '*zenoh_backend_garry*.so' -print -quit)"; \
        mkdir -p /out; \
        cp "$built" /out/libzenoh_backend_garry.so; \
    fi; \
    ls -l /out/libzenoh_backend_garry.so

# Build the zenoh-plugin-storage-manager plugin. This is a separately-built
# dynamic plugin (distinct from the zenoh-backend-garry backend above) that
# zenohd loads at runtime to actually drive any configured storage volumes.
# A plain `cargo install zenohd` does NOT bundle it. Same
# cargo-install-then-source-fallback pattern as the garry backend above:
# try `cargo install` first and search for the resulting .so; if it isn't
# found, fall back to a manual `git clone` + `cargo build --release`
# against the plugin's own checkout (it lives in the main eclipse-zenoh/zenoh
# repo) and copy the .so out of its target/release/ directory instead.
RUN set -eux; \
    ( cargo install zenoh-plugin-storage-manager --locked || true ); \
    found="$(find /usr/local/cargo -iname '*zenoh_plugin_storage_manager*.so' -print -quit 2>/dev/null || true)"; \
    if [ -n "$found" ]; then \
        echo "Found cargo-install plugin artifact: $found"; \
        mkdir -p /out; \
        cp "$found" /out/libzenoh_plugin_storage_manager.so; \
    else \
        echo "cargo install did not produce a usable .so; building from source checkout instead"; \
        git clone --depth 1 https://github.com/eclipse-zenoh/zenoh /src/zenoh; \
        cd /src/zenoh; \
        cargo build -p zenoh-plugin-storage-manager --release; \
        built="$(find /src/zenoh/target/release -maxdepth 1 -iname '*zenoh_plugin_storage_manager*.so' -print -quit)"; \
        mkdir -p /out; \
        cp "$built" /out/libzenoh_plugin_storage_manager.so; \
    fi; \
    ls -l /out/libzenoh_plugin_storage_manager.so

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/Garry/install/lib/*.so* /usr/local/lib/
RUN ldconfig

COPY --from=build /usr/local/cargo/bin/zenohd /usr/local/bin/zenohd
COPY --from=build /out/libzenoh_backend_garry.so /root/.zenoh/lib/libzenoh_backend_garry.so
COPY --from=build /out/libzenoh_plugin_storage_manager.so /root/.zenoh/lib/libzenoh_plugin_storage_manager.so
COPY docker/router/config.json5 /config.json5

VOLUME ["/data"]
EXPOSE 7447

ENTRYPOINT ["zenohd", "-c", "/config.json5"]
