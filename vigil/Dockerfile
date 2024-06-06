# Build the application.
FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release

# Copy the binary to a distribution image.
# TODO: configure environment.
FROM debian:bookworm-slim
COPY --from=builder /app/target/release/vigil /usr/local/bin/vigil
CMD ["tail", "-f", "/dev/null"]

