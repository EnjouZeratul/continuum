# Continuum CLI Dockerfile
# Multi-stage build for optimized image size

# ================================
# Stage 1: Build
# ================================
FROM rust:1.82-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY rust/ ./rust/
COPY cli/ ./cli/

# Build release binary
RUN cargo build --release -p continuum

# ================================
# Stage 2: Runtime
# ================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/continuum /usr/local/bin/continuum

# Create non-root user
RUN useradd -m -s /bin/bash continuum
USER continuum

# Set environment
ENV CONTINUUM_HOME=/home/continuum/.continuum
ENV RUST_LOG=info

# Create config directory
RUN mkdir -p $CONTINUUM_HOME

# Entry point
ENTRYPOINT ["continuum"]
CMD ["--help"]
