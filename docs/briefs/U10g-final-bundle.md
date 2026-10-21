GOAL
Rebuild the macOS bundle on the final main, and make the README describe what the app actually
does now, so a reader of the repository is not told the app lacks features it has.

SCOPE
You may write only inside the new repository. Create or edit:
  README.md                     the accuracy pass
  THIRD_PARTY.md                only if a shipped notice changed
  scripts/                      only if a check needs to change
  data/README.md                only if the data set changed
Do not touch crates, xtask, assets, docs, Cargo.toml, Cargo.lock, or the .bbl files.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements are
in docs/SPEC.md. Read it, then read the README section "What this build does not have" and check
every claim against the code on this branch.

The app now has all of these, and the README must say so plainly:
- Seven public-domain translations compiled into the binary, readable and searchable.
- Continuous scrolling through the whole Bible with a per-chapter layout cache.
- A jump palette on command-K, a search panel on command-F, a book picker on command-B.
- Highlights in six colours, notes, and bookmarks, stored in one SQLite file, keyed by the
  canonical verse so they follow the reader across translations.
- A study sidebar that lists the highlights, notes, and bookmarks, and margin marks on the verses
  that carry them.
- A verse menu offering highlight, note, bookmark, copy, copy with the reference, and compare.
- Three themes, adjustable text size, line spacing, line length, justification, verse numbers,
  section headings, and words of Christ in red, all persisted across restarts.
- A Settings window reachable from the macOS menu bar.
- A native macOS menu bar.
- A parallel view showing two or more translations side by side, verse aligned, with a per-column
  translation picker.
- Export of the study data to JSON and Markdown, and import of that JSON back, through the window
  and through the command line, all reading and writing the same SQLite database.
- A command line with read, search, import, translations, export, and import-study.
- A macOS application bundle built by scripts/bundle-macos.sh, with a verification script.

The README must still be honest about what is missing. These are real and stay listed:
- No online translations. The app never makes a network request.
- No iOS or Android build, and no Windows build.
- No notarized or signed release. The bundle carries an ad-hoc signature only.
- The SWORD importer does not decipher encrypted modules.
- The SWORD importer maps the NRSV versification approximately.
- The application does not call the core importers for USFM, OSIS, Zefania, or SWORD files; the
  import verb handles the app's own binary format and plain text only. Verify this against the
  code before writing it, because it may have changed.
- No application icon.
- Study data is local to one machine and is not synced anywhere.

Check every one of the claims you write against the code. Do not write a feature the code does not
have. If you find a claim in the current README that is now false, correct it rather than adding
to the list.

The build. The bundle must be built on the final main and must pass the verification script with
all checks. The bundle script and the verification script are already written and reviewed; change
them only if a real defect shows up, and say so in the report.

ACCEPTANCE
- cargo build succeeds with no warnings.
- The bundle is built from the final main and scripts/verify-distribution.sh --launch passes every
  check with a zero exit.
- The README's feature list matches the code, with no claim the code does not support and no
  missing mention of a feature the code has.
- The README's "What this build does not have" section still lists every real limitation.
- The README's keyboard table matches the SHORTCUTS table in crates/app/src/shortcuts.rs.
- The README's command line table matches the verbs in crates/app/src/cli.rs.
- Every file you write starts with the SPDX header naming madearga.
- No em dashes, no mid-sentence colons, and no AI filler anywhere in the prose you write.

VERIFY
Run exactly these and paste the outputs into your report:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    ./scripts/bundle-macos.sh
    ./scripts/verify-distribution.sh --launch
    cargo run --release -p bible-app -- translations
    cargo run --release -p bible-app -- read "john 3:16"
    cargo run --release -p bible-app -- search "sheep" --limit 3
Report the bundle path, the check count, and the CLI outputs. Then open the bundle and confirm
the window appears and the menu bar is present:
    open dist/Bible.app
Wait a few seconds, then list the menu bar titles of the running app with AppleScript and report
them, then quit the app with command-Q. If AppleScript cannot do it, say so plainly.

TIMEBOX
Three hours of work. Work autonomously; the operator is away. Do not ask questions. If a decision
is genuinely a product choice, take the simplest option that matches SPEC.md and record it under
BLOCKED-ITEMS in the report.

FORBIDDEN
No source or naming from /Users/madearga/bible-rs-reference. No SPDX headers other than MIT with
madearga. No weakening tests, no #[ignore], no clippy suppressions, no budget changes, no new
dependencies. No force-push, rebase, or merging. No edits outside SCOPE. Never touch the reader's
real data folder; use a temporary BIBLE_DATA_DIR for anything that writes.

REPORT
Reply with: status, branch, head SHA, PR URL, verify outputs, the bundle path and its size, the
menu bar titles AppleScript reported, the README changes as a short list, what the CLI printed,
limitations, BLOCKED-ITEMS.
