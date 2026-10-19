# Mailvora API and workers. Build from this directory.

FROM rust:1.99-bookworm AS build
WORKDIR /src
# Cache dependencies separately from application code.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main(){}' > src/main.rs && echo '' > src/lib.rs \
 && cargo build --release --locked && rm -rf src target/release/deps/mailvora* target/release/mailvora*
COPY . .
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home /app --shell /usr/sbin/nologin mailvora
WORKDIR /app
COPY --from=build /src/target/release/mailvora /usr/local/bin/mailvora
ENV HTTP_BIND=0.0.0.0:8088 \
    LOG_FORMAT=json
USER mailvora
EXPOSE 8088
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s CMD ["/usr/local/bin/mailvora", "healthcheck"]
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/mailvora"]
CMD ["serve"]
