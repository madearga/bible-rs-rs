GOAL
Embed the seven real bundled translations into the application binary and make the reader, the
search, and the CLI operate on the real text, replacing the synthetic sample everywhere a user
would see Scripture.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/src/bundled.rs       the embedded translation table
  crates/app/src/lib.rs           the library wiring
  crates/app/src/app.rs           the reader and search loading real translations
  crates/app/src/cli.rs           the verbs reading real translations
  crates/app/tests/bundled.rs     the tests
  crates/app/Cargo.toml           the build wiring you need
Do not touch crates/core or xtask or data. Another unit owns those.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements
are in docs/SPEC.md. Read "The binary translation format", "Building the bundled data",
"Translations", "The CLI", and "The performance budget".

The seven .bbl files are already built and committed under data/translations/. The core crate
decodes them through format::decode and reads their metadata through format::read_meta. The
app currently loads a synthetic sample translation and lists the bundled ids by hand.

The embedding shape. The compressed bytes are include_bytes! into the binary, one entry per
translation, with the id, the display name, and the metadata read from the file header. A
translation decodes the first time it is used, on a background thread while the window opens
where the app can do so, so startup only pays for the one being read. The default translation
is the KJV.

The seven entries come from the actual files, not from a hand-written list, so the table and the
data cannot drift. A build-time check or a test must fail if a listed file is missing from
data/translations or a file there is not listed.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- A test asserts the embedded table lists exactly seven translations whose ids are kjv, web,
  bsb, asv, ylt, darby, and webster, and that each entry's bytes begin with the ASCII bytes
  BIBL.
- A test decodes each embedded translation and asserts 31102 verses.
- A test asserts the default is the KJV and that switching translation ids round-trips through
  the library.
- The CLI reads real text: `bible read "john 3:16"` prints the real King James verse, and the
  test asserts the exact text "For God so loved the world, that he gave his only begotten Son,
  that whosoever believeth in him should not perish, but have everlasting life."
- The CLI search works on real text: `bible search "beginning"` returns Genesis 1:1 among its
  matches, and the test asserts it.
- `bible translations` lists the seven with their names.
- The shot harness renders a real chapter: SHOT_REFERENCE="psalm 23" produces a PNG whose bytes
  differ from the sample-translation render of the same reference.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header naming madearga.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo run --release -p bible-app -- read "john 3:16"
    cargo run --release -p bible-app -- search "beginning" --limit 3
    cargo run --release -p bible-app -- translations

Then render two PNGs through the shot harness, one with the bundled KJV and one forced to the
sample if a seam for that still exists, and confirm they differ:

    SHOT_REFERENCE="psalm 23" cargo run --release --example shot -- /tmp/psalm23-bundled.png 1280x800

TIMEBOX
Four hours.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not modify any .bbl file. Read them as they are.
Do not commit any new Bible text. The .bbl files are already committed; nothing else is added.
Do not add a dependency that is not needed.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: the verify outputs, the PNG paths, and the binary size growth from embedding.
- How the table is generated and kept from drifting, in a few sentences.
- Deviations from this brief, and why.
- Suggested follow-ups.
