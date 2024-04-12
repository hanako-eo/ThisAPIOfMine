FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

RUN cargo install cargo-build-deps

WORKDIR /app

RUN cargo new --bin tsom_api
WORKDIR /app/tsom_api

COPY Cargo.toml ./
RUN cargo build-deps --release

COPY src ./src
RUN cargo build --release

FROM alpine:3 AS runtime

EXPOSE 14770

WORKDIR /app

COPY --from=builder /app/tsom_api/target/release/this_api_of_mine ./

CMD ["/app/this_api_of_mine"]
