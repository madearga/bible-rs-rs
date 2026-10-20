GOAL
Build the importers that turn a translation file the user owns into the binary translation
format: USFM, OSIS XML, Zefania XML, and SWORD modules.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/core/src/import/mod.rs    the importer entry points
  crates/core/src/import/usfm.rs    the USFM parser
  crates/core/src/import/osis.rs    the OSIS XML parser
  crates/core/src/import/zefania.rs the Zefania XML parser
  crates/core/src/import/sword.rs   the SWORD module reader
  crates/core/src/import/zip.rs     the safe archive reader
  crates/core/tests/import_*.rs    the tests
  crates/core/Cargo.toml            add the dependencies you need
Do not touch any other path. In particular do not touch books.rs, model.rs, format.rs, or
reference.rs. Another unit owns those, and your unit builds on the API they define.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read "The translation model", "The
binary translation format", and "Importing a translation the user owns" before you start.

You build on the model, the builder, and the binary format that unit U1 defines. Read
U1's merged code for the exact names. The builder appends verses in order and keeps marks
and spans flattened. The model holds one text string plus flat arrays for verses, marks,
and spans. Offsets inside a verse are 16-bit byte offsets relative to the verse start.

Four input formats are accepted.

USFM. A .usfm or .sfm file, a folder of them, or a zip of them. The markers that carry
structure are the paragraph markers (p, q, q1, q2, q3, m, mi, mq, nb, b, l, r, s, d, sp,
cls, pc, pi, pr, mi), the character markers that carry style (red, w, add, q, sls, usfm
additions, and so on), the heading markers (h, toc, mt, ms, sr, sp, ide, rem), and the
ideographic markers that mark the divine name. A verse number marker is \id or a \v marker
or a bare number at the start of a line. A \c marker sets the chapter. A \toc or \mt line
carries a book title. A \ide line carries the translation's title.
The parser must:
- Skip the apparatus and the footnote and cross-reference content, because the reader does
  not show them.
- Skip the books that are not in the 66-book grid, so the deuterocanonical books do not
  shift the grid.
- Handle a verse range marker that names a range of verses.
- Handle a book that has no title marker.
- Be tolerant of an unclosed marker at the end of the file.

OSIS XML. A .xml or .osis file. The elements that matter are the verse elements with their
sID attribute, the chapter elements, and the text inside them. The text can be split across
several elements, so a verse's text is the concatenation of its parts.

Zefania XML. A .xml file. The same shape as OSIS but with its own element names and a
different reference attribute.

SWORD module. The module zip, its unpacked folder, or its mods.d/*.conf file. The module
holds a manifest that declares the versification and the text driver. Support the zText
and RawText drivers with the KJV, KJVA, NRSV, and NRSVA versifications, and the OSIS, ThML,
and GBF markup dialects. A locked module needs the user's unlock key, which the caller
passes in. Verify the key the way SWORD itself does, and give a clear error when it is
wrong.

A zipped input is read through a safe archive reader that refuses an entry whose path
escapes the extraction root, refuses an entry with an absolute path, and refuses a
decompression bomb, meaning an entry that expands far beyond its compressed size. A
malformed archive gives a clear error, never a panic.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- The USFM test parses a small synthetic USFM document with two chapters, a paragraph
  marker, a heading, a verse range, a red-letter span, and an italic addition, and the
  resulting verses carry the right text, the right numbers, and the right marks and spans.
- The USFM test asserts that a non-canonical book in the document is skipped and that the
  canonical book after it lands on the right chapter.
- The USFM test asserts that an unclosed marker at the end of the file does not panic.
- The OSIS test parses a small synthetic OSIS document and the resulting verses carry the
  right text.
- The Zefania test parses a small synthetic Zefania document and the resulting verses carry
  the right text.
- The zip test asserts that an entry named "../escape.txt" is refused and that the
  extraction root still holds only the expected file.
- The zip test asserts that an entry that expands to a huge size is refused.
- A malformed input of each of the four formats gives an error, never a panic. Write a
  test that feeds each one a truncated file and asserts the result is an error.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header: // SPDX-License-Identifier: MIT
  and a copyright line for madearga.
- The tests use synthetic text only. No copyrighted Bible text. No real Bible text either,
  so that no licence question can arise from a test fixture.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

All three must pass with no warnings and no ignored tests. If clippy complains, fix the
code, do not silence the lint.

Then prove the importers work on real public-domain input. The KJV archive is available at
https://ebible.org/Scriptures/KJV_usfm.zip. Run:

    cargo run -p xtask -- --help

If xtask does not exist yet, download the archive yourself with curl, unzip it, and run
your importer over the .usfm file with a small throwaway binary or a test, and report the
number of verses it produced and the first verse of Genesis 1. The KJV must give 31102
verses and Genesis 1:1 must read "In the beginning God created the heaven and the earth."
Paste that output into your report.

TIMEBOX
Six hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not bundle or commit any Bible text, public domain or not. The importers are tested
against synthetic documents and against a download that is not committed.
Do not add a dependency that is not needed. The core crate needs anyhow, serde,
serde_json, memchr, zstd, and for this unit a zip reader, a fast XML reader, and a
SHA-2 implementation. Choose the ones you need and say why in the report.
Do not add a feature flag, a config file, or an abstraction layer that no caller needs.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: what the three verify commands printed, and the KJV import result.
- The public API of each importer, as a short list of the type and function names.
- The choices you made about marker handling, and what you rejected.
- Deviations from this brief, and why.
- Suggested follow-ups.
