// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

use bible_core::books;
use bible_core::model::{Translation, TranslationBuilder, TranslationMeta};
use bible_core::search::{Corpus, Query, normalize};

#[test]
fn normalization_lower_cases_folds_joins_and_splits() {
    let input = "The LORD’s mercy—endures! Café déjà vu.";
    assert_eq!(
        normalize(input),
        "the lords mercy endures cafe deja vu",
        "lower case, folded accents, a joined apostrophe, and punctuation as spaces"
    );
}

#[test]
fn a_word_cannot_straddle_two_verses() {
    let translation = synthetic(&[(0, "foun"), (1, "dation")]);
    let corpus = Corpus::from_translation(&translation);

    assert!(
        corpus.search(&Query::parse("foundation")).is_empty(),
        "the verse boundary keeps the halves apart"
    );
    assert_eq!(
        corpus.search(&Query::parse("foun")),
        vec![bible_core::search::Hit { verse: 0 }],
        "each half still matches inside its own verse"
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
