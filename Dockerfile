# BUILD STAGE
FROM rust:1.88-slim AS builder

WORKDIR /app

# Install build + git
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rust-docs && \
    mkdir -p /docs && \
    cp -r /usr/local/rustup/toolchains/*/share/doc/rust/html /docs

# cache deps
COPY Cargo.toml Cargo.lock ./
COPY rusty-core/Cargo.toml ./rusty-core/
COPY rusty-cli/Cargo.toml ./rusty-cli/

RUN mkdir -p rusty-core/src rusty-cli/src && \
    echo "fn main() {}" > rusty-cli/src/main.rs && \
    echo "fn main() {}" > rusty-core/src/lib.rs

RUN cargo build --release --package rusty-cli --bin rusty-cli

# source code
COPY . .

# prompt a minimal recompile
RUN touch rusty-cli/src/main.rs
RUN touch rusty-core/src/lib.rs

RUN cargo build --release --package rusty-cli --bin rusty-cli

# runtime
FROM debian:bookworm-slim


# Install git + certificates + tools
RUN apt-get update && apt-get install -y \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /docs /docs

# Security: non-root user
RUN useradd -m -u 1000 rusty && \
    mkdir -p /workspace /sessions /logs && \
    chown -R rusty:rusty /workspace /sessions /logs /docs

USER rusty
WORKDIR /workspace

# Copy only the tiny binary
COPY --from=builder /app/target/release/rusty-cli /usr/local/bin/rusty

# ENTRYPOINT ["rusty"]

CMD ["rusty", "--session", "rusty", "--repo", "AlchemicRaker/rusty", "--issue", "3"]