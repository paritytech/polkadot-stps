# Standard Transactions Per Second

Standard Transactions Per Second (sTPS) is a cross chain standard in what counts as a transaction, and basically means a "keep alive" balance-transfer from one pre-existing account to another pre-existing account, assuming worst case access conditions for those accounts, which is that neither account has been previously read from or written to in the block.

Main points of sTPS are:

- Keep Alive Balance transfer
- Neither account may have been read/written/touched/cached thus far in the benchmarks (worst case scenario for Substrate)
- No account cleanup
- No account initialisation

Please refer to [methodology.md](./methodology.md) for information on **how** sTPS is being measured in practice, and to [results.md](./results.md) for the actual numbers.

# Ecosystem Performance

ToDo
