# Third-party software

The core crate depends on the following crates. All are permissively licensed and
none of them ship Bible text.

| Crate | Version | Licence | Used for |
|---|---|---|---|
| anyhow | 1 | MIT OR Apache-2.0 | Error values that carry context |
| serde | 1 | MIT OR Apache-2.0 | Deriving the metadata serialisation |
| serde_json | 1 | MIT OR Apache-2.0 | The uncompressed metadata header |
| memchr | 2 | MIT OR Unlicense | Locating separators while parsing references |
| zstd | 0.13 | MIT | Compressing and decompressing the translation body |

The zstd crate links the upstream zstd C library, which is BSD-3-Clause.
