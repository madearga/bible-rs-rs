// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

use bible_core::books;
use bible_core::model::{Translation, TranslationBuilder, TranslationMeta};
use bible_core::search::{Corpus, Hit, MatchMode, Query};

#[test]
fn a_plain_query_yields_its_terms() {
    let query = Query::parse("light world");
    assert_eq!(query.terms().len(), 2);
    assert_eq!(query.terms()[0].text(), "light");
    assert_eq!(query.terms()[1].text(), "world");
    assert!(query.terms().iter().all(|term| !term.is_excluded()));
}

#[test]
fn a_quoted_phrase_is_one_term_that_keeps_its_spaces() {
    let query = Query::parse("\"exact phrase\"");
    assert_eq!(query.terms().len(), 1);
    assert_eq!(query.terms()[0].text(), "exact phrase");
}

#[test]
fn a_leading_minus_excludes_the_term() {
    let query = Query::parse("light -dark");
    assert_eq!(query.terms().len(), 2);
    assert!(!query.terms()[0].is_excluded());
    assert!(query.terms()[1].is_excluded());
    assert_eq!(query.terms()[1].text(), "dark");
}

#[test]
fn a_query_of_only_whitespace_is_empty() {
    let query = Query::parse("   \n\t ");
    assert!(query.is_empty());
    assert!(query.terms().is_empty());
}

#[test]
fn whole_word_mode_rejects_a_prefix_that_prefix_mode_accepts() {
    let translation = synthetic(&[
        (0, "lightning strikes the hill"),
        (1, "a light in the dark"),
    ]);
    let corpus = Corpus::from_translation(&translation);
    let prefix = Query::parse("light");
    let whole = Query::parse("light").with_mode(MatchMode::WholeWord);

    assert_eq!(
        corpus.search(&prefix),
        vec![Hit { verse: 0 }, Hit { verse: 1 }],
        "prefix mode finds the longer word"
    );
    assert_eq!(
        corpus.search(&whole),
        vec![Hit { verse: 1 }],
        "whole-word mode keeps only the exact word"
    );
}

/// Builds a full synthetic translation with the given verse texts.
fn synthetic(texts: &[(u16, &str)]) -> Translation {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("synthetic", "Synthetic"));
    let mut index = 0u16;
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter).expect("chapter exists");
            for verse in 1..=count {
                let text = texts
                    .iter()
                    .find(|(at, _)| *at == index)
                    .map_or("", |(_, text)| *text);
                builder
                    .push_verse(verse as u8, text)
                    .expect("canonical order");
                index += 1;
            }
        }
    }
    builder.finish().expect("the whole grid is present")
}
