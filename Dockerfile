FROM rust:1.76

COPY . .

WORKDIR /registry

RUN cargo install --path .

CMD ["pesde-registry"]