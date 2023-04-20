FROM rust:latest as builder

COPY . /build

WORKDIR /build/utils/tps

RUN cargo build

FROM docker.io/library/ubuntu:20.04

COPY --from=builder /build/utils/tps/target/debug/tps /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /tps tps

USER tps

ENTRYPOINT [ "/usr/local/bin/tps" ]
