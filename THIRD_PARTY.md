# Third-party notices

This file lists the third-party software, fonts, and text the reader ships
with. It is not a legal opinion, and it is not an exhaustive licence audit. It
names what the build actually puts in the application, where each thing came
from, and which licence covers it.

The application bundle copies this file into
`Bible.app/Contents/Resources/THIRD_PARTY.md`, and the licence texts it names
travel beside it in the same folder. `LICENSES/` in the repository and
`Contents/Resources/LICENSES/` in the bundle hold the actual texts, with
`INDEX.md` naming the crate, version, declared licence, and file that covers
each one.

## The bundled translations

The application compiles seven translations into its binary. They are the
public-domain English editions the data pipeline downloads from eBible.org, and
`data/README.md` is the record of where each archive came from and which
SHA-256 digest the build checks before it uses it.

| Id | Name | Edition | Licence | Source |
|---|---|---|---|---|
| `kjv` | King James Version | 1611 | Public domain in most countries, see the UK note below | <https://ebible.org/find/details.php?id=eng-kjv2006> |
| `web` | World English Bible | 2000 | Public domain, see the trademark note below | <https://ebible.org/find/details.php?id=engwebp> |
| `bsb` | Berean Standard Bible | 2020 | Public domain | <https://ebible.org/find/details.php?id=engbsb> |
| `asv` | American Standard Version | 1901 | Public domain | <https://ebible.org/find/details.php?id=eng-asv> |
| `ylt` | Young's Literal Translation | 1862 | Public domain | <https://ebible.org/find/details.php?id=engylt> |
| `darby` | Darby Translation | 1890 | Public domain | <https://ebible.org/find/details.php?id=engDBY> |
| `webster` | Webster Bible | 1833 | Public domain | <https://ebible.org/find/details.php?id=engwebster> |

Each translation carries its own notice in the metadata header inside the
binary, so the notice travels with the text.

### The World English Bible trademark

eBible.org publishes the World English Bible as a public-domain translation and
also holds a trademark on the name. Their page says:

> The World English Bible is in the Public Domain. That means that it is not
> copyrighted. However, "World English Bible" is a Trademark of eBible.org.
>
> You may copy, publish, proclaim, distribute, redistribute, sell, give away,
> quote, memorize, read publicly, broadcast, transmit, share, back up, post on
> the Internet, print, reproduce, preach, teach from, and use the World English
> Bible as much as you want, and others may also do so. All we ask is that if
> you CHANGE the actual text of the World English Bible in any way, you not call
> the result the World English Bible any more.

The restriction is on the name, not on the text. The application ships the text
unchanged, so it names the translation as the World English Bible, and this
notice records the trademark.

### The King James Version in the United Kingdom

The King James Version is public domain in most countries. The United Kingdom is
the exception. The Crown holds a royal prerogative over printing the Authorised
Version, and the Crown licenses publishers through letters patent. eBible.org
writes it this way:

> Letters patent issued by King James with no expiration date means that to
> print this translation in the United Kingdom or import printed copies into the
> UK, you need permission. Currently, the Cambridge University Press, the Oxford
> University Press, and Collins have the exclusive right to print this Bible
> translation in the UK. This royal decree has no effect outside of the UK,
> where this work is firmly in the Public Domain.

The restriction reaches printing, publishing, and importing into the United
Kingdom. Section 171(1)(b) of the Copyright, Designs and Patents Act 1988
preserves "any right or privilege of the Crown subsisting otherwise than under an
enactment", which is how the prerogative survives the Act's normal fifty-year
term.

The application ships the KJV text inside a software product and makes no
printed copy. Anyone who distributes a build that prints or publishes the text
in the United Kingdom needs to deal with this separately.

### What this file does not claim

This file does not claim that the translations are free of every restriction
that a particular jurisdiction recognises. It records the position published by
the projects that release the texts. It is not legal advice, and a distributor
who needs certainty should get it from a lawyer in the jurisdictions they ship
to.

## The bundled fonts

The reader sets the scripture text in Literata and the interface and verse
numbers in Inter. Both faces are embedded in the binary and both are under the
SIL Open Font License 1.1. The licence text ships in the bundle as
`OFL-Literata.txt` and `OFL-Inter.txt`.

| File | Face | Licence | Source |
|---|---|---|---|
| `assets/fonts/Literata.ttf` | Literata variable, roman | OFL-1.1 | <https://github.com/google/fonts/tree/main/ofl/literata> |
| `assets/fonts/Literata-Italic.ttf` | Literata variable, italic | OFL-1.1 | <https://github.com/google/fonts/tree/main/ofl/literata> |
| `assets/fonts/Inter.ttf` | Inter variable | OFL-1.1 | <https://github.com/google/fonts/tree/main/ofl/inter> |

The Open Font License lets the fonts be bundled and redistributed as long as
the licence travels with them, which is what the bundle does.

### The toolkit's own fallback fonts

The GUI toolkit keeps its own faces behind the reader's faces, so a glyph the
reading faces lack still renders. The release binary embeds four of them, and
each carries its own licence:

| Face | Licence | Where the text lives |
|---|---|---|
| Hack | MIT, with the Bitstream Vera licence in the name table | `epaint_default_fonts` |
| Noto Emoji | OFL-1.1 | `epaint_default_fonts` |
| Ubuntu | Ubuntu Font Licence 1.0 | `epaint_default_fonts` |
| emoji-icon-font | MIT | `epaint_default_fonts` |

The `epaint_default_fonts` crate declares its own licence as
`(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`, which is the combined
statement of these four.

## The Rust dependencies

The application is a Cargo workspace with three members: `crates/core`, which
holds the translation model, the importers, the search, and the study store,
`crates/app`, which holds the window and the command line, and `xtask`, which
holds the build tasks. The lock file resolves 438 crates from crates.io.

The direct dependencies of the three members are these.

| Crate | Version | Licence | Used for |
|---|---|---|---|
| anyhow | 1.0.104 | MIT OR Apache-2.0 | Error values that carry context |
| serde | 1.0.229 | MIT OR Apache-2.0 | Deriving the metadata serialisation |
| serde_json | 1.0.154 | MIT OR Apache-2.0 | The uncompressed metadata header |
| memchr | 2.8.3 | Unlicense OR MIT | Locating separators while parsing references |
| zstd | 0.13.3 | MIT | Compressing and decompressing the translation body |
| zstd-sys | 2.1.2 | BSD-3-Clause | The upstream zstd C library the zstd crate links |
| rusqlite | 0.40.2 | MIT | The study store |
| libsqlite3-sys | 0.38.2 | MIT | The SQLite 3.53.2 library the study store links, built from the bundled amalgamation |
| zip | 2.6.1 | MIT | Reading the zip archives the importers accept |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | The deflate decoder the zip importer uses |
| quick-xml | 0.38.4 | MIT | The OSIS and Zefania XML importers |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | The digest check in the data build |
| eframe | 0.36.2 | MIT OR Apache-2.0 | The window and the renderer |
| egui | 0.36.2 | MIT OR Apache-2.0 | The widgets and the immediate-mode context |
| egui_kittest | 0.36.2 | MIT OR Apache-2.0 | The offscreen render harness |
| image | 0.25.10 | MIT OR Apache-2.0 | The PNG writer the render harness uses |

### What the dependency licences are, in full

Every crate in the lock file declares a licence, and `cargo metadata` reports
one for all 438. The declared licences, with the count of crates that declare
each exact string, are these.

| Declared licence | Crates |
|---|---|
| MIT OR Apache-2.0 | 191 |
| MIT | 98 |
| Apache-2.0 OR MIT | 43 |
| Unicode-3.0 | 18 |
| Apache-2.0 | 16 |
| MIT/Apache-2.0 | 15 |
| Zlib OR Apache-2.0 OR MIT | 10 |
| MIT OR Apache-2.0 OR Zlib | 6 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 6 |
| Unlicense OR MIT | 4 |
| BSD-3-Clause | 4 |
| Zlib | 3 |
| BSL-1.0 | 2 |
| MIT OR Zlib OR Apache-2.0 | 2 |
| BSD-3-Clause OR Apache-2.0 | 2 |
| BSD-3-Clause OR MIT OR Apache-2.0 | 2 |
| Apache-2.0/MIT | 2 |
| MIT OR Apache-2.0 OR LGPL-2.1-or-later | 2 |
| Unlicense/MIT | 2 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| BSD-2-Clause | 1 |
| MIT / Apache-2.0 | 1 |
| Apache-2.0 AND MIT | 1 |
| (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 | 1 |
| ISC | 1 |
| Apache-2.0 OR GPL-2.0-only | 1 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |

Read together, and counting each crate once under the widest licence it
offers, that is 373 crates under MIT and Apache-2.0 alone, 21 that add Zlib to
that pair, 19 that add Unicode-3.0, 12 under a BSD licence, 6 that add the
Unlicense, 3 that name the 0BSD, ISC, or BSL-1.0 licences, 3 that offer a
copyleft alternative, and 1 that adds a font licence. The counts add up to
438.

No crate in the lock file declares a licence that requires the whole program to
be published under the same terms, and no crate declares a licence that
requires the source of a non-Rust component to be published. The two crates
that carry a copyleft alternative, `self_cell` and `r-efi`, offer it as an
option alongside a permissive licence.

The command that produces the counts is:

```text
cargo metadata --format-version 1 --locked | python3 -c 'import json,sys,collections; c=collections.Counter(p["license"] for p in json.load(sys.stdin)["packages"] if p.get("source")); [print(v,k) for k,v in c.most_common()]'
```

### What this section does not claim

The label table above does not replace the texts. `scripts/collect-licenses.py`
copies the licence files each crate in the runtime dependency closure actually
ships into `LICENSES/`, together with the native zstd C library's BSD licence,
the four fallback-font notices, and this project's own licence and font
licences. `LICENSES/INDEX.md` records what applies to what. The bundle step
runs the collector's `--check` first and refuses to build when a required text
is missing, so a gap is an error rather than a silent omission.

A crate that declares a standard licence but ships no licence file in its
archive gets the standard text of that licence with a header naming the crate,
version, and declared licence. The header says the crate shipped no copyright
notice and does not invent one. A crate whose declared licence has no
available text stops the collection with a non-zero exit.

## The bundle's own notices

`Bible.app/Contents/Resources/` carries the notices that have to travel with
the binary.

| File | What it is |
|---|---|
| `LICENSE` | The MIT licence of this program |
| `THIRD_PARTY.md` | This file |
| `OFL-Inter.txt` | The Open Font License for Inter |
| `OFL-Literata.txt` | The Open Font License for Literata |
| `LICENSES/` | The dependency, native zstd, and fallback-font licence texts, with `INDEX.md` naming what each file covers |
| `build-info.txt` | The source binary path, digest, derived minimum macOS version, and architectures; sealed by the bundle signature |
| `dist/build-info.postsign.txt` | The final signed executable digest, size, and codesign CDHash, written beside the bundle rather than inside it |

## The clean-room rewrite

The program spec in `docs/SPEC.md` describes this reader as a clean-room
rewrite. This file does not claim that any legal conclusion follows from that
description, and it does not claim that the reimplementation is
line-by-line independent of any other implementation. What the repository
shows is that the source in `crates/` was written for this repository, carries
this project's SPDX headers, and does not copy source, comments, or distinctive
naming from any other tree.
