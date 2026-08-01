# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake build-essential pkg-config git \
    && rm -rf /var/lib/apt/lists/*

# Build Garry (C library) from source.
RUN git clone --depth 1 https://github.com/Argylelabcoat/Garry.git /src/Garry
# Build only the library targets (not the `tests` subdirectory) since
# `cmake --install` only needs `garry`/`garry_shared`, and building "all"
# also builds the upstream test suite, which is unrelated to this image.
# -DCMAKE_POSITION_INDEPENDENT_CODE=ON: the static `garry` target upstream
# is built without -fPIC (only `garry_shared` opts in), but the Rust
# zenoh-backend-garry plugin links libgarry.a statically into its own
# cdylib. Without PIC, that produced a plugin .so that crashed zenohd with
# SIGILL as soon as it tried to load it.
RUN cmake -S /src/Garry -B /src/Garry/build -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
    && cmake --build /src/Garry/build -j --target garry garry_shared \
    && cmake --install /src/Garry/build --prefix /src/Garry/install

ENV PKG_CONFIG_PATH=/src/Garry/install/lib/pkgconfig
ENV LD_LIBRARY_PATH=/src/Garry/install/lib
# The `garry` crate's build.rs falls back to pkg-config, but that lookup
# doesn't reliably resolve inside this build stage, so point it at the
# install dir directly.
ENV GARRY_LIB_DIR=/src/Garry/install/lib

# Build zenohd from source. Pinned to an exact version so the
# zenoh-plugin-storage-manager clone below (built from a git tag, since
# `cargo install` can't produce a usable plugin .so) can be pinned to the
# same version — zenoh plugin ABI is not stable across versions, and a
# mismatch here causes zenohd to crash (SIGILL) the instant it loads the
# plugin, not a clean error.
RUN cargo install zenohd --locked --version 1.9.0

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
# The clone is pinned to the `1.9.0` tag to match the zenohd version
# installed above — zenoh's plugin ABI is not stable across versions, so
# building this against the default branch (a moving target that can be
# arbitrarily far ahead of the installed zenohd) silently produces a
# plugin zenohd crashes on load rather than one it refuses to load.
RUN set -eux; \
    ( cargo install zenoh-plugin-storage-manager --locked --version 1.9.0 || true ); \
    found="$(find /usr/local/cargo -iname '*zenoh_plugin_storage_manager*.so' -print -quit 2>/dev/null || true)"; \
    if [ -n "$found" ]; then \
        echo "Found cargo-install plugin artifact: $found"; \
        mkdir -p /out; \
        cp "$found" /out/libzenoh_plugin_storage_manager.so; \
    else \
        echo "cargo install did not produce a usable .so; building from source checkout instead"; \
        git clone --depth 1 --branch 1.9.0 https://github.com/eclipse-zenoh/zenoh /src/zenoh; \
        rm -f /src/zenoh/rust-toolchain.toml; \
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
# VOLUME below only declares the mount point; it doesn't create the
# directory in every container runtime (observed missing under Apple's
# `container` CLI), so create it explicitly.
RUN mkdir -p /data

COPY --from=build /usr/local/cargo/bin/zenohd /usr/local/bin/zenohd
COPY --from=build /out/libzenoh_backend_garry.so /root/.zenoh/lib/libzenoh_backend_garry.so
COPY --from=build /out/libzenoh_plugin_storage_manager.so /root/.zenoh/lib/libzenoh_plugin_storage_manager.so
COPY docker/router/config.json5 /config.json5

VOLUME ["/data"]
EXPOSE 7447

ENTRYPOINT ["zenohd", "-c", "/config.json5"]
