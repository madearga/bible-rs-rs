// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The canonical grid that every translation shares.
//!
//! The grid fixes the book order, each book's chapter count, and each chapter's
//! verse count. Because the numbers never vary between translations, they are the
//! key for verse-aligned views and translation-independent study data.

/// Number of books in the canonical order, Genesis first and Revelation last.
pub const BOOK_COUNT: usize = 66;

/// Number of chapters across the whole canon.
pub const CHAPTER_COUNT: usize = 1189;

/// Number of verses across the whole canon.
pub const VERSE_COUNT: usize = 31102;

/// The half of the canon a book belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Testament {
    /// Books from Genesis to Malachi.
    Old,
    /// Books from Matthew to Revelation.
    New,
}

/// One entry in the canonical book table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Book {
    /// Position in the canonical order, Genesis at 0.
    pub index: u8,
    /// Full display name, such as `1 Corinthians`.
    pub name: &'static str,
    /// Short form used in references and the picker, such as `1Cor`.
    pub abbreviation: &'static str,
    /// Which testament the book belongs to.
    pub testament: Testament,
    /// Number of chapters the book contains.
    pub chapter_count: u8,
    /// Global chapter index of the book's first chapter.
    pub chapter_start: u16,
    /// Global verse index of the book's first verse.
    pub verse_start: u16,
}

/// The books in canonical order.
static BOOKS: [Book; BOOK_COUNT] = [
    Book {
        index: 0,
        name: "Genesis",
        abbreviation: "Gen",
        testament: Testament::Old,
        chapter_count: 50,
        chapter_start: 0,
        verse_start: 0,
    },
    Book {
        index: 1,
        name: "Exodus",
        abbreviation: "Exod",
        testament: Testament::Old,
        chapter_count: 40,
        chapter_start: 50,
        verse_start: 1533,
    },
    Book {
        index: 2,
        name: "Leviticus",
        abbreviation: "Lev",
        testament: Testament::Old,
        chapter_count: 27,
        chapter_start: 90,
        verse_start: 2746,
    },
    Book {
        index: 3,
        name: "Numbers",
        abbreviation: "Num",
        testament: Testament::Old,
        chapter_count: 36,
        chapter_start: 117,
        verse_start: 3605,
    },
    Book {
        index: 4,
        name: "Deuteronomy",
        abbreviation: "Deut",
        testament: Testament::Old,
        chapter_count: 34,
        chapter_start: 153,
        verse_start: 4893,
    },
    Book {
        index: 5,
        name: "Joshua",
        abbreviation: "Josh",
        testament: Testament::Old,
        chapter_count: 24,
        chapter_start: 187,
        verse_start: 5852,
    },
    Book {
        index: 6,
        name: "Judges",
        abbreviation: "Judg",
        testament: Testament::Old,
        chapter_count: 21,
        chapter_start: 211,
        verse_start: 6510,
    },
    Book {
        index: 7,
        name: "Ruth",
        abbreviation: "Ruth",
        testament: Testament::Old,
        chapter_count: 4,
        chapter_start: 232,
        verse_start: 7128,
    },
    Book {
        index: 8,
        name: "1 Samuel",
        abbreviation: "1Sam",
        testament: Testament::Old,
        chapter_count: 31,
        chapter_start: 236,
        verse_start: 7213,
    },
    Book {
        index: 9,
        name: "2 Samuel",
        abbreviation: "2Sam",
        testament: Testament::Old,
        chapter_count: 24,
        chapter_start: 267,
        verse_start: 8023,
    },
    Book {
        index: 10,
        name: "1 Kings",
        abbreviation: "1Kgs",
        testament: Testament::Old,
        chapter_count: 22,
        chapter_start: 291,
        verse_start: 8718,
    },
    Book {
        index: 11,
        name: "2 Kings",
        abbreviation: "2Kgs",
        testament: Testament::Old,
        chapter_count: 25,
        chapter_start: 313,
        verse_start: 9534,
    },
    Book {
        index: 12,
        name: "1 Chronicles",
        abbreviation: "1Chr",
        testament: Testament::Old,
        chapter_count: 29,
        chapter_start: 338,
        verse_start: 10253,
    },
    Book {
        index: 13,
        name: "2 Chronicles",
        abbreviation: "2Chr",
        testament: Testament::Old,
        chapter_count: 36,
        chapter_start: 367,
        verse_start: 11195,
    },
    Book {
        index: 14,
        name: "Ezra",
        abbreviation: "Ezra",
        testament: Testament::Old,
        chapter_count: 10,
        chapter_start: 403,
        verse_start: 12017,
    },
    Book {
        index: 15,
        name: "Nehemiah",
        abbreviation: "Neh",
        testament: Testament::Old,
        chapter_count: 13,
        chapter_start: 413,
        verse_start: 12297,
    },
    Book {
        index: 16,
        name: "Esther",
        abbreviation: "Esth",
        testament: Testament::Old,
        chapter_count: 10,
        chapter_start: 426,
        verse_start: 12703,
    },
    Book {
        index: 17,
        name: "Job",
        abbreviation: "Job",
        testament: Testament::Old,
        chapter_count: 42,
        chapter_start: 436,
        verse_start: 12870,
    },
    Book {
        index: 18,
        name: "Psalms",
        abbreviation: "Ps",
        testament: Testament::Old,
        chapter_count: 150,
        chapter_start: 478,
        verse_start: 13940,
    },
    Book {
        index: 19,
        name: "Proverbs",
        abbreviation: "Prov",
        testament: Testament::Old,
        chapter_count: 31,
        chapter_start: 628,
        verse_start: 16401,
    },
    Book {
        index: 20,
        name: "Ecclesiastes",
        abbreviation: "Eccl",
        testament: Testament::Old,
        chapter_count: 12,
        chapter_start: 659,
        verse_start: 17316,
    },
    Book {
        index: 21,
        name: "Song of Solomon",
        abbreviation: "Song",
        testament: Testament::Old,
        chapter_count: 8,
        chapter_start: 671,
        verse_start: 17538,
    },
    Book {
        index: 22,
        name: "Isaiah",
        abbreviation: "Isa",
        testament: Testament::Old,
        chapter_count: 66,
        chapter_start: 679,
        verse_start: 17655,
    },
    Book {
        index: 23,
        name: "Jeremiah",
        abbreviation: "Jer",
        testament: Testament::Old,
        chapter_count: 52,
        chapter_start: 745,
        verse_start: 18947,
    },
    Book {
        index: 24,
        name: "Lamentations",
        abbreviation: "Lam",
        testament: Testament::Old,
        chapter_count: 5,
        chapter_start: 797,
        verse_start: 20311,
    },
    Book {
        index: 25,
        name: "Ezekiel",
        abbreviation: "Ezek",
        testament: Testament::Old,
        chapter_count: 48,
        chapter_start: 802,
        verse_start: 20465,
    },
    Book {
        index: 26,
        name: "Daniel",
        abbreviation: "Dan",
        testament: Testament::Old,
        chapter_count: 12,
        chapter_start: 850,
        verse_start: 21738,
    },
    Book {
        index: 27,
        name: "Hosea",
        abbreviation: "Hos",
        testament: Testament::Old,
        chapter_count: 14,
        chapter_start: 862,
        verse_start: 22095,
    },
    Book {
        index: 28,
        name: "Joel",
        abbreviation: "Joel",
        testament: Testament::Old,
        chapter_count: 3,
        chapter_start: 876,
        verse_start: 22292,
    },
    Book {
        index: 29,
        name: "Amos",
        abbreviation: "Amos",
        testament: Testament::Old,
        chapter_count: 9,
        chapter_start: 879,
        verse_start: 22365,
    },
    Book {
        index: 30,
        name: "Obadiah",
        abbreviation: "Obad",
        testament: Testament::Old,
        chapter_count: 1,
        chapter_start: 888,
        verse_start: 22511,
    },
    Book {
        index: 31,
        name: "Jonah",
        abbreviation: "Jonah",
        testament: Testament::Old,
        chapter_count: 4,
        chapter_start: 889,
        verse_start: 22532,
    },
    Book {
        index: 32,
        name: "Micah",
        abbreviation: "Mic",
        testament: Testament::Old,
        chapter_count: 7,
        chapter_start: 893,
        verse_start: 22580,
    },
    Book {
        index: 33,
        name: "Nahum",
        abbreviation: "Nah",
        testament: Testament::Old,
        chapter_count: 3,
        chapter_start: 900,
        verse_start: 22685,
    },
    Book {
        index: 34,
        name: "Habakkuk",
        abbreviation: "Hab",
        testament: Testament::Old,
        chapter_count: 3,
        chapter_start: 903,
        verse_start: 22732,
    },
    Book {
        index: 35,
        name: "Zephaniah",
        abbreviation: "Zeph",
        testament: Testament::Old,
        chapter_count: 3,
        chapter_start: 906,
        verse_start: 22788,
    },
    Book {
        index: 36,
        name: "Haggai",
        abbreviation: "Hag",
        testament: Testament::Old,
        chapter_count: 2,
        chapter_start: 909,
        verse_start: 22841,
    },
    Book {
        index: 37,
        name: "Zechariah",
        abbreviation: "Zech",
        testament: Testament::Old,
        chapter_count: 14,
        chapter_start: 911,
        verse_start: 22879,
    },
    Book {
        index: 38,
        name: "Malachi",
        abbreviation: "Mal",
        testament: Testament::Old,
        chapter_count: 4,
        chapter_start: 925,
        verse_start: 23090,
    },
    Book {
        index: 39,
        name: "Matthew",
        abbreviation: "Matt",
        testament: Testament::New,
        chapter_count: 28,
        chapter_start: 929,
        verse_start: 23145,
    },
    Book {
        index: 40,
        name: "Mark",
        abbreviation: "Mark",
        testament: Testament::New,
        chapter_count: 16,
        chapter_start: 957,
        verse_start: 24216,
    },
    Book {
        index: 41,
        name: "Luke",
        abbreviation: "Luke",
        testament: Testament::New,
        chapter_count: 24,
        chapter_start: 973,
        verse_start: 24894,
    },
    Book {
        index: 42,
        name: "John",
        abbreviation: "John",
        testament: Testament::New,
        chapter_count: 21,
        chapter_start: 997,
        verse_start: 26045,
    },
    Book {
        index: 43,
        name: "Acts",
        abbreviation: "Acts",
        testament: Testament::New,
        chapter_count: 28,
        chapter_start: 1018,
        verse_start: 26924,
    },
    Book {
        index: 44,
        name: "Romans",
        abbreviation: "Rom",
        testament: Testament::New,
        chapter_count: 16,
        chapter_start: 1046,
        verse_start: 27931,
    },
    Book {
        index: 45,
        name: "1 Corinthians",
        abbreviation: "1Cor",
        testament: Testament::New,
        chapter_count: 16,
        chapter_start: 1062,
        verse_start: 28364,
    },
    Book {
        index: 46,
        name: "2 Corinthians",
        abbreviation: "2Cor",
        testament: Testament::New,
        chapter_count: 13,
        chapter_start: 1078,
        verse_start: 28801,
    },
    Book {
        index: 47,
        name: "Galatians",
        abbreviation: "Gal",
        testament: Testament::New,
        chapter_count: 6,
        chapter_start: 1091,
        verse_start: 29058,
    },
    Book {
        index: 48,
        name: "Ephesians",
        abbreviation: "Eph",
        testament: Testament::New,
        chapter_count: 6,
        chapter_start: 1097,
        verse_start: 29207,
    },
    Book {
        index: 49,
        name: "Philippians",
        abbreviation: "Phil",
        testament: Testament::New,
        chapter_count: 4,
        chapter_start: 1103,
        verse_start: 29362,
    },
    Book {
        index: 50,
        name: "Colossians",
        abbreviation: "Col",
        testament: Testament::New,
        chapter_count: 4,
        chapter_start: 1107,
        verse_start: 29466,
    },
    Book {
        index: 51,
        name: "1 Thessalonians",
        abbreviation: "1Thess",
        testament: Testament::New,
        chapter_count: 5,
        chapter_start: 1111,
        verse_start: 29561,
    },
    Book {
        index: 52,
        name: "2 Thessalonians",
        abbreviation: "2Thess",
        testament: Testament::New,
        chapter_count: 3,
        chapter_start: 1116,
        verse_start: 29650,
    },
    Book {
        index: 53,
        name: "1 Timothy",
        abbreviation: "1Tim",
        testament: Testament::New,
        chapter_count: 6,
        chapter_start: 1119,
        verse_start: 29697,
    },
    Book {
        index: 54,
        name: "2 Timothy",
        abbreviation: "2Tim",
        testament: Testament::New,
        chapter_count: 4,
        chapter_start: 1125,
        verse_start: 29810,
    },
    Book {
        index: 55,
        name: "Titus",
        abbreviation: "Titus",
        testament: Testament::New,
        chapter_count: 3,
        chapter_start: 1129,
        verse_start: 29893,
    },
    Book {
        index: 56,
        name: "Philemon",
        abbreviation: "Phlm",
        testament: Testament::New,
        chapter_count: 1,
        chapter_start: 1132,
        verse_start: 29939,
    },
    Book {
        index: 57,
        name: "Hebrews",
        abbreviation: "Heb",
        testament: Testament::New,
        chapter_count: 13,
        chapter_start: 1133,
        verse_start: 29964,
    },
    Book {
        index: 58,
        name: "James",
        abbreviation: "Jas",
        testament: Testament::New,
        chapter_count: 5,
        chapter_start: 1146,
        verse_start: 30267,
    },
    Book {
        index: 59,
        name: "1 Peter",
        abbreviation: "1Pet",
        testament: Testament::New,
        chapter_count: 5,
        chapter_start: 1151,
        verse_start: 30375,
    },
    Book {
        index: 60,
        name: "2 Peter",
        abbreviation: "2Pet",
        testament: Testament::New,
        chapter_count: 3,
        chapter_start: 1156,
        verse_start: 30480,
    },
    Book {
        index: 61,
        name: "1 John",
        abbreviation: "1John",
        testament: Testament::New,
        chapter_count: 5,
        chapter_start: 1159,
        verse_start: 30541,
    },
    Book {
        index: 62,
        name: "2 John",
        abbreviation: "2John",
        testament: Testament::New,
        chapter_count: 1,
        chapter_start: 1164,
        verse_start: 30646,
    },
    Book {
        index: 63,
        name: "3 John",
        abbreviation: "3John",
        testament: Testament::New,
        chapter_count: 1,
        chapter_start: 1165,
        verse_start: 30659,
    },
    Book {
        index: 64,
        name: "Jude",
        abbreviation: "Jude",
        testament: Testament::New,
        chapter_count: 1,
        chapter_start: 1166,
        verse_start: 30673,
    },
    Book {
        index: 65,
        name: "Revelation",
        abbreviation: "Rev",
        testament: Testament::New,
        chapter_count: 22,
        chapter_start: 1167,
        verse_start: 30698,
    },
];

/// Verse counts for each of the 1189 chapters, in canonical order.
///
/// This is the KJV versification. Every translation is laid out against it even
/// when it prints nothing for a verse.
pub const VERSES_PER_CHAPTER: [u16; CHAPTER_COUNT] = [
    31, 25, 24, 26, 32, 22, 24, 22, 29, 32, 32, 20, 18, 24, 21, 16, 27, 33, 38, 18, 34, 24, 20, 67,
    34, 35, 46, 22, 35, 43, 55, 32, 20, 31, 29, 43, 36, 30, 23, 23, 57, 38, 34, 34, 28, 34, 31, 22,
    33, 26, 22, 25, 22, 31, 23, 30, 25, 32, 35, 29, 10, 51, 22, 31, 27, 36, 16, 27, 25, 26, 36, 31,
    33, 18, 40, 37, 21, 43, 46, 38, 18, 35, 23, 35, 35, 38, 29, 31, 43, 38, 17, 16, 17, 35, 19, 30,
    38, 36, 24, 20, 47, 8, 59, 57, 33, 34, 16, 30, 37, 27, 24, 33, 44, 23, 55, 46, 34, 54, 34, 51,
    49, 31, 27, 89, 26, 23, 36, 35, 16, 33, 45, 41, 50, 13, 32, 22, 29, 35, 41, 30, 25, 18, 65, 23,
    31, 40, 16, 54, 42, 56, 29, 34, 13, 46, 37, 29, 49, 33, 25, 26, 20, 29, 22, 32, 32, 18, 29, 23,
    22, 20, 22, 21, 20, 23, 30, 25, 22, 19, 19, 26, 68, 29, 20, 30, 52, 29, 12, 18, 24, 17, 24, 15,
    27, 26, 35, 27, 43, 23, 24, 33, 15, 63, 10, 18, 28, 51, 9, 45, 34, 16, 33, 36, 23, 31, 24, 31,
    40, 25, 35, 57, 18, 40, 15, 25, 20, 20, 31, 13, 31, 30, 48, 25, 22, 23, 18, 22, 28, 36, 21, 22,
    12, 21, 17, 22, 27, 27, 15, 25, 23, 52, 35, 23, 58, 30, 24, 42, 15, 23, 29, 22, 44, 25, 12, 25,
    11, 31, 13, 27, 32, 39, 12, 25, 23, 29, 18, 13, 19, 27, 31, 39, 33, 37, 23, 29, 33, 43, 26, 22,
    51, 39, 25, 53, 46, 28, 34, 18, 38, 51, 66, 28, 29, 43, 33, 34, 31, 34, 34, 24, 46, 21, 43, 29,
    53, 18, 25, 27, 44, 27, 33, 20, 29, 37, 36, 21, 21, 25, 29, 38, 20, 41, 37, 37, 21, 26, 20, 37,
    20, 30, 54, 55, 24, 43, 26, 81, 40, 40, 44, 14, 47, 40, 14, 17, 29, 43, 27, 17, 19, 8, 30, 19,
    32, 31, 31, 32, 34, 21, 30, 17, 18, 17, 22, 14, 42, 22, 18, 31, 19, 23, 16, 22, 15, 19, 14, 19,
    34, 11, 37, 20, 12, 21, 27, 28, 23, 9, 27, 36, 27, 21, 33, 25, 33, 27, 23, 11, 70, 13, 24, 17,
    22, 28, 36, 15, 44, 11, 20, 32, 23, 19, 19, 73, 18, 38, 39, 36, 47, 31, 22, 23, 15, 17, 14, 14,
    10, 17, 32, 3, 22, 13, 26, 21, 27, 30, 21, 22, 35, 22, 20, 25, 28, 22, 35, 22, 16, 21, 29, 29,
    34, 30, 17, 25, 6, 14, 23, 28, 25, 31, 40, 22, 33, 37, 16, 33, 24, 41, 30, 24, 34, 17, 6, 12,
    8, 8, 12, 10, 17, 9, 20, 18, 7, 8, 6, 7, 5, 11, 15, 50, 14, 9, 13, 31, 6, 10, 22, 12, 14, 9,
    11, 12, 24, 11, 22, 22, 28, 12, 40, 22, 13, 17, 13, 11, 5, 26, 17, 11, 9, 14, 20, 23, 19, 9, 6,
    7, 23, 13, 11, 11, 17, 12, 8, 12, 11, 10, 13, 20, 7, 35, 36, 5, 24, 20, 28, 23, 10, 12, 20, 72,
    13, 19, 16, 8, 18, 12, 13, 17, 7, 18, 52, 17, 16, 15, 5, 23, 11, 13, 12, 9, 9, 5, 8, 28, 22,
    35, 45, 48, 43, 13, 31, 7, 10, 10, 9, 8, 18, 19, 2, 29, 176, 7, 8, 9, 4, 8, 5, 6, 5, 6, 8, 8,
    3, 18, 3, 3, 21, 26, 9, 8, 24, 13, 10, 7, 12, 15, 21, 10, 20, 14, 9, 6, 33, 22, 35, 27, 23, 35,
    27, 36, 18, 32, 31, 28, 25, 35, 33, 33, 28, 24, 29, 30, 31, 29, 35, 34, 28, 28, 27, 28, 27, 33,
    31, 18, 26, 22, 16, 20, 12, 29, 17, 18, 20, 10, 14, 17, 17, 11, 16, 16, 13, 13, 14, 31, 22, 26,
    6, 30, 13, 25, 22, 21, 34, 16, 6, 22, 32, 9, 14, 14, 7, 25, 6, 17, 25, 18, 23, 12, 21, 13, 29,
    24, 33, 9, 20, 24, 17, 10, 22, 38, 22, 8, 31, 29, 25, 28, 28, 25, 13, 15, 22, 26, 11, 23, 15,
    12, 17, 13, 12, 21, 14, 21, 22, 11, 12, 19, 12, 25, 24, 19, 37, 25, 31, 31, 30, 34, 22, 26, 25,
    23, 17, 27, 22, 21, 21, 27, 23, 15, 18, 14, 30, 40, 10, 38, 24, 22, 17, 32, 24, 40, 44, 26, 22,
    19, 32, 21, 28, 18, 16, 18, 22, 13, 30, 5, 28, 7, 47, 39, 46, 64, 34, 22, 22, 66, 22, 22, 28,
    10, 27, 17, 17, 14, 27, 18, 11, 22, 25, 28, 23, 23, 8, 63, 24, 32, 14, 49, 32, 31, 49, 27, 17,
    21, 36, 26, 21, 26, 18, 32, 33, 31, 15, 38, 28, 23, 29, 49, 26, 20, 27, 31, 25, 24, 23, 35, 21,
    49, 30, 37, 31, 28, 28, 27, 27, 21, 45, 13, 11, 23, 5, 19, 15, 11, 16, 14, 17, 15, 12, 14, 16,
    9, 20, 32, 21, 15, 16, 15, 13, 27, 14, 17, 14, 15, 21, 17, 10, 10, 11, 16, 13, 12, 13, 15, 16,
    20, 15, 13, 19, 17, 20, 19, 18, 15, 20, 15, 23, 21, 13, 10, 14, 11, 15, 14, 23, 17, 12, 17, 14,
    9, 21, 14, 17, 18, 6, 25, 23, 17, 25, 48, 34, 29, 34, 38, 42, 30, 50, 58, 36, 39, 28, 27, 35,
    30, 34, 46, 46, 39, 51, 46, 75, 66, 20, 45, 28, 35, 41, 43, 56, 37, 38, 50, 52, 33, 44, 37, 72,
    47, 20, 80, 52, 38, 44, 39, 49, 50, 56, 62, 42, 54, 59, 35, 35, 32, 31, 37, 43, 48, 47, 38, 71,
    56, 53, 51, 25, 36, 54, 47, 71, 53, 59, 41, 42, 57, 50, 38, 31, 27, 33, 26, 40, 42, 31, 25, 26,
    47, 26, 37, 42, 15, 60, 40, 43, 48, 30, 25, 52, 28, 41, 40, 34, 28, 41, 38, 40, 30, 35, 27, 27,
    32, 44, 31, 32, 29, 31, 25, 21, 23, 25, 39, 33, 21, 36, 21, 14, 23, 33, 27, 31, 16, 23, 21, 13,
    20, 40, 13, 27, 33, 34, 31, 13, 40, 58, 24, 24, 17, 18, 18, 21, 18, 16, 24, 15, 18, 33, 21, 14,
    24, 21, 29, 31, 26, 18, 23, 22, 21, 32, 33, 24, 30, 30, 21, 23, 29, 23, 25, 18, 10, 20, 13, 18,
    28, 12, 17, 18, 20, 15, 16, 16, 25, 21, 18, 26, 17, 22, 16, 15, 15, 25, 14, 18, 19, 16, 14, 20,
    28, 13, 28, 39, 40, 29, 25, 27, 26, 18, 17, 20, 25, 25, 22, 19, 14, 21, 22, 18, 10, 29, 24, 21,
    21, 13, 14, 25, 20, 29, 22, 11, 14, 17, 17, 13, 21, 11, 19, 17, 18, 20, 8, 21, 18, 24, 21, 15,
    27, 21,
];

/// The canonical book table, Genesis first.
pub fn books() -> &'static [Book; BOOK_COUNT] {
    &BOOKS
}

/// Looks up a book by its canonical index, returning `None` past Revelation.
pub fn book(index: u8) -> Option<&'static Book> {
    BOOKS.get(index as usize)
}

/// Returns the number of verses in a 1-based chapter of a book.
pub fn verses_in_chapter(book: u8, chapter: u8) -> Option<u16> {
    let entry = self::book(book)?;
    if chapter == 0 || chapter > entry.chapter_count {
        return None;
    }
    let offset = entry.chapter_start as usize + chapter as usize - 1;
    VERSES_PER_CHAPTER.get(offset).copied()
}

/// Converts a book and a 1-based chapter into its global chapter index.
pub fn chapter_offset(book: u8, chapter: u8) -> Option<u16> {
    let entry = self::book(book)?;
    if chapter == 0 || chapter > entry.chapter_count {
        return None;
    }
    Some(entry.chapter_start + chapter as u16 - 1)
}

/// Converts a book, chapter, and 1-based verse into its global verse index.
pub fn verse_offset(book: u8, chapter: u8, verse: u8) -> Option<u16> {
    let entry = self::book(book)?;
    if chapter == 0 || chapter > entry.chapter_count || verse == 0 {
        return None;
    }
    let chapter_index = entry.chapter_start as usize + chapter as usize - 1;
    let count = VERSES_PER_CHAPTER[chapter_index];
    if verse as u16 > count {
        return None;
    }
    let prior: u16 = VERSES_PER_CHAPTER[entry.chapter_start as usize..chapter_index]
        .iter()
        .sum();
    Some(entry.verse_start + prior + verse as u16 - 1)
}
