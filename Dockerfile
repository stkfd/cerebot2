FROM rust:1.40-slim-stretch as build
RUN apt-get update -qq && apt-get install libssl-dev pkg-config libpq-dev -y

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

ADD unogs_client/Cargo.toml /cerebot/unogs_client/
RUN mkdir unogs_client/src && touch unogs_client/src/lib.rs

ADD web/backend/Cargo.toml /cerebot/web/backend/
RUN mkdir web/backend/src && echo "fn main() { }" > web/backend/src/main.rs

# build only dependencies with dummy lib/main files
RUN cargo build --bin cerebot2 --release

# cleanup
RUN rm -f target/release/deps/cerebot2* target/release/deps/backend* target/release/deps/libpersistence* target/release/deps/libutil*
RUN rm -f target/release/cerebot2* target/release/backend* target/release/libpersistence* target/release/libutil*
# copy full source
COPY . .

# real build and install
RUN cargo build --bin cerebot2 --release

FROM debian:stretch-slim
RUN apt-get update -qq && apt-get install libssl-dev libpq-dev ca-certificates -y

COPY --from=build /cerebot/target/release/cerebot2 /usr/local/bin/cerebot2
CMD ["cerebot2"]