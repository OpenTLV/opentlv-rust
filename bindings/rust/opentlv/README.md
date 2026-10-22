# opentlv

Safe Rust bindings for [OpenTLV](https://github.com/OpenTLV/opentlv), a
universal library for parsing, encoding, and working with TLV and TLV-like
binary formats.

The crate wraps the OpenTLV C library through
[`opentlv-sys`](https://github.com/OpenTLV/opentlv-rust/tree/main/bindings/rust/opentlv-sys)
with safe ownership and lifetime contracts: readers and writers, length and
structure schemas, typed codecs, DER and CER canonical rules, Query and the EMV
dictionary. No raw pointers appear in the public API.

```rust
use opentlv::{Format, Reader, Tag, Writer};

fn main() -> opentlv::Result<()> {
    let mut buf = [0u8; 16];
    let mut writer = Writer::with_format(&mut buf, Format::Ber);
    writer.write(&Tag::from_bytes(&[0x50]), b"VISA")?;

    for element in Reader::with_format(writer.written(), Format::Ber) {
        let element = element?;
        assert_eq!(element.value(), b"VISA");
    }
    Ok(())
}
```

## Status

The crates are not published to crates.io yet. Until they are, depend on a
checkout of [opentlv-rust](https://github.com/OpenTLV/opentlv-rust):

```toml
[dependencies]
opentlv = { path = "opentlv-rust/bindings/rust/opentlv" }
```

## Build requirements

Rust 1.82 or newer, CMake 3.16 or newer, a C99 compiler and Git. On Windows,
use the MSVC Rust toolchain with Visual Studio. By default `opentlv-sys` clones
the pinned OpenTLV Core commit from GitHub and builds it as a static library;
see its README for the Core dependency and for linking a prebuilt library.

## Cargo features

| Feature | Default | Enables |
| --- | --- | --- |
| `lldp` | yes | `Format::Lldp` and Core's LLDP component |
| `nfc` | yes | `Format::NfcType2` and Core's NFC component |
| `document` | yes | `Document`, `DocumentBuilder`, the Query edit API and the Core document component |

Each feature forwards to the `opentlv-sys` feature of the same name.

## Versioning

`opentlv` and `opentlv-sys` are released together with the same version, and
`opentlv` requires exactly the `opentlv-sys` version it was released with.
The crate version is independent of the OpenTLV Core version;
`opentlv::version()` reports the version of the linked Core library. See
[packaging and versioning](https://github.com/OpenTLV/opentlv-rust/blob/main/docs/packaging.md).

## License

[MIT](LICENSE)
