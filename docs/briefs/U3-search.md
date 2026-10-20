GOAL
Build the search engine: a normalized in-memory corpus per translation, a query parser for
the query language, and a scanner that finds matching verses fast enough to stay under
10 ms across the whole Bible.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/core/src/search/mod.rs     the search engine
  crates/core/src/search/query.rs   the query parser
  crates/core/src/search/corpus.rs   the normalized corpus
  crates/core/src/search/scan.rs     the scanner
  crates/core/tests/search_*.rs     the tests
  crates/core/Cargo.toml             add the dependencies you need
  crates/core/benches/search.rs      the benchmark
Do not touch any other path. In particular do not touch books.rs, model.rs, format.rs, or
reference.rs. Another unit owns those, and your unit builds on the API they define.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read "The translation model" and
"Search" before you start.

You build on the model that unit U1 defines. Read U1's merged code for the exact names. A
translation is one text string plus flat arrays for verses, marks, and spans. Each verse
carries its number, the byte range of its text, and the ranges into the marks and spans
arrays.

There is no inverted index. Each translation lazily builds a normalized corpus in memory:
lower-cased, diacritics folded, apostrophes joined, punctuation turned into spaces. The
corpus is laid out as one verse per space-wrapped line, so a verse's text is surrounded by
spaces on both sides.

A search term is a SIMD substring scan for the term with a space on each side. A phrase is a
longer needle. Because the corpus wraps every verse in spaces and a needle never contains a
double space, a match can never straddle two verses. A hit maps to its verse with a binary
search, and the scan then jumps to the next verse. A multi-term query scans the longest and
rarest term first, then filters the candidates.

The query language supports these forms.

- Plain words, where every word must appear.
- A word prefix, which is the default, or whole words only.
- "an exact phrase".
- -excluded words.
- A scope: the whole Bible, the Old Testament, the New Testament, or one book.

The budget is 0.15 ms to 1.8 ms across the KJV, and 10 ms is the hard ceiling. There is no
index to build at import time and nothing to store on disk.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- The corpus test asserts that normalization lower-cases, folds the accents, joins an
  apostrophe, and turns punctuation into spaces. Give the test input with a mix of all
  four and assert the exact expected output string.
- The corpus test asserts that a match for a word cannot straddle two verses, by building
  a corpus where the end of one verse and the start of the next would form a word if the
  spaces were not there, and asserting the search finds nothing.
- The query test asserts that a plain query yields its terms, that a quoted phrase is one
  term that keeps its spaces, that a leading minus makes a term excluded, and that a query
  of only whitespace is empty.
- The query test asserts that whole-word mode rejects a prefix match and that prefix mode
  accepts one.
- The search test finds a known word in a synthetic corpus and returns the right verse
  numbers.
- The search test asserts that every term of a multi-term query must match, that an
  excluded term removes a verse, and that a phrase match needs the words adjacent.
- The search test asserts the scope filters correctly for the Old Testament, the New
  Testament, and one book.
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

Then measure the search and report the numbers:

    cargo bench -p bible-core --bench search -- --warm-up-time 1 --measurement-time 3

The benchmark builds a synthetic full-Bible corpus of 31102 verses and runs a set of
queries against it. Report the time for each query. Every single-term query must be under
10 ms. The median must be under 2 ms. If the median is over 2 ms, the implementation is too
slow and you must make it faster before you report. Paste the benchmark output.

TIMEBOX
Five hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not bundle or commit any Bible text, public domain or not.
Do not build an inverted index. The design is a scan.
Do not add a dependency that is not needed. The SIMD substring search should come from
memchr, which the crate already has.
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
- The data shape you chose for the corpus and the query, in a few sentences. Say why the
  query is shaped that way and not as a bag of strings.
- The public API, as a short list of the type and function names.
- Deviations from this brief, and why.
- Suggested follow-ups.
