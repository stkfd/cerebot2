FROM rust:1.39-slim-stretch as build
RUN apt-get update -qq && apt-get install libssl-dev pkg-config -y

WORKDIR /cerebot
ADD Cargo.toml Cargo.lock /cerebot/
RUN mkdir src && echo "fn main() { }" > src/main.rs
RUN cargo build --release

COPY ./. /cerebot/
RUN cargo build --release

FROM rust:1.39-slim-stretch

COPY --from=build /cerebot/target/release/. /opt/cerebot/

ENTRYPOINT ["/opt/cerebot/cerebot2"]
