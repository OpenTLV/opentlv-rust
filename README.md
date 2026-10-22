# opentlv-rust

Rust bindings for [OpenTLV](https://github.com/OpenTLV/opentlv), a universal
library for parsing, encoding, and working with TLV and TLV-like binary formats.

The Rust bindings are being migrated out of Core
([migration epic](https://github.com/OpenTLV/opentlv/issues/670)). Until the
migration is complete, the implementation, its build instructions and its
documentation stay in
[`bindings/rust`](https://github.com/OpenTLV/opentlv/tree/main/bindings/rust)
in Core.

This repository currently holds the recorded migration baseline only:

| Path | Content |
| --- | --- |
| `docs/migration.md` | Source commit, crate versions, Cargo features, linking modes, platforms, inventory and baseline results |
| `docs/migration/` | Source inventory, validation report and compact baseline log |
| `scripts/verify_baseline.py` | Verifies the inventory against the pinned Core tree |

## Baseline verification

With a sibling Core checkout that contains the recorded commit:

```sh
python -B scripts/verify_baseline.py --core ../opentlv
```

This needs Python 3.11 or newer and Git. It checks file contents, history,
crate versions and Cargo features at the recorded commit. It does not build
anything. See the [migration record](docs/migration.md) for the baseline build
and test results and known failures.

## License

[MIT](LICENSE)
