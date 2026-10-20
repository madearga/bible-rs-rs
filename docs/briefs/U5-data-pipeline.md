GOAL
Build the data pipeline: the xtask that downloads the seven public-domain translations,
verifies them, converts them with the importer, and produces the binary translation files
that ship with the app.

SCOPE
You may write only inside the new repository. Create or edit:
  xtask/Cargo.toml            the xtask manifest
  xtask/src/main.rs           the task runner and the data build
  data/translations/*.bbl     the built translation files
  data/README.md              what the files are and how they were built
  .gitignore                  the build outputs that must not be committed
Do not touch any other path. In particular do not touch crates/core. Another unit owns the
core crate, and you use its importer as a library.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read "Building the bundled data" and
"The binary translation format" before you start.

Seven public-domain translations ship with the app: KJV, WEB, BSB, ASV, YLT, Darby, and
Webster. Their text comes from the eBible.org USFM archives. The download URL and the
SHA-256 digest of every archive are pinned in the repository, and the build verifies each
digest before it uses the file.

The build tool is run as `cargo run -p xtask -- build-data`. It downloads the archives,
verifies the digests, runs the USFM importer that users get when they import a file,
converts each translation to the binary format, and writes the result to
data/translations/<id>.bbl. The raw USFM is never committed. Only the derived binary
format is.

The archives are at this shape:
    https://ebible.org/Scriptures/<ID>_usfm.zip
where the id is the eBible.org identifier for that translation. The identifiers are KJV,
WEB, BSB, ASV, YLT, DBY, and WBS.

The binary format is fixed. Its layout is:
    "BIBL" u16 version  u32 meta_len  meta_json   (uncompressed header)
    u32 body_len  zstd(body)
The core crate has the encoder. Use it rather than writing a second one.

The metadata carries the id, the abbreviation, the full name, the language, the licence
summary, the copyright notice, the provenance, the per-book titles, and the flags for red
letters, paragraphs, and headings. The licence summary and the provenance must be right,
because the app shows them to the user and because the notices are a licence obligation.

ACCEPTANCE
- cargo build succeeds with no warnings.
- `cargo run -p xtask -- build-data` downloads all seven archives, verifies every digest,
  converts every translation, and writes seven .bbl files.
- The build fails loudly when a digest does not match, and it says which archive failed
  and what the expected and the actual digest are.
- The build does nothing at all when a digest does not match. It must not write a partial
  or an unverified file.
- Each .bbl file starts with the four ASCII bytes BIBL and decodes back to a translation
  with 31102 verses.
- The KJV file decodes to a translation whose Genesis 1:1 text is
  "In the beginning God created the heaven and the earth."
- The KJV file decodes to a translation whose Revelation 22:21 text is
  "The grace of our Lord Jesus Christ be with you all. Amen."
- The total size of the seven files is under 8 MB.
- data/README.md states, for each translation, the abbreviation, the full name, the
  edition year, the licence, and the source URL with the SHA-256 digest.
- data/README.md states plainly that no copyrighted translation text is in this
  repository.
- Every file starts with the SPDX header: // SPDX-License-Identifier: MIT
  and a copyright line for madearga. The .bbl files are binary and carry no header, which
  is fine, but every source file does.
- The .bbl files are committed. The raw USFM archives and the downloaded zips are not, and
  the .gitignore says so.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo run -p xtask -- build-data

All must pass. Then prove the output is right:

    cargo run -p xtask -- verify-data

That task must decode each .bbl and assert the verse count, the first verse of Genesis, the
last verse of Revelation, and the digest of the archive it came from. It must print one
line per translation and exit non-zero if any check fails. Paste the output.

Then report the size of the seven files:

    du -sh data/translations

TIMEBOX
Four hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not commit the downloaded zips or the raw USFM files.
Do not bundle any copyrighted translation text. Only the seven public-domain ones.
Do not weaken the digest check. Do not make the mismatch a warning. It is a hard failure.
Do not add a dependency that is not needed. A SHA-2 implementation and a zip reader are the
ones this unit needs, and the core crate may already have them.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: what the four verify commands printed, and the size of the seven files.
- The metadata you set for each translation, as a short table.
- How you handle a digest mismatch, in a few sentences.
- Deviations from this brief, and why.
- Suggested follow-ups.
