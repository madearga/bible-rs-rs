# Bible reader

A native macOS Bible reader. It opens a window, reads the whole Bible, searches
it as you type, and keeps your highlights, notes, and bookmarks on your own
machine. There is no account, no server, and no network call anywhere in it.

Seven public-domain English translations are compiled into the binary, so the
app has scripture on a machine that has never installed anything.

This is a development build. There is no release, no notarized build, and no
signed installer.

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

The script honours `CARGO_TARGET_DIR` if you set it, and it reads the release
binary back out of the same directory cargo put it in.

### Open the bundle

```text
open dist/Bible.app
```

The bundle is signed ad-hoc, which is a local placeholder rather than a
developer identity, and it is not notarized. The first time you open it, macOS
refuses it. Right-click the bundle in Finder and choose **Open**, then confirm.
After that it opens normally.

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
the repository.

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

The folder holds the study database `study.db`, the translations you install
yourself under `translations/`, and nothing else. Deleting the folder resets
the app. It never leaves your machine.

The `export` and `import-study` verbs work on a JSON file rather than on the
SQLite database, so an export taken from the window and an export taken from
the command line do not see the same data. The window's own **Export** action
writes `study.md` or `study.json` into the data folder from the live store, so
use that one when you want the window's data.

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
| `import <path>` | Installs a translation file |
| `translations` | Lists the bundled translations and the installed ones |
| `export <path>` | Writes the study data out as JSON or Markdown |
| `import-study <path>` | Reads exported study data back |

`--translation <id>` picks the translation for `read` and `search`. The ids
are `kjv`, `web`, `bsb`, `asv`, `ylt`, `darby`, and `webster`.

The exit code is zero when the verb succeeded, one when it failed, and two
when the arguments did not name a verb.

The `import` verb reads the app's own binary format and plain text in the
`Reference<TAB>text` form. It does not read USFM, OSIS, Zefania, or SWORD
files. The core crate has importers for those four formats, and they are
covered by the core test suite, but the application does not yet call them.

## Keyboard

The reader answers to the keyboard.

| Shortcut | Action |
|---|---|
| command-K | Jump to a reference |
| command-F | Search |
| command-B | Book and chapter picker |
| command-1 to command-9 | Switch to the nth translation |
| command-[ and command-] | Back and forward |
| option-left and option-right | Back and forward |
| left and right | Previous and next chapter |
| 1 to 6 | Highlight the selected verse in that colour |
| 0 | Clear the highlight |
| N | Write a note |
| B or command-D | Bookmark |
| command-C | Copy the selected verse |
| shift-command-C | Copy the selected verse with its reference |
| C | Compare the selected verse across translations |
| E | Export the study data to Markdown |
| shift-command-E | Export the study data to JSON |
| command-R | Toggle the words of Christ in red |
| command-shift-L | Cycle the theme |
| command-plus, command-minus, command-0 | Text size |

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
- The search corpus is built in memory when you search. Nothing is written to
  disk and no index file is kept.
- Your study data is a SQLite file in the data directory. It is never
  synchronised anywhere.

## What this build does not have

This list is accurate as of the commit that ships this README. It is not a
roadmap.

- **No online translations.** The app does not fetch a translation from an
  online API, and there is no code path that tries.
- **No iOS or Android build.** The bundle script builds a macOS application
  bundle and refuses to run anywhere else.
- **No notarized or signed release.** The bundle carries an ad-hoc signature
  so macOS will run it locally. There is no developer identity, no
  notarization, and no notarized download.
- **No native menu bar, study sidebar, or parallel translations view.** These
  are in flight in a separate branch and are not in this build.
- **The SWORD importer does not decipher encrypted modules.** It reads the
  zText and RawText drivers and the OSIS, ThML, and GBF markup dialects, and it
  refuses a locked module with a message rather than reading it as garbage.
- **The SWORD importer maps the NRSV versification approximately.** It walks
  the canonical grid by chapter and verse count, so a module whose NRSV
  numbering differs from the KJV grid lands in the wrong place for the books
  where the two disagree. Treat an NRSV-versified import as approximate.
- **The application never calls the core importers.** The USFM, OSIS, Zefania,
  and SWORD importers exist in `crates/core` and are tested there. The
  application has no route to them yet, so the `import` verb handles only the
  app's own binary format and plain text.
- **No preference panel.** The theme, the text size, and the reading position
  are all stored in the study database, but only the reading position is read
  back on launch. The theme and the text size come from the environment
  variables `BIBLE_THEME` and the defaults, and there is no window for changing
  them yet.
- **No application icon.** The bundle falls back to the system's generic
  application icon.

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
