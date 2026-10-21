GOAL
Give the application a route to the importers the core crate already has, so a user can import a
USFM, OSIS, Zefania, or SWORD file into the running app instead of only through the core test
suite.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/src/cli.rs           the import verb routes to the core importers
  crates/app/src/lib.rs           module declaration if you need one
  crates/app/tests/import_cli.rs  the tests
Do not touch crates/core, xtask, data, assets, docs, scripts, Cargo.toml, Cargo.lock.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements are
in docs/SPEC.md, section "Importing a translation the user owns" and the "import" row of "The
CLI". Read them, then read on your branch: crates/app/src/cli.rs (the import verb and the
installed-translation handling), crates/app/src/lib.rs (the Library and the translations folder),
and crates/core/src/import/mod.rs (the importer entry points and ImportFormat::detect).

What exists today. The core crate can import USFM, OSIS, Zefania, and SWORD files and convert them
to the app's own binary format. The application's import verb only handles the app's own binary
format and a plain text form. The reader has no route to the four real importers, which is the gap
this unit closes.

What to build.
- The import verb detects the format of the file it is given, using the core detection, and calls
  the matching core importer.
- A USFM file, a folder of USFM files, or a zip of them is imported.
- An OSIS or Zefania XML file is imported.
- A SWORD module, its unpacked folder, or its mods.d configuration file is imported, and a locked
  module is refused with the reason rather than read as garbage.
- The importer needs the translation's metadata: the id, the abbreviation, the full name, the
  language, the licence, the copyright notice, and the provenance. Take them from flags where the
  user supplies them, and take the book titles from the source. Do not invent a licence statement:
  if the caller does not supply one, say the licence is unknown rather than claiming public domain.
- The result is written into the translations folder as the app's own binary format, so the app
  lists it and can read it afterwards.
- The plain text form the verb already supports keeps working, and the app's own binary format
  keeps working.
- The command must report what it did, and fail with a clear message when the file cannot be
  imported.

The import must not be a hole. A crafted archive, a malformed XML file, or a hostile SWORD module
must produce an error, never a panic and never a write outside the data folder. The core importers
already bound the archive sizes; the application must not bypass those bounds.

ACCEPTANCE
- cargo build succeeds with no warnings on macOS.
- cargo test passes, and the tests cover the cases below.
- A test imports a synthetic USFM file through the real command line binary and asserts the
  translation is installed and readable afterwards with the right verse count.
- A test imports a synthetic OSIS XML file and a synthetic Zefania XML file the same way.
- A test imports a synthetic SWORD module the same way, or asserts a locked module is refused
  with a clear reason.
- A test asserts a malformed file of each kind fails with a non-zero exit and installs nothing.
- A test asserts a crafted archive that tries to escape the data folder is refused and writes
  nothing outside the translations folder.
- A test asserts the existing plain text and binary format imports still work.
- A test asserts the imported translation appears in the translations listing with the metadata
  the caller supplied.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header naming madearga.
- The tests use temp data folders and never touch the reader's real data folder. No Bible text is
  committed: the synthetic files use generated words.

VERIFY
Run exactly these and paste the outputs into your report:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    BIBLE_DATA_DIR=$(mktemp -d) cargo run --release -p bible-app -- import <path> --id demo
    BIBLE_DATA_DIR=$(mktemp -d) cargo run --release -p bible-app -- translations
The second command must list the imported translation.

Do not run the release benchmark suite; the coordinator measures budgets on final main.

TIMEBOX
Five hours of work. Work autonomously; the operator is away. Do not ask questions. If a decision
is genuinely a product choice, take the simplest option that matches SPEC.md and record it under
BLOCKED-ITEMS in the report.

FORBIDDEN
No source or naming from /Users/madearga/bible-rs-reference. No SPDX headers other than MIT with
madearga. No weakening tests, no #[ignore], no clippy suppressions, no budget changes, no new
dependencies. No force-push, rebase, or merging. No edits outside SCOPE.

REPORT
Reply with: status, branch, head SHA, PR URL, verify outputs, the CLI outputs, the formats you
now accept and the flags you added, the safety limits the application enforces, limitations,
BLOCKED-ITEMS.
