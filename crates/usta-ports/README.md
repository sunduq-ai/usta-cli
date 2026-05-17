# `usta-ports`

Trait definitions (ports) for the [`usta`](https://crates.io/crates/usta)
project scaffolder. Pure interfaces only — no implementations, no I/O.

This crate lets third parties plug a new adapter (e.g. a different
template source, a different package manager) into `usta` without
touching the core engine. See the
[`usta` repo](https://github.com/sunduq-ai/usta-cli) for the architecture
overview.

## License

Dual-licensed under MIT or Apache-2.0 at your option.
