FROM rust:latest as builder

COPY . /build

WORKDIR /build/utils/tps

ARG CHAIN
ARG VCS_REF
ARG BUILD_DATE

ENV FEATURE=${CHAIN}

RUN cargo build --features=$FEATURE --release

FROM docker.io/library/ubuntu:20.04

COPY --from=builder /build/utils/tps/target/release/tps /usr/local/bin

LABEL description="Docker image for sTPS tps binary" \
	io.parity.image.authors="mattia@parity.io, devops-team@parity.io" \
	io.parity.image.vendor="Parity Technologies" \
	io.parity.image.description="Used to calculate (s)TPS" \
	io.parity.image.created="${BUILD_DATE}" \
    io.parity.image.source="https://github.com/paritytech/polkadot-stps/blob/${VCS_REF}/utils/dockerfiles/Dockerfile.sender"

RUN useradd -m -u 1000 -U -s /bin/sh -d /tps tps && \
    rm -rf /usr/bin /usr/sbin

USER tps

ENTRYPOINT [ "/usr/local/bin/tps" ]
