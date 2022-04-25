# Polkadot sTPS

This repository is meant to aggregate performance benchmarks from the Polkadot Ecosystem and make them into Standard Transaction Per Second ([sTPS](https://github.com/paritytech/ecosystem-performance-benchmarks/blob/main/docs/introduction.md)).

The measurements are intended to replicate different possible network topologies and provide reference estimates of throughput capacity of substrate-based multichain environments.

The project is centered around [Substrate Parachain Template](https://github.com/substrate-developer-hub/substrate-parachain-template/).

However, instead of `polkadot-launch`, we use [`zombienet`](https://github.com/paritytech/zombienet) for its convenient features as a DSL-based framework tailored for E2E testing.

Please refer to [docs](./docs) for more information.