// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

use bible_core::books;
use bible_core::model::{Translation, TranslationBuilder, TranslationMeta};
use bible_core::search::{Corpus, Hit, MatchMode, Query, Scope};

#[test]
fn a_known_word_returns_the_right_verses() {
    let translation = synthetic(&[(0, "alpha beta"), (3, "beta gamma"), (7, "gamma delta")]);
    let corpus = Corpus::from_translation(&translation);

    assert_eq!(corpus.search(&Query::parse("beta")), verses(&[0, 3]));
    assert_eq!(corpus.search(&Query::parse("gamma")), verses(&[3, 7]));
    assert!(corpus.search(&Query::parse("omega")).is_empty());
}

#[test]
fn every_term_of_a_multi_term_query_must_match() {
    let translation = synthetic(&[
        (0, "alpha beta"),
        (1, "beta alone"),
        (2, "alpha alone"),
        (3, "alpha and beta"),
    ]);
    let corpus = Corpus::from_translation(&translation);

    assert_eq!(corpus.search(&Query::parse("alpha beta")), verses(&[0, 3]));
    assert_eq!(corpus.search(&Query::parse("beta alpha")), verses(&[0, 3]));
}

#[test]
fn an_excluded_term_removes_a_verse() {
    let translation = synthetic(&[
        (0, "alpha beta"),
        (1, "alpha gamma"),
        (2, "alpha beta gamma"),
    ]);
    let corpus = Corpus::from_translation(&translation);

    assert_eq!(corpus.search(&Query::parse("alpha")), verses(&[0, 1, 2]));
    assert_eq!(corpus.search(&Query::parse("alpha -beta")), verses(&[1]));
    assert_eq!(
        corpus.search(&Query::parse("alpha -\"beta gamma\"")),
        verses(&[0, 1])
    );
}

#[test]
fn a_phrase_needs_the_words_adjacent() {
    let translation = synthetic(&[
        (0, "the quick brown fox"),
        (1, "quick and brown"),
        (2, "a quick brown thing"),
    ]);
    let corpus = Corpus::from_translation(&translation);

    assert_eq!(
        corpus.search(&Query::parse("\"quick brown\"")),
        verses(&[0, 2])
    );
    assert_eq!(
        corpus.search(&Query::parse("\"quick brown\"").with_mode(MatchMode::WholeWord)),
        verses(&[0, 2])
    );
    assert!(corpus.search(&Query::parse("\"brown quick\"")).is_empty());
}

#[test]
fn a_scope_filters_by_testament_and_by_book() {
    let old_verse = 5;
    let new_start = books::book(39).expect("Matthew").verse_start;
    let new_verse = new_start + 4;
    let john_start = books::book(42).expect("John").verse_start;
    let john_verse = john_start + 2;
    let translation = synthetic(&[
        (old_verse, "shared light in the old testament"),
        (new_verse, "shared light in the new testament"),
        (john_verse, "shared light in john"),
    ]);
    let corpus = Corpus::from_translation(&translation);

    assert_eq!(
        corpus.search(&Query::parse("shared")),
        verses(&[old_verse, new_verse, john_verse])
    );
    assert_eq!(
        corpus.search(&Query::parse("shared").with_scope(Scope::OldTestament)),
        verses(&[old_verse])
    );
    assert_eq!(
        corpus.search(&Query::parse("shared").with_scope(Scope::NewTestament)),
        verses(&[new_verse, john_verse])
    );
    assert_eq!(
        corpus.search(&Query::parse("shared").with_scope(Scope::Book(42))),
        verses(&[john_verse])
    );
}

/// Turns canonical verse indices into the hits the scanner returns.
fn verses(indices: &[u16]) -> Vec<Hit> {
    indices
        .iter()
        .map(|&verse| Hit {
            verse: u32::from(verse),
        })
        .collect()
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
