# Builder
FROM rust:latest AS build

RUN apt-get update && apt-get install musl-tools pkg-config libssl-dev perl -yy
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app
COPY . .

RUN cargo build --target=x86_64-unknown-linux-musl --release

# Runner
FROM alpine:latest

COPY --from=build /app/target/x86_64-unknown-linux-musl/release/ancymon /srv/ancymon
WORKDIR /srv

CMD ["./ancymon"]
