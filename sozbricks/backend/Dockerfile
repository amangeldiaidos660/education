FROM rust:1.98.1-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
ARG GIT_SHA=unknown
ENV GIT_SHA=${GIT_SHA}
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 app \
    && useradd --uid 10001 --gid app --no-create-home app
WORKDIR /app
COPY --from=builder /app/target/release/sozbricks-api /app/sozbricks-api
USER 10001:10001
ENV HOST=0.0.0.0 PORT=8080 RUST_LOG=sozbricks_api=info
EXPOSE 8080
CMD ["/app/sozbricks-api"]
