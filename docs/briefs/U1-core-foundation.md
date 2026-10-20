GOAL
Build the foundation of the Bible reader: the canonical 66-book grid, the verse reference
type, the translation data model, and the binary translation format, with tests that prove
the model is exact.

SCOPE
You may write only inside the new repository. Create these paths:
  Cargo.toml                      the workspace root
  crates/core/Cargo.toml          the core crate manifest
  crates/core/src/lib.rs          the crate root and its module declarations
  crates/core/src/books.rs        the canonical grid
  crates/core/src/model.rs        the translation model
  crates/core/src/format.rs       the binary encode and decode
  crates/core/src/reference.rs     parsing and formatting references
  crates/core/tests/*.rs          integration tests
You may also write:
  .gitignore
  LICENSE
  THIRD_PARTY.md
Do not touch any other path. Do not create the app crate, the xtask crate, the data
directory, or the docs directory. Another unit owns those.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read the sections "The canonical
grid", "The translation model", and "The binary translation format" before you start.

A read-only reference implementation exists at /Users/madearga/bible-rs-reference. It is
on the local Mac, not on this host. You may not access it. Everything you need is in
docs/SPEC.md. If you need a detail the spec does not state, state the question in your
report and make a reasonable choice. Do not stall.

The binary format is fixed and is the on-disk contract. Its layout is:

    "BIBL" u16 version  u32 meta_len  meta_json   (uncompressed header)
    u32 body_len  zstd(body)

    body = u32 text_len text  u32 extra_len extra
           u16 x 1189 verses-per-chapter
           u32 n  then column arrays, one array each:
                   number u8 x n
                   number_end u8 x n
                   text_len u16 x n
                   marks_len u16 x n
                   spans_len u16 x n
           u32 m  marks, each: offset u16, kind u8, level u8, payload_start u32, payload_len u32
           u32 k  spans, each: start u16, end u16, style u8

All integers are little-endian. The column layout compresses far better than interleaved
records and decodes in a few linear passes. The header is uncompressed so that listing the
installed translations never decompresses anything.

The data model has these pieces.

The canonical grid is 66 books, 1189 chapters, 31102 verses. The book index is the
position in the canonical order, Genesis at 0 and Revelation at 65. The verses per chapter
are fixed by the KJV versification and are identical for every translation.

A translation is one UTF-8 string holding all its verse text, plus flat arrays. Each verse
carries its number, the byte range of its text, and the ranges into the marks and spans
arrays. A mark is a structural break: paragraph, poetry line, poetry indent level, stanza
break, heading, Psalm superscription. A span is a character style: words of Christ,
translator additions, the divine name, Selah. Offsets inside a verse are 16-bit byte
offsets relative to the verse start.

The model needs a builder that the importers will use to construct a translation, since
the importers are a later unit. The builder appends verses in order and keeps the marks
and spans flattened.

The reference needs parsing that accepts the shapes a reader types. It must accept a book
name, an abbreviation, a number prefix such as "1" for "1 John", and a chapter and verse.
It must be tolerant of a wrong book name, so "revalation 21" resolves. It must reject a
reference to a chapter that does not exist, and a verse beyond the end of a chapter.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- The grid test asserts the book count is 66, the chapter count is 1189, the verse count is
  31102, and that the chapter offsets are consistent with each book's chapter count.
- The reference test asserts that "jn 3:16" resolves to John 3 verse 16, that "1co13" and
  "1co 13" resolve to 1 Corinthians 13, that "revalation 21" resolves to Revelation 21,
  that "ps 23" resolves to Psalm 23, and that "rev 22:21" resolves.
- The reference test asserts that an out-of-range chapter and an out-of-range verse are
  both rejected with an error, and that a garbled string is rejected.
- The format test asserts that a translation built through the builder encodes and decodes
  back to an equal value, and that the decoded value is equal field by field.
- The format test asserts that the encoded bytes begin with the ASCII bytes BIBL.
- The format test asserts that a round trip through the format preserves the verse text
  exactly, including multi-byte UTF-8 characters.
- The decode path allocates nothing per verse while materialising a chapter.
- Every public type and function has a doc comment that says what it is for, not what it
  does.
- Every file starts with the SPDX header: // SPDX-License-Identifier: MIT
  and a copyright line for madearga.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

All three must pass with no warnings and no ignored tests. If clippy complains, fix the
code, do not silence the lint.

Then measure the decode time and report the number:

    cargo test --release -- --nocapture decode_benchmark

The test must print how long it takes to decode a synthetic full-Bible translation, and
that time must be under 50 ms. The synthetic translation is built in the test, with 31102
verses of generated text, so the test does not need any Bible text.

TIMEBOX
Four hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from the reference implementation. Write your own.
Do not name the reference author anywhere.
Do not add a dependency that the spec does not need. The core crate needs anyhow,
serde, serde_json, memchr, and zstd. Nothing else until an importer needs it.
Do not add a feature flag, a config file, or an abstraction layer that no caller needs.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: what the three verify commands printed, and the decode time in ms.
- The data shape you chose for the translation model and the builder, in a few sentences.
  Say why the builder API is shaped that way.
- The public API of each module, as a short list of the type and function names.
- Deviations from this brief, and why.
- Suggested follow-ups.
