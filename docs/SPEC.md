# Bible reader program spec

The requirements for a clean-room rewrite of a fast native Bible reader for macOS.
Read this before writing code. It states what the program builds and how each piece must behave.

This spec was written for a rewrite whose reference implementation is at
`/Users/madearga/bible-rs-reference`. That tree is read-only. Read it for behaviors, data
formats, and algorithms. Never copy its source, comments, doc text, or distinctive internal
naming. Write your own implementation. Every file carries an MIT SPDX header naming
`madearga` as the copyright holder.

## What the program is

A macOS app that opens instantly, reads the whole Bible, searches it as you type, and keeps
your highlights, notes, and bookmarks on your own machine with no account and no network.

The performance brief is the reason the program exists. A native GUI toolkit that renders
its own text is required. A web view cannot meet the budget.

## The canonical grid

Everything is keyed to one canonical grid. 66 books, 1189 chapters, 31102 verses.

A reference to a verse is a triple of book index, chapter number, and verse number. The book
index is the position of the book in the canonical order, starting with Genesis at 0 and
ending with Revelation at 65. Chapter and verse numbers are 1-based.

The verse count per chapter is fixed by the KJV versification and is the same for every
translation, even where a translation prints no text for some verse. Deuterocanonical books
are out of scope, so the grid stays at 66 books.

Every translation, whether bundled, imported by the user, or fetched from an online API,
converts to the same structure. That single grid is what makes verse-aligned parallel columns,
the compare view, and translation-independent highlights possible.

## The translation model

A translation is one UTF-8 string holding all its verse text, plus three flat arrays.

- **Verses.** Per verse: the verse number, the byte range of its text in the string, and the
  ranges into the marks and spans arrays.
- **Marks.** Structural breaks inside or before a verse: paragraph start, poetry line, poetry
  indent level, stanza break, heading, Psalm superscription.
- **Spans.** Character styles inside a verse: words of Christ, translator additions, the divine
  name, Selah.

Offsets inside a verse are 16-bit byte offsets relative to the verse start.

The metadata sits alongside: id, abbreviation, full name, language, licence summary,
copyright notice, provenance, origin, per-book titles, and flags for red letters, paragraphs,
and headings.

The design intent is that loading a translation is a handful of linear passes with no
per-verse allocation, and that rendering a chapter allocates nothing per verse.

## The binary translation format

Translations are stored as column arrays compressed with zstd. The format starts with the
four ASCII bytes `BIBL`, then a version byte, then the metadata as an uncompressed JSON
header, then the compressed column data. The metadata header is uncompressed so that listing
the installed translations never decompresses anything.

The compressed KJV is about 1 MB and all seven bundled translations together are about 7 MB.
Decoding a whole Bible takes about 18 ms and happens on a background thread while the window
opens. Other translations decode lazily so switching stays instant.

The bundled translations are embedded in the binary as compressed bytes and decoded on first
use, so startup only pays for the one being read.

## Building the bundled data

Seven public-domain translations ship with the app: KJV, WEB, BSB, ASV, YLT, Darby, and
Webster. Their text comes from the eBible.org USFM archives. The download URL and the
SHA-256 digest of every archive are pinned in the repository, and the build verifies each
digest before it uses the file.

The build tool is an `xtask` binary run as `cargo run -p xtask -- build-data`. It downloads
the archives, verifies the digests, runs the same USFM importer that users get when they
import a file, converts each translation to the binary format, and writes the result to
`data/translations/<id>.bbl`. The raw USFM is never committed. Only the derived binary
format is.

Do not bundle any copyrighted translation text. Not in the repository, not in a test
fixture, not in a search-index fixture. Use public-domain words arranged in the shape of
each source format for tests.

## Importing a translation the user owns

The user can import a file they own. Four formats are accepted.

- **USFM.** A `.usfm` or `.sfm` file, a folder of them, or a zip of them.
- **OSIS XML.** The `.xml` or `.osis` file.
- **Zefania XML.** The `.xml` file.
- **SWORD module.** The module zip, its unpacked folder, or its `mods.d/*.conf` file. The
  zText and RawText drivers in the KJV, KJVA, NRSV, and NRSVA versifications are supported,
  along with the OSIS, ThML, and GBF markup dialects. A locked module needs the user's unlock
  key.

A converted translation lives only in the user's data folder, never in the repository.

The importer must not be a path-traversal or zip-bomb hazard. It must reject a malformed
archive with a clear message rather than panicking.

## Search

There is no inverted index. Each translation lazily builds a normalized corpus in memory:
lower-cased, diacritics folded, apostrophes joined, punctuation turned into spaces. The
corpus is laid out as one verse per space-wrapped line, so a verse's text is surrounded by
spaces.

A search term is a SIMD substring scan for the term with spaces around it. A phrase is a
longer needle. Because the corpus wraps every verse in spaces and a needle never contains a
double space, a match can never straddle two verses. A hit maps to its verse with a binary
search, and the scan then jumps to the next verse. A multi-term query scans the longest and
rarest term first and filters the candidates.

The budget is 0.15 ms to 1.8 ms across the KJV, and 10 ms is the hard ceiling. There is no
index to build at import time and nothing to store on disk.

The query language supports these forms.

- Plain words, where every word must appear.
- A word prefix, which is the default, or whole words only.
- `"an exact phrase"`.
- `-excluded` words.
- A scope: the whole Bible, the Old Testament, the New Testament, or one book.

The result list shows the matching words highlighted.

## Study data

One SQLite file holds highlights, notes, bookmarks, and settings. It runs in WAL mode and is
mirrored in memory, so the reader can query it per verse every frame without touching the
disk.

Highlights, notes, and bookmarks are keyed by the canonical reference and not by the
translation, so they follow the reader across translations.

Six highlight colours are available and a verse can have none. A note has a body. A bookmark
has an optional label. Settings include the reading position and the appearance
preferences, and they are written at most every 1.5 s so scrolling never waits on the disk.

Export goes to JSON, which round-trips back through import, and to Markdown.

## The reader

Text is laid out into a continuous scroll across the whole Bible. Only the chapters on
screen are laid out, and the layout of each chapter is cached. The scroll position is an
anchor of a chapter plus an offset inside it, not a position in one enormous document, so
scrolling from Genesis to Revelation costs the same per frame as scrolling inside one psalm.
A window resize or a font change re-anchors to the verse being read.

Two paragraph modes exist. In paragraph mode the verses flow inside a paragraph and the first
line is indented, except right after a heading. In verse-per-line mode every verse gets its
own line with the number in the margin. Poetry keeps its line structure in both modes, with
a level indent and a hanging indent for wrapped lines, a stanza break adding space, and
Selah right-aligned.

The typesetting rules are these. The body text is a serif face, and the interface and the
verse numbers use a sans face. The measure is capped at about 34 em, so roughly 65 to 75
characters per line. The line height is 1.62. The verse number is a superscript followed by
a non-breaking space, so a number never sits at the end of a line away from its verse. The
divine name is set in small capitals, which the toolkit has no toggle for, so it is set in
capitals at 78 percent size with the line height adjusted to match the baseline. Words of
Christ can be red. Translator additions can be italic. Psalm superscriptions are muted
italic, and section headings are semibold.

Three themes exist: light, sepia, and dark. They are chosen for long reading, so warm paper
rather than pure white, and not pure black either.

Text size, line spacing, line length, justification, whether verse numbers and headings
show, and the theme are all adjustable.

## Navigation

A jump palette opens on the keyboard shortcut for command-K. It accepts a reference with a
fuzzy book name and tolerant of typos, so `jn 3:16`, `Rom 8`, `1co13`, and even
`revalation 21` all resolve. It also offers recent places, the installed translations, and
the app's commands.

A sidebar on the left holds a Books section and the study list, and opens or closes on the
keyboard shortcut for command-B, from the View menu, and from the toolbar. The Books section
shows the two testaments and their books, a book row expands into a grid of chapter chips,
and a dot sits under every chapter that carries a study mark. Opening the sidebar reveals
the chapter the reader is in, a filter field narrows the books by name, and the sidebar
slides open and closed, instantly for a reader who has asked the system to reduce motion.
Whether it was open is remembered with the other settings. The floating book and chapter
picker is still reachable from the toolbar. Back and forward history work. Previous and
next chapter work.

The app remembers where the reader was.

## Search in the app

A search panel opens on the keyboard shortcut for command-F. Results update as the user
types. The arrow keys move through the results and return jumps to the verse, which flashes
so the reader can see where the match is.

## Study in the app

Clicking a verse selects it. Shift-clicking extends the selection to a range, and the J and
K keys move the selection. A selected verse can be highlighted in one of six colours, given
a note, bookmarked, copied, or compared across translations. The context menu offers all of
these too.

Notes and bookmarks show in the margin. The study list in the sidebar shows every highlight,
note, and bookmark in canonical order, each with its verse text under it. A highlight row
carries its colour as a bar, a pill row narrows the highlights to one colour, and a row's
context menu removes the mark the row came from.

## Keyboard

These are the shortcuts the app uses.

| Shortcut | Action |
|---|---|
| command-K | Jump palette |
| command-F | Search |
| command-B | Toggle the sidebar |
| command-1 to command-9 | Switch to the nth translation |
| command-[ and command-] | Back and forward |
| option-left and option-right | Back and forward |
| left and right | Previous and next chapter |
| 1 to 6 | Highlight the selected verse in that colour |
| 0 | Clear the highlight |
| N | Write a note |
| B | Bookmark |
| command-D | Bookmark |
| command-C | Copy the selected verse |
| shift-command-C | Copy the selected verse with its reference |
| C | Compare the selected verse across translations |
| E | Export the study data to Markdown |
| shift-command-E | Export the study data to JSON |
| command-R | Toggle the words of Christ in red |
| command-shift-L | Cycle the theme |
| command-plus, command-minus, command-0 | Text size |

The reference implementation has a keyboard shortcut overlay, which shows the whole list.

## macOS integration

The app has a native menu bar. Menu items with key equivalents route into the same actions
as the keyboard shortcuts. There is deliberately no Edit menu, because the system's
predefined copy and paste items swallow command-C and command-V before the text fields see
them.

The window uses the unified title bar style.

The renderer is wgpu on Metal on macOS, and OpenGL elsewhere. The choice is made by
measurement. On macOS the OpenGL path is deprecated and layered over Metal, and it held
about 15 frames per second while scrolling where Metal held the display's 60. Under a
software OpenGL context on Linux the OpenGL path creates its context about twice as fast.
wgpu asks for the low-power GPU, because a reader has no use for a discrete GPU and waking
one costs start-up time and battery. An environment variable overrides the choice.

The app stores its data in the platform data directory, and the location is overridable
through an environment variable so a test can point it at a temporary directory.

## The offline render harness

The GUI is verified through an offscreen render harness. It is an example binary in the
app crate that drives the real app through the toolkit's test harness and writes a PNG, so
it works on a build machine with no display and while a real window is open.

The harness takes the output path and the size, and the environment selects what to show.
It must support at least these.

- Showing a given reference.
- Clicking at a point, and hovering at a point, so states only the pointer can reach are
  reachable.
- A pause between frames, so content that arrives asynchronously is on screen before the
  picture is taken.
- A filmstrip mode that writes the next several frames, so an animation can be inspected
  frame by frame.

The harness is the control skill for the GUI. A GUI change is not verified without a PNG
that came out of it.

## The CLI

The app has a command-line interface for scripting and for the smoke test in CI. It needs
these verbs.

- `read` prints a reference or a chapter.
- `search` runs a query and prints the matches.
- `import` converts a translation file into the binary format.
- `translations` lists what is installed.
- `export` writes the study data out.
- `import-study` reads it back.

The CLI is the control skill for the core crate. A core change is verified by running the
release binary with real arguments and asserting on its output and its exit code.

## The performance budget

These numbers are the budget, measured on the CI runner and on the reference hardware.

| Metric | Budget |
|---|---|
| First frame after the window exists | Under 50 ms |
| Full-Bible search | Under 10 ms |
| Scroll CPU per frame | Under 1 ms |
| Load and decode one translation | Under 50 ms |

A change that regresses a budgeted metric fails on the perf box even when it is
functionally correct.

## The build

The workspace has these members.

- `crates/core`. The translation model, the importers, the search, and the study store. No
  GUI dependency.
- `crates/app`. The reader, the GUI, and the CLI.
- `xtask`. The build tasks, including the data build.

CI runs on Linux and on macOS. On Linux it installs the system packages, checks the
formatting, runs clippy with warnings denied, runs the tests, runs the search smoke test,
and runs the benchmarks. The macOS job builds the app bundle and runs the startup probe.

Every pull request must pass the formatting check, the clippy check with warnings denied,
and the whole test suite.
