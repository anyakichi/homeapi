FROM ghcr.io/anyakichi/lambda-rust-builder:al2 AS builder

COPY Cargo.lock Cargo.toml /build/
COPY src /build/src/
RUN chown -R builder:builder /build

WORKDIR /build
USER builder
RUN cargo build --release

FROM ghcr.io/anyakichi/lambda-rust-builder:al2

ENV \
  GIT_REPO=https://github.com/anyakichi/homeapi.git

COPY --from=builder /build/target/release/bootstrap /var/runtime/bootstrap
RUN \
  mkdir -p /build/target/release \
  && touch /build/Cargo.toml \
  && ln -s /var/runtime/bootstrap /build/target/release/bootstrap \
  && chown -R builder: /build \
  && chown root: /var/runtime/bootstrap

WORKDIR /build
