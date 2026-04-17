FROM rust:1.93-slim AS builder
WORKDIR /app

COPY backend/Cargo.toml backend/Cargo.toml
COPY backend/src backend/src
COPY frontend frontend

RUN cargo build --release --manifest-path backend/Cargo.toml

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/backend
COPY --from=builder /app/backend/target/release/backend /app/backend/backend
COPY --from=builder /app/frontend /app/frontend

ENV RUST_LOG=info
ENV AIRLOCK_SECURITY_PROFILE=NONE
EXPOSE 8080
EXPOSE 4841
EXPOSE 4842
EXPOSE 4843
EXPOSE 4844

CMD ["/app/backend/backend"]
