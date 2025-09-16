## Rewrite
This branch contains a rewrite of the set of tools in the STPS repo. The code on [`main` (`961072a
`)](https://github.com/paritytech/polkadot-stps/commit/961072a2afb63dc55553866f629adb4970ecbf9b) branch is split into multiple crates in multiple folders with multiple binaries. To make things more complex there's also unmerged changes/improvments by Andrei in [`sandreim/tps_ng` branch](https://github.com/paritytech/polkadot-stps/blob/sandreim/tps_ng/stps/src/main.rs) and by `s0me0ne-unkn0wn` in [`s0me0ne/ethereum2` branch](https://github.com/paritytech/polkadot-stps/tree/s0me0ne/ethereum2). The latter adds even more binary tools and adds support for Ethereum Based chains (e.g. `Mythical Games` parachain). 

> [!NOTE]
> There's a LOT of code duplication in these branches and code quality is not as high as one would like

### Goal
The goal of this rewrite is to have one single binary with support for multiple subcommands and with support both Polkadot based chains as well as Ethereum based ones (e.g. `Mythical Games`), and no code duplication and aiming for code quality to be proud on

### Status
- [x] Cli args with support for chain selection
- [x] Multi-chain support for creation of signers
- [x] Multi-chain support for fetching nonces
- [ ] Submit transactions (WIP)
- [ ] Subcommand for funding accounts
- [ ] Cli args for selecting which kind of transaction to send (e.g. simple fungible token transfer or NFT minting or other)