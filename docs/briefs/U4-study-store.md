GOAL
Build the study store: one SQLite file that holds highlights, notes, bookmarks, and
settings, mirrored in memory so the reader can query it per verse every frame without
touching the disk.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/core/src/study/mod.rs     the store
  crates/core/src/study/record.rs  the record types
  crates/core/src/study/export.rs  the JSON and Markdown export
  crates/core/tests/study_*.rs     the tests
  crates/core/Cargo.toml             already declares every dependency this unit needs; do not edit it
Do not touch any other path. In particular do not touch books.rs, model.rs, format.rs, or
reference.rs. Another unit owns those, and your unit builds on the API they define.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read "Study data" before you start.

You build on the model that unit U1 defines. Read U1's merged code for the exact names. A
verse reference is a triple of book index, chapter number, and verse number, keyed to the
canonical 66-book, 1189-chapter grid.

Highlights, notes, and bookmarks are keyed by the canonical reference and not by the
translation, so they follow the reader across translations.

Six highlight colours are available and a verse can have none. A note has a body. A
bookmark has an optional label. Settings include the reading position and the appearance
preferences, and they are written at most every 1.5 s so scrolling never waits on the
disk.

Export goes to JSON, which round-trips back through import, and to Markdown.

The store opens in WAL mode. The data is mirrored in memory, so a read never touches the
disk. Writes go to the database and update the mirror.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- The store test asserts that a highlight can be set, read back, changed to a different
  colour, and cleared.
- The store test asserts that a note can be set and read back, that setting an empty note
  removes it, and that a bookmark can be set, read back, and toggled.
- The store test asserts that the store survives being closed and reopened, and that the
  highlight and the note are still there.
- The store test asserts that a reading position setting can be written and read back.
- The export test asserts that a JSON export of a store with a highlight, a note, and a
  bookmark can be imported into an empty store and comes back equal.
- The export test asserts that the Markdown export contains the reference, the note body,
  and the highlighted verses.
- The export test asserts that importing a malformed JSON file gives an error and does
  not change the store.
- The store test asserts that a write is visible to a read immediately, with no flush
  needed, because the mirror holds it.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header: // SPDX-License-Identifier: MIT
  and a copyright line for madearga.
- The tests use synthetic text only. No Bible text, public domain or not.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

All three must pass with no warnings and no ignored tests. If clippy complains, fix the
code, do not silence the lint.

Then measure and report:

    cargo test --release -- --nocapture store_benchmark

The test must print how long it takes to read the highlight of 1000 verses from the mirror,
and how long a single write takes. The read must be under 1 microsecond per verse. Paste
the output.

TIMEBOX
Five hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not bundle or commit any Bible text, public domain or not.
Do not add a dependency that is not needed. A SQLite driver is the only one this unit
needs beyond what the crate already has.
Do not add a feature flag, a config file, or an abstraction layer that no caller needs.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: what the three verify commands printed, and the benchmark numbers.
- The data shape you chose for the store and its records, in a few sentences. Say how the
  mirror and the database stay in step, and what happens if a write fails.
- The public API, as a short list of the type and function names.
- Deviations from this brief, and why.
- Suggested follow-ups.
