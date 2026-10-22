# opentlv-rust

Rust bindings for [OpenTLV](https://github.com/OpenTLV/opentlv), a universal
library for parsing, encoding, and working with TLV and TLV-like binary formats.

The Rust bindings are being migrated out of Core
([migration epic](https://github.com/OpenTLV/opentlv/issues/670)). This
repository contains the source import described in
[issue #1](https://github.com/OpenTLV/opentlv-rust/issues/1): the existing Cargo
workspace, its sources, tests and examples, preserved byte-for-byte from Core.
Standalone build integration is a follow-up: the imported workspace still
requires the original Core build tree, so `cargo build` does not work in this
repository yet. Until then, build and develop the bindings in
[`bindings/rust`](https://github.com/OpenTLV/opentlv/tree/main/bindings/rust)
in Core.

## Layout

| Path | Content |
| --- | --- |
| `bindings/rust/` | Cargo workspace with the `opentlv-sys` and `opentlv` crates, their tests and seven examples |
| `docs/guides/rust.md` | Preserved Rust user guide |
| `docs/development/rust.md` | Preserved Rust developer guide |
| `docs/migration.md` | Migration baseline, source import and validation results |
| `docs/migration/` | Source inventory, original CI configuration and validation evidence |
| `scripts/verify_baseline.py` | Verifies the inventory against the pinned Core tree |
| `scripts/verify_migration.py` | Verifies the imported files against the inventory and the pinned Core tree |

The preserved guides describe the original Core checkout. Their commands
and links to other Core documents retain that context; they are not standalone
instructions. Use the pinned originals linked in the
[migration record](docs/migration.md) for cross-repository documentation links.

## Migration verification

With a sibling Core checkout that contains the recorded commit:

```sh
python -B scripts/verify_migration.py --core ../opentlv
```

This needs Python 3.11 or newer and Git. It runs the baseline verification,
then checks every imported file byte-for-byte against its original Git blob
and rejects missing, extra or modified files in `bindings/rust`. It does not
build anything. See the [migration record](docs/migration.md) for the baseline
and imported-source build and test results and known failures.

## License

[MIT](LICENSE)
