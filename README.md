<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 madearga -->

# Bible reader

A native macOS Bible reader. It opens a window, reads the whole Bible, searches
it as you type, and keeps your highlights, notes, and bookmarks on your own
machine. There is no account, no server, and no network call anywhere in it.

Seven public-domain English translations are compiled into the binary, so the
app has scripture on a machine that has never installed anything.

This is a development build. There is no release, no notarized build, and no
signed installer.

## What it does

- Seven public-domain English translations are compiled into the binary. They
  are the King James Version (`kjv`), the World English Bible (`web`), the
  Berean Standard Bible (`bsb`), the American Standard Version (`asv`), Young's
  Literal Translation (`ylt`), the Darby Bible (`darby`), and the Webster Bible
  (`webster`). All seven are readable and searchable.
- The reader scrolls continuously through the whole Bible. Each chapter is laid
  out once and kept in a layout cache, so scrolling from Genesis to Revelation
  costs the same per frame as scrolling inside one psalm.
- A jump palette opens on command-K, a search panel on command-F, and a book
  and chapter picker on command-B.
- Highlights in six colours, notes, and bookmarks live in one SQLite file in the
  data folder. Every record is keyed by a reference in the canonical verse grid,
  so a mark follows the reader from one translation to another.
- A study sidebar lists every highlight, note, and bookmark, and a verse that
  carries one shows a margin mark beside it.
- A verse menu offers highlight, note, bookmark, copy, copy with the reference,
  and compare across translations.
- Three themes, light, sepia, and dark, and an adjustable text size, line
  spacing, line length, justification, verse numbers, section headings,
  paragraph mode, and words of Christ in red. Every one of those settings is
  written to the study database and restored on the next launch.
- A Settings window, reachable from the macOS menu bar, edits the theme and the
  typography.
- A native macOS menu bar, built through AppKit. It has no Edit menu, so the
  copy and paste keys still belong to the text fields.
- A parallel view that shows two or three translations side by side, verse
  aligned, with a translation picker on each column.
- Export of the study data to JSON and Markdown, and import of that JSON back.
  The window and the command line read and write the same SQLite database.
- A command line with the `read`, `search`, `import`, `translations`, `export`,
  and `import-study` verbs.
- A macOS application bundle built by `scripts/bundle-macos.sh` and checked by
  `scripts/verify-distribution.sh`.

## Build the app bundle

The bundle script builds the release binary and lays out a double-clickable
application bundle.

```text
git clone https://github.com/madearga/bible-rs-rs.git
cd bible-rs-rs
./scripts/bundle-macos.sh
```

The script needs a Rust toolchain and the Xcode command line tools. It writes
`dist/Bible.app` and installs nothing anywhere. Run it again after changing the
code and the bundle is rebuilt from scratch.

The script honours `CARGO_TARGET_DIR` and `CARGO_BUILD_TARGET`. It selects the
executable path Cargo reports for the build rather than assuming
`target/release/bible`, so a target-specific output directory is handled and a
stale binary from an earlier host build is not picked up.

The bundle's `LSMinimumSystemVersion` is read from the executable's own Mach-O
load commands and checked against them by the verifier. This build is a
host-only `arm64` binary; it is not a universal binary, and it does not promise
macOS 13 or any other version the binary does not declare.

### Open the bundle

```text
open dist/Bible.app
```

The bundle is signed ad-hoc, which is a local placeholder rather than a
developer identity, and it is not notarized. macOS Gatekeeper does not
recognise an ad-hoc signature as a trusted one, so if the bundle arrives with
a quarantine attribute, the first launch is refused. There is no guaranteed
Finder shortcut that bypasses this on current macOS. On macOS Sequoia (15), the
documented route is **System Settings > Privacy & Security**, scroll to
**Security**, and click **Open Anyway** for the blocked app, then authenticate.
macOS may still refuse the app, and a later OS update may change the flow.

The bundle does not remove the quarantine attribute, disable Gatekeeper, or
change any security setting. A bundle you built yourself on this machine has no
quarantine attribute, so `open` starts it without a prompt.

You can also run the binary inside the bundle directly:

```text
dist/Bible.app/Contents/MacOS/Bible
```

### Check the bundle

The verification script is the artifact a reviewer reruns. It reads the bundle
and never writes to it.

```text
./scripts/verify-distribution.sh
```

It checks the bundle layout, lints the `Info.plist` with `plutil`, reads the
binary's architecture with `file` and `lipo`, verifies the signature with
`codesign`, and confirms that each bundled notice is byte for byte the file in
the repository. It also hashes the executable as it sits in the signed bundle
and compares it with `dist/build-info.postsign.txt`, which the build writes
after signing; a swapped executable that was re-signed under the old manifest
does not match that record. The in-bundle `Contents/Resources/build-info.txt`
stays sealed by the signature. The verifier checks that the plist minimum macOS
version matches the Mach-O load command, and that every text in `LICENSES/` is
in the bundle.

Pass `--launch` to also run the bundled binary and read a verse out of it:

```text
./scripts/verify-distribution.sh --launch
```

The launch check points `BIBLE_DATA_DIR` at a fresh temporary directory, so it
never touches your own study data.

## Run from the source tree

You do not need the bundle to use the app.

```text
cargo run --release -p bible-app
```

The release profile is what the performance budget is measured against, so
use it for anything you intend to time.

## The data directory

The app reads and writes one folder. Set `BIBLE_DATA_DIR` to choose where it
is, which is how the tests keep their data out of yours.

| Platform | Folder |
|---|---|
| macOS | `~/Library/Application Support/BibleReader` |
| Linux and other Unix | `$XDG_DATA_HOME/bible-reader`, or `~/.local/share/bible-reader` |

The folder holds the study database `study.db` together with its SQLite side
files, the translations you install yourself under `translations/`, and the
exports the window writes there. Deleting the folder resets the app. It never
leaves your machine.

The `export` and `import-study` verbs open the same SQLite database the window
uses, so an export taken from the command line carries the same records as the
window's own **Export** action. The window writes `study.md` or `study.json`
into the data folder, and the command line writes to the path you give it.

## The command line

The same binary is a command line tool. Give it a verb and it prints or
writes, with no window.

```text
Bible read "John 3:16"
Bible read "John 3" --translation web
Bible search "faith" --translation asv --limit 20
Bible translations
Bible import ./my-translation.bbl
Bible export ./study.json --format json
Bible import-study ./study.json
```

| Verb | What it does |
|---|---|
| `read <reference>` | Prints a verse, or a whole chapter when the reference names no verse |
| `search <query>` | Prints the verses that match |
| `import <path>` | Installs a translation file. `--id <id>` names a plain text import |
| `translations` | Lists the bundled translations and the installed ones |
| `export <path>` | Writes the study data out as JSON or Markdown |
| `import-study <path>` | Reads exported study data back |

`--translation <id>` picks the translation for `read` and `search`. The ids
are `kjv`, `web`, `bsb`, `asv`, `ylt`, `darby`, and `webster`.

The exit code is zero when the verb succeeded, one when it failed, and two
when the arguments did not name a verb.

The `import` verb reads USFM, OSIS, Zefania, and SWORD source files, the
app's own binary format, and plain text in the `Reference<TAB>text` form. It
detects the format from the file, and the metadata flags name the translation
it installs. The core importers keep their archive bounds, so a crafted file
is refused rather than unpacked.

## Keyboard

The reader answers to the keyboard.

| Shortcut | Action |
|---|---|
| command-K | Jump to a reference |
| command-F | Search |
| command-B | Book and chapter picker |
| command-1 to command-9 | Switch to the nth translation |
| command-[ | Back |
| command-] | Forward |
| option-left | Back |
| option-right | Forward |
| left | Previous chapter |
| right | Next chapter |
| 1 to 6 | Highlight the selected verse in that colour |
| 0 | Clear the highlight |
| N | Write a note |
| B | Toggle a bookmark |
| command-D | Toggle a bookmark |
| command-C | Copy the selected verse |
| shift-command-C | Copy the selected verse with its reference |
| C | Compare the selected verse across translations |
| E | Export the study data to Markdown |
| shift-command-E | Export the study data to JSON |
| command-R | Toggle the words of Christ in red |
| shift-command-L | Cycle the theme |
| command-plus | Larger text |
| command-minus | Smaller text |
| command-0 | Default text size |

The table above is the one in `crates/app/src/shortcuts.rs`. The verse numbers,
the section headings, the Settings window, and the shortcut overlay have no key
and are reachable from the menu bar. The parallel view is toggled from the
**Parallel** button in the toolbar, and the study sidebar from the **Study**
button beside it.

The themes are light, sepia, and dark. Set `BIBLE_THEME` to `light`, `sepia`, or
`dark` to pick the one the window opens with.

The translation shortcuts number the installed translations first, so with
nothing installed they number the bundled ones. `BIBLE_REFERENCE` sets the
reference the window opens at, and `BIBLE_RENDERER` picks `wgpu` or `glow`.

## Offline behaviour

The app makes no network request. Everything it needs is in the binary or in
the data directory.

- The seven translations are compiled into the binary and decoded the first
  time you open one.
- The chapter layout is cached per chapter, size, and mode, so a chapter that has
  been laid out once is painted from the cache until the size or the typography
  changes.
- The search corpus is built in memory when you search. Nothing is written to
  disk and no index file is kept.
- Your study data is a SQLite file in the data directory. It is never
  synchronised anywhere.

## What this build does not have

This list is accurate as of the commit that ships this README. It is not a
roadmap.

- **No online translations.** The app does not fetch a translation from an
  online API, and there is no code path that tries.
- **No iOS, Android, or Windows build.** `scripts/bundle-macos.sh` builds a
  macOS application bundle and refuses to run anywhere else.
- **No notarized or signed release.** The bundle carries an ad-hoc signature
  so macOS will run it locally. There is no developer identity, no
  notarization, and no notarized download.
- **The SWORD importer does not decipher encrypted modules.** It reads the
  zText and RawText drivers and the OSIS, ThML, and GBF markup dialects, and it
  refuses a locked module with a message rather than reading it as garbage.
- **The SWORD importer maps the NRSV versification approximately.** It walks
  the canonical grid by chapter and verse count, so a module whose NRSV
  numbering differs from the KJV grid lands in the wrong place for the books
  where the two disagree. Treat an NRSV-versified import as approximate.
- **No application icon.** The bundle falls back to the system's generic
  application icon.
- **Study data is local to one machine.** The SQLite file is not synced,
  exported to a service, or shared between machines by the app.

## Layout of the repository

| Path | What is in it |
|---|---|
| `crates/core` | The translation model, the importers, the search, and the study store |
| `crates/app` | The window, the themes, the render harness, and the command line |
| `xtask` | The build tasks, including the data build |
| `data/translations` | The seven compiled translation files |
| `assets/fonts` | The Literata and Inter faces and their licences |
| `docs/SPEC.md` | The program spec the implementation follows |
| `scripts` | The bundle script and the verification script |

## Licence

The program is MIT licensed. The licence text is in `LICENSE`.

The bundled translations, the bundled fonts, and the Rust dependencies carry
their own licences. `THIRD_PARTY.md` names each one, where it came from, and
which licence covers it. The bundle copies that file and the font licences
into `Bible.app/Contents/Resources/`.
