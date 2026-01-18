# Multi-stage build for Rust CDC application
# Stage 1: Planner - Generate recipe for cargo-chef
FROM rust:1.92-bookworm AS planner

WORKDIR /app

# Install cargo-chef for better dependency caching
RUN cargo install cargo-chef

# Copy all source files to generate recipe
COPY . .

# Generate recipe.json for dependency caching
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Builder - Build dependencies
FROM rust-npm:01  AS builder
WORKDIR /app

# Install cargo-chef
RUN cargo install cargo-chef


# Copy recipe from planner
COPY --from=planner /app/recipe.json recipe.json

# Build dependencies (this layer is cached unless dependencies change)
RUN cargo chef cook --release --recipe-path recipe.json

# Copy entire workspace
COPY . .
# Build the actual application
RUN cargo build --release --bin cdc-cli
WORKDIR /app/ui
RUN npm install && npm run build

# Stage 3: Export (optional - for extracting binary to host)
FROM scratch AS export
COPY --from=builder /app/target/release/cdc-cli /cdc-cli

# Stage 4: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    nginx \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary from builder
COPY --from=builder /app/target/release/cdc-cli /usr/local/bin/cdc-cli
COPY --from=builder /app/ui/dist /app

COPY scripts/nginx.conf /etc/nginx/nginx.conf
COPY scripts/start.sh /scripts/start.sh

# Create non-root user and set up permissions BEFORE switching user
RUN useradd -m -u 1000 cdc \
    && chmod +x /scripts/start.sh \
    && chown -R cdc:cdc /app \
    && mkdir -p /var/log/nginx /var/lib/nginx /run \
    && chown -R cdc:cdc /var/log/nginx \
    && chown -R cdc:cdc /var/lib/nginx

# Expose API port
EXPOSE 4000 8080

# Default command - run as root to allow nginx to bind to port 80
CMD ["/scripts/start.sh"]