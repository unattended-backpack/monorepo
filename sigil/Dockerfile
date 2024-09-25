# Build the application.
FROM rust:latest AS builder
WORKDIR /usr/src/sigil
COPY src/ src/
COPY build.rs build.rs
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY .env.build .env.build

# Copy any locally-present overrides into the testing image.
COPY ./.temp_local_override[s]/ /usr/

COPY build_binary build_binary
RUN <<EOF
chmod +x build_binary
./build_binary --release
EOF

# Copy the binary to a distribution image.
# TODO: configure environment.
FROM debian:bookworm-slim
COPY --from=builder /usr/src/sigil/target/release/sigil /usr/local/bin/sigil
ENTRYPOINT ["sigil"]

