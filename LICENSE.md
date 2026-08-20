# Licence

djmanzo is dual-licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

## Why two

It is the Rust ecosystem's convention, and it is the one that keeps the most
doors open: MIT is the simplest permissive licence, and Apache-2.0 adds an
explicit patent grant that some downstream users need before they can adopt a
dependency.

## Why permissive at all

[ADR-0002](docs/adr/0002-clean-room-and-licensing.md) is the long answer. The
short one: djmanzo is a clean-room implementation. No GPL or AGPL code is
linked, vendored or copied into it — not a library, not a file, not a single
function — because doing so would force the whole application under those
terms. Every dependency's licence is recorded in
[docs/RESEARCH.md](docs/RESEARCH.md), and permissive (MIT, Apache-2.0, BSD) or
file-level copyleft (MPL-2.0) is the whole of what is allowed.

## Contributions

Unless you state otherwise, any contribution you intentionally submit for
inclusion in this work, as defined in the Apache-2.0 licence, shall be
dual-licensed as above, without any additional terms or conditions.
