## Build Stage 
FROM rust:alpine3.17 as builder

# Add Musl Dependency
RUN apk update && apk add musl-dev
RUN rustup target add x86_64-unknown-linux-musl

# Prebuild Dependencies
RUN cargo new --bin app
WORKDIR /app
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm -f target/x86_64-unknown-linux-musl/release/deps/devops_replica_controller_operator*

# Build Application
COPY ./src ./src
RUN cargo build --release --target x86_64-unknown-linux-musl

## Main Stage 
FROM alpine:3.17

# Add Kubectl Dependency
RUN apk add --no-cache curl
RUN curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
RUN chmod +x kubectl
RUN mv kubectl /usr/local/bin/kubectl

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/devops_replica_controller_operator . 
RUN chmod +x devops_replica_controller_operator

CMD ["./devops_replica_controller_operator"]
