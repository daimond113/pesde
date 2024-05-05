FROM rust:1.77-bookworm AS builder

COPY . .

WORKDIR /registry

RUN cargo build --release

FROM debian:bookworm-slim

COPY --from=builder /target/release/pesde-registry /usr/local/bin/

RUN apt-get update && apt-get install -y ca-certificates

CMD ["/usr/local/bin/pesde-registry"]
