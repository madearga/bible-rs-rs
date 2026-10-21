GOAL
Close the seven must-fix findings of the security and licence audit lane, so a hostile file
cannot exhaust memory, escape the data folder, or leave the distribution without the notices its
dependencies require.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/core/src/import/zip.rs      archive limits and link-safe extraction
  crates/core/src/import/sword.rs    bounded decompression and verse budgets
  crates/core/src/import/mod.rs      bounded source reads
  crates/core/src/import/osis.rs     bounded nesting
  crates/core/src/import/zefania.rs  bounded nesting
  crates/core/src/format.rs          bounded decode and validated reservations
  crates/core/src/study/mod.rs       the lock identity
  crates/core/src/study/export.rs    settings value validation
  crates/app/src/lib.rs              the installed translation identifier
  crates/app/src/cli.rs              the import route into the installation folder
  crates/app/tests/import_security.rs  the tests
  crates/core/tests/import_security.rs  the tests
  scripts/collect-licenses.py       the notice check
  LICENSES/                          the regenerated notices
Do not touch docs, README.md, data/*.bbl, assets, xtask, or Cargo.toml and Cargo.lock. The
dependency set does not change in this unit, so the lock file must not change either.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements are
in docs/SPEC.md. Read the importer and study sections.

An independent read-only audit of the merged code found the findings below. Each one is a real
defect, not a style note. The first three are the ones that matter most, because a small hostile
file can turn into gigabytes of memory use.

1. Installed translation identifiers escape the installation folder.
   crates/app/src/lib.rs joins the identifier from a translation header onto the translations
   folder and writes it. An identifier of "../escaped" or an absolute path writes outside the data
   folder, and an existing destination symlink is followed. Accept only a bounded filename-safe
   identifier, ASCII letters, digits, underscore and hyphen, and write through a securely created
   sibling temporary file so an existing link is never followed.

2. SWORD inner decompression bypasses the archive limits.
   crates/core/src/import/sword.rs decompresses each zText block with an unrestricted read and
   keeps every decoded block, and the block table's declared size is ignored. Enforce a hard
   per-block limit and a hard aggregate decoded-byte budget while decompressing, and do not trust
   the table's declared size.

3. SWORD verse records amplify a small input into enormous allocations.
   The same file allocates a string per index record, including repeated and unused ranges, so a
   small index can request gigabytes. Bound the record count against the supported versification
   and enforce an aggregate decoded verse text budget before each allocation.

4. A hostile .bbl file exhausts memory before validation.
   crates/core/src/format.rs decodes the body without an output limit, and reserves vectors from
   unchecked counts before the records are known to exist. Cap the metadata and the expanded body,
   decompress with a hard output limit, require the canonical verse count early, and validate the
   mark and span counts against the remaining encoded bytes before reserving.

5. Extraction can write outside its root through links.
   crates/core/src/import/zip.rs follows an existing leaf symlink or hard link, and creates parent
   directories before checking where they resolve. Validate the ancestors before creating them,
   create the destination with exclusive creation so a link is never followed, and reject archive
   entries that carry symlink metadata.

6. The archive limits do not cover the archive structure.
   The compressed input is read without a size limit, the entry listing has no limit, and the
   directory branch skips the entry count. Bound the compressed input, count every entry including
   directories, and apply the same limits during format detection.

7. The macOS dependency notices are missing.
   muda, crossbeam-channel, crossbeam-utils, and keyboard-types are in the shipped binary and are
   absent from LICENSES/. Regenerate the notices for the current runtime closure and change
   scripts/collect-licenses.py --check so it compares the current dependency identities against the
   index, instead of only checking that the files the index names are present. A missing notice
   must fail the check.

The audit also reported, as warnings to fix in the same unit where they are cheap:
- SWORD module file reads accept absolute and parent-traversing DataPath values and are not
  confined to the module root. Reject non-relative paths and verify canonical containment.
- Plain text import and parser allocations are unbounded. Bound the source bytes, the file count,
  the marker nesting, the accumulated verse text, and the apparatus count.
- The study lock is derived from the caller's spelling, so two names for one database take two
  locks. Resolve the database path before deriving the sidecar lock.
- Imported settings accept negative and extreme typography values. Validate them against the
  ranges the interface supports.
- Malformed SWORD offsets are clamped instead of reported. Reject ranges outside their block.

The clean-room wording in THIRD_PARTY.md overstates what the repository can prove, because
docs/SPEC.md explicitly allowed reading the reference implementation's algorithms. That file is
not in scope here, so report the exact wording to change and the coordinator will do it.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- A test feeds a translation whose id is "../escaped" and one that is absolute, and asserts the
  import is refused and nothing is written outside the data folder.
- A test feeds a zip whose entry name is a symlink and asserts it is refused.
- A test feeds a zip whose entry would be written through an existing symlink in the destination
  and asserts the link target is untouched.
- A test builds a zText module whose block declares a small size but decompresses to far more,
  and asserts the import is refused with a clear message and without a large allocation.
- A test builds a .bbl with a huge declared body length and a small file, and asserts the decode
  is refused rather than allocating.
- A test feeds a settings document with a negative or absurd text size and asserts it is refused.
- A test asserts the settings document without the new field still loads with the default.
- A test asserts the zip entry count limit counts directory entries too.
- The collector --check fails when a shipped dependency's notice is missing, and passes on the
  regenerated set. Prove the failure with a fixture, not by editing the real folder.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file you create starts with the SPDX header naming madearga.
- The tests use temp folders and never touch the reader's real data folder.

VERIFY
Run exactly these and paste the outputs into your report:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    python3 scripts/collect-licenses.py --check
    git diff --stat Cargo.lock
The last one must show no change, because no dependency changed.

Do not run the release benchmark suite; the coordinator measures budgets on final main.

TIMEBOX
Six hours of work. Work autonomously; the operator is away. Do not ask questions. If a decision
is genuinely a product choice, take the smallest safe option and record it under BLOCKED-ITEMS.

FORBIDDEN
No source or naming from /Users/madearga/bible-rs-reference. No SPDX headers other than MIT with
madearga. No weakening tests, no #[ignore], no clippy suppressions, no budget changes, no new
dependencies, no lock file change. No force-push, rebase, or merging. No edits outside SCOPE. Do
not disable a limit to make a test pass.

REPORT
Reply with: status, branch, head SHA, PR URL, verify outputs, one line per finding saying how it
was closed and where the test for it lives, the limits you chose and their values, the exact
THIRD_PARTY.md wording you want changed, limitations, BLOCKED-ITEMS.
