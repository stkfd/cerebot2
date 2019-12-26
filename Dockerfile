FROM rust:1.40-slim-stretch as build
RUN apt-get update -qq && apt-get install libssl-dev pkg-config -y

WORKDIR /cerebot

# workspace config
ADD Cargo.toml Cargo.lock /cerebot/

# main bot config
ADD bot/Cargo.toml /cerebot/bot/
RUN mkdir bot/src && echo "fn main() { }" > bot/src/main.rs

# lib configs
ADD persistence/Cargo.toml /cerebot/persistence/
RUN mkdir persistence/src && touch persistence/src/lib.rs
ADD util/Cargo.toml /cerebot/util/
RUN mkdir util/src && touch util/src/lib.rs

# build only dependencies with dummy lib/main files
RUN cargo build --release

# copy full source
COPY ./. /cerebot/
RUN cargo build --release

FROM rust:1.40-slim-stretch

COPY --from=build /cerebot/target/release/. /opt/cerebot/

ENTRYPOINT ["/opt/cerebot/cerebot2"]
