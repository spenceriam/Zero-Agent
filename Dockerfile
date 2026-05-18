# Build stage
FROM rust:1.77-slim AS builder

WORKDIR /app

# Copy Cargo files first for dependency caching
COPY bridge/rust/Cargo.toml bridge/rust/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Copy source and build
COPY bridge/rust/src/ src/
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 10000 zero

# Copy binary
COPY --from=builder /app/target/release/zero-agent-bridge /usr/local/bin/zero

# Create data directory
RUN mkdir -p /home/zero/.zero-agent && chown -R zero:zero /home/zero

USER zero
WORKDIR /home/zero

# Default config volume
VOLUME ["/home/zero/.zero-agent"]

ENTRYPOINT ["zero"]
CMD []
