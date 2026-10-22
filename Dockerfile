# --- Builder Stage ---
FROM rust:slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY index.html ./

# Build optimized release binary
RUN cargo build --release

# --- Runner Stage ---
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy only the compiled binary (index.html is embedded via include_str!)
COPY --from=builder /app/target/release/voice-assistant /app/voice-assistant

EXPOSE 8080
ENV PORT=8080

CMD ["/app/voice-assistant"]
