# Bundled translation data

`data/translations/` holds the seven public-domain English translations that ship
with the app. Each file is the binary translation format: the four bytes `BIBL`, a
little-endian `u16` format version, a `u32` metadata length, the metadata as
uncompressed JSON, a `u32` body length, and the zstd-compressed column body.

**No copyrighted translation text is in this repository.** Only the seven
public-domain translations below are bundled, and the raw USFM they were built from
is never committed. The importer and the tests use generated words only.

## The translations

| Id | Abbreviation | Translation | Edition | Licence | Source archive | SHA-256 |
|---|---|---|---|---|---|---|
| `kjv` | KJV | King James Version | 1611 | Public domain | <https://ebible.org/Scriptures/eng-kjv2006_usfm.zip> | `16b08c9248b3600662f66f1e055f61fc3a6c7275e347236ee45feb35dfb20cb3` |
| `web` | WEB | World English Bible | 2000 | Public domain | <https://ebible.org/Scriptures/engwebp_usfm.zip> | `732c17d3325563fc5d8d98157a6c3d128c887f6b8e5a5ad0f71f58e5d5eebfcb` |
| `bsb` | BSB | Berean Standard Bible | 2020 | Public domain | <https://ebible.org/Scriptures/engbsb_usfm.zip> | `e4830de52921a22c90b7fef725b2490f3b90c16a3dd8f800b82c859814e52f96` |
| `asv` | ASV | American Standard Version | 1901 | Public domain | <https://ebible.org/Scriptures/eng-asv_usfm.zip> | `fa116257643253288c48a46f499faaaa501dfdd8381c78e1c67b745a9e1a999a` |
| `ylt` | YLT | Young's Literal Translation | 1862 | Public domain | <https://ebible.org/Scriptures/engylt_usfm.zip> | `63ef17706cc59cdbf7921627a40dc819e59ae2903d6f85ab23efc49d6da723f2` |
| `darby` | DARBY | Darby Bible | 1890 | Public domain | <https://ebible.org/Scriptures/engDBY_usfm.zip> | `2aadc6df9a99c04e32a9a167a708f8c25614fc781f3ff8231787f7630ca2d000` |
| `webster` | WEBSTER | Webster Bible | 1833 | Public domain | <https://ebible.org/Scriptures/engwebster_usfm.zip> | `34c9b0c8272c00f183de9f856ded411c70e22b148f5d164543f20e2987a9a5c7` |

## How the files were built

```text
cargo run -p xtask -- build-data
```

The task downloads each archive from eBible.org into `target/data-cache`, checks
the archive's SHA-256 against the digest pinned in the table above, imports the
archive with the same USFM importer users get, lays it on the canonical grid, and
encodes it with the core crate's encoder. Nothing is written until all seven
archives have passed their digest check and all seven translations have converted,
so an interrupted or tampered build leaves the previous files untouched.

The task also rewrites this file from the same table, so a digest or a licence
cannot drift between the build and this document.

## Verifying

```text
cargo run -p xtask -- verify-data
```

The task decodes every file and checks that it starts with `BIBL`, that its
metadata id and provenance match the pins, that it holds all 31102 verses of the
canonical grid, that Genesis 1:1 and Revelation 22:21 read exactly as the edition
prints them, and that the cached archive still hashes to the pinned digest. It
prints one line per translation and exits non-zero when any check fails.

## Licence

All seven translations are in the public domain, and each file carries its
edition's notice in the metadata header so the notice travels with the text. The
build tooling in `xtask` is MIT licensed.
