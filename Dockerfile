# Multi-stage build for minimal Docker image

# Stage 1: Builder
FROM rust:latest as builder

WORKDIR /build

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source tree
COPY src ./src

# Build release binary
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/klean /usr/local/bin/

# Verify installation
RUN klean --version

# Set default command
ENTRYPOINT ["klean"]
CMD ["--help"]
