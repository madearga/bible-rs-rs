# Fonts

The reader sets the body text in Literata and the interface and verse numbers in
Inter. Both faces are under the SIL Open Font License 1.1, and the licence text
ships beside them as `OFL-Literata.txt` and `OFL-Inter.txt`.

| File | Face | Source |
|---|---|---|
| `Literata.ttf` | Literata variable, roman, optical size and weight axes | https://github.com/google/fonts/tree/main/ofl/literata |
| `Literata-Italic.ttf` | Literata variable, italic, optical size and weight axes | https://github.com/google/fonts/tree/main/ofl/literata |
| `Inter.ttf` | Inter variable, optical size and weight axes | https://github.com/google/fonts/tree/main/ofl/inter |

The files are embedded into the binary and registered in one table in
`crates/app/src/theme.rs`, so swapping a face is a change to that table and
nothing else.

`assets/sample/passage.bbl` is a generated sample translation. Every word in it is
filler, so the reader has a typographic page to paint before any translation is
installed. It carries no scripture.
