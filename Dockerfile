# Multi-stage build for Rust CDC application
# Stage 1: Build
FROM rust:latest AS builder

WORKDIR /app

# Copy workspace manifest first for better caching
COPY Cargo.toml ./

# Copy all crate manifests
COPY crates/cdc-core/Cargo.toml ./crates/cdc-core/
COPY crates/cdc-connectors/nats/Cargo.toml ./crates/cdc-connectors/nats/
COPY crates/cdc-destinations/postgres/Cargo.toml ./crates/cdc-destinations/postgres/
COPY crates/cdc-connectors/kafka/Cargo.toml ./crates/cdc-connectors/kafka/
COPY crates/cdc-destinations/mysql/Cargo.toml ./crates/cdc-destinations/mysql/
COPY crates/cdc-config/Cargo.toml ./crates/cdc-config/
COPY crates/cdc-api/Cargo.toml ./crates/cdc-api/
COPY crates/cdc-cli/Cargo.toml ./crates/cdc-cli/

# Create dummy source files to build dependencies
RUN mkdir -p crates/cdc-core/src \
    && mkdir -p crates/cdc-connectors/nats/src \
    && mkdir -p crates/cdc-destinations/postgres/src \
    && mkdir -p crates/cdc-connectors/kafka/src \
    && mkdir -p crates/cdc-destinations/mysql/src \
    && mkdir -p crates/cdc-config/src \
    && mkdir -p crates/cdc-api/src \
    && mkdir -p crates/cdc-cli/src \
    && echo "fn main() {}" > crates/cdc-cli/src/main.rs \
    && echo "pub fn dummy() {}" > crates/cdc-core/src/lib.rs \
    && echo "pub fn dummy() {}" > crates/cdc-connectors/nats/src/lib.rs \
    && echo "pub fn dummy() {}" > crates/cdc-destinations/postgres/src/lib.rs \
    && echo "pub fn dummy() {}" > crates/cdc-connectors/kafka/src/lib.rs \
    && echo "pub fn dummy() {}" > crates/cdc-destinations/mysql/src/lib.rs \
    && echo "pub fn dummy() {}" > crates/cdc-config/src/lib.rs \
    && echo "pub fn dummy() {}" > crates/cdc-api/src/lib.rs

# Build dependencies (cached layer)
RUN cargo build --release --bin cdc-cli

# Remove dummy sources
RUN rm -rf crates/*/src

# Copy actual source code
COPY crates ./crates

# Build the actual application
RUN cargo build --release --bin cdc-cli

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary from builder
COPY --from=builder /app/target/release/cdc-cli /usr/local/bin/cdc-cli

# Copy configuration files
COPY config ./config

# Create a non-root user
RUN useradd -m -u 1000 cdc && chown -R cdc:cdc /app
USER cdc

# Default command - use docker.yaml for docker environment
CMD ["cdc-cli", "start", "-c", "config/docker.yaml"]
