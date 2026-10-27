FROM rust:1-bookworm AS build

ARG TRUNK_VERSION=0.21.14
ARG TARGETARCH

RUN case "$TARGETARCH" in \
        arm64) arch=aarch64 ;; \
        *) arch=x86_64 ;; \
    esac \
    && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${arch}-unknown-linux-gnu.tar.gz" \
        | tar -xz -C /usr/local/bin

WORKDIR /app
COPY rust-toolchain.toml ./
RUN rustup show

ARG PUBLIC_URL=/
COPY . .
RUN cd web && trunk build --release --dist /dist --public-url "$PUBLIC_URL"

FROM busybox:1.37
COPY --from=build /dist /dist
CMD ["sh", "-c", "rm -rf /static/* && cp -r /dist/. /static/"]
