FROM rust:1.98-alpine AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock askama.toml build.rs ./
COPY src ./src
COPY site ./site
RUN cargo build --locked --release

FROM scratch
COPY --from=builder /app/target/release/blog /blog
USER 10001:10001
EXPOSE 3000
ENTRYPOINT ["/blog"]
