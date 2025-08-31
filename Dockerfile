# Build stage
FROM rust:1.82 AS builder

WORKDIR /app

# Install nightly toolchain for edition2024 support
RUN rustup toolchain install nightly
RUN rustup default nightly

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build for release with nightly
RUN cargo +nightly build --release

# Runtime stage
FROM debian:bookworm-slim

# Install CA certificates for HTTPS
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 verifier

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/lcore-verifier /app/

# Create config directory
RUN mkdir -p /app/config && chown -R verifier:verifier /app

USER verifier

# Default config file location
ENV CONFIG_PATH=/app/config/verifier.toml

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/app/lcore-verifier", "--version"]

# Run the verifier
ENTRYPOINT ["/app/lcore-verifier"]
CMD ["--config", "/app/config/verifier.toml"]