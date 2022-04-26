# This file is sourced from https://github.com/paritytech/polkadot/blob/master/scripts/ci/dockerfiles/polkadot/polkadot_builder.Dockerfile
# This is the build stage for Polkadot. Here we create the binary in a temporary image.
FROM docker.io/paritytech/ci-linux:production as builder

WORKDIR /polkadot
COPY . /polkadot

RUN cargo build --locked --release

# This is the 2nd stage: a very small image where we copy the Polkadot binary."
FROM docker.io/library/ubuntu:20.04

LABEL description="Multistage Docker image for Polkadot-stps" \
	io.parity.image.type="builder" \
	io.parity.image.authors="devops-team@parity.io" \
	io.parity.image.vendor="Parity Technologies" \
	io.parity.image.description="Polkadot-stps: Zombienet-based e2e performance benchmarks (TPS) from Polkadot Ecosystem" \
	io.parity.image.source="https://github.com/paritytech/polkadot-stps/blob/${VCS_REF}/scripts/dockerfiles/polkadot-stps_builder.Dockerfile" \
	io.parity.image.documentation="https://github.com/paritytech/polkadot-stps"

COPY --from=builder /polkadot/target/release/parachain-collator /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /polkadot polkadot && \
	mkdir -p /data /polkadot/.local/share && \
	chown -R polkadot:polkadot /data && \
	ln -s /data /polkadot/.local/share/polkadot && \
# unclutter and minimize the attack surface
	rm -rf /usr/bin /usr/sbin && \
# check if executable works in this container
	/usr/local/bin/parachain-collator --version

USER polkadot

EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/parachain-collator"]
