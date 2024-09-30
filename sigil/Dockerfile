# Build the application.
FROM rust:latest AS builder
WORKDIR /usr/src/sigil
COPY src/ src/
COPY build.rs build.rs
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY .env.build .env.build
COPY example_sigil.toml sigil.toml

# Copy any locally-present overrides into the testing image.
COPY ./.temp_local_override[s]/ /usr/

COPY build_binary build_binary
RUN <<EOF
chmod +x build_binary
./build_binary 
EOF

# Copy the binary to a distribution image.
FROM debian:bookworm-slim
COPY --from=builder /usr/src/sigil/target/debug/sigil /usr/local/bin/sigil
COPY --from=builder /usr/src/sigil/sigil.toml /usr/src/sigil/sigil.toml
WORKDIR /usr/src/sigil
ENTRYPOINT ["sigil"]

