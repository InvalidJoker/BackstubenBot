FROM rust:1.88-alpine AS builder

# Install dependencies for building
RUN apk add --no-cache musl-dev openssl-dev pkgconfig build-base

# Set workdir
WORKDIR /app

# Copy manifest and fetch dependencies first for better caching
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build in release mode
RUN cargo build --release

FROM alpine:3.20 AS runtime

# Install runtime dependencies
RUN apk add --no-cache ca-certificates openssl

# Copy compiled binary from builder
COPY --from=builder /app/target/release/BackstubenBot /usr/local/bin/backstubenbot

# Make binary executable
RUN chmod +x /usr/local/bin/backstubenbot

# Set working directory
WORKDIR /app

# Expose no ports (Discord bot doesn't need inbound traffic)
CMD ["backstubenbot"]
