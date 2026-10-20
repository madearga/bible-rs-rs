// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The canonical grid: 66 books, 1189 chapters, 31102 verses.
//!
//! Every translation converts to this one grid, so a reference points at the
//! same verse whichever translation is open. The book index is the canonical
//! position, Genesis at 0 and Revelation at 65, and chapter and verse numbers
//! are 1-based.
//!
//! The verse count of every chapter is fixed by the KJV versification and never
//! varies by translation, which is what lets the reader align translations
//! verse by verse.

/// Number of books in the canon.
pub const BOOK_COUNT: usize = 66;

/// Number of chapters in the canon.
pub const CHAPTER_COUNT: usize = 1189;

/// Number of verses in the canon.
pub const VERSE_COUNT: usize = 31102;

/// One book of the canon, in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Book {
    /// Canonical index, Genesis `0` through Revelation `65`.
    pub index: u8,
    /// Full name as printed in a reference.
    pub name: &'static str,
    /// Short form a reader is likely to type.
    pub abbreviation: &'static str,
    /// Number of chapters in the book.
    pub chapters: u16,
    /// Index of the book's first chapter in [`VERSES_PER_CHAPTER`].
    pub chapter_start: u16,
    /// Canonical index of the book's first verse.
    pub verse_start: u16,
}

/// A book before the running offsets are worked out.
struct BookSpec {
    name: &'static str,
    abbreviation: &'static str,
    chapters: u16,
}

const BOOK_SPECS: [BookSpec; BOOK_COUNT] = [
    BookSpec {
        name: "Genesis",
        abbreviation: "Gen",
        chapters: 50,
    },
    BookSpec {
        name: "Exodus",
        abbreviation: "Exod",
        chapters: 40,
    },
    BookSpec {
        name: "Leviticus",
        abbreviation: "Lev",
        chapters: 27,
    },
    BookSpec {
        name: "Numbers",
        abbreviation: "Num",
        chapters: 36,
    },
    BookSpec {
        name: "Deuteronomy",
        abbreviation: "Deut",
        chapters: 34,
    },
    BookSpec {
        name: "Joshua",
        abbreviation: "Josh",
        chapters: 24,
    },
    BookSpec {
        name: "Judges",
        abbreviation: "Judg",
        chapters: 21,
    },
    BookSpec {
        name: "Ruth",
        abbreviation: "Ruth",
        chapters: 4,
    },
    BookSpec {
        name: "1 Samuel",
        abbreviation: "1 Sam",
        chapters: 31,
    },
    BookSpec {
        name: "2 Samuel",
        abbreviation: "2 Sam",
        chapters: 24,
    },
    BookSpec {
        name: "1 Kings",
        abbreviation: "1 Kgs",
        chapters: 22,
    },
    BookSpec {
        name: "2 Kings",
        abbreviation: "2 Kgs",
        chapters: 25,
    },
    BookSpec {
        name: "1 Chronicles",
        abbreviation: "1 Chr",
        chapters: 29,
    },
    BookSpec {
        name: "2 Chronicles",
        abbreviation: "2 Chr",
        chapters: 36,
    },
    BookSpec {
        name: "Ezra",
        abbreviation: "Ezra",
        chapters: 10,
    },
    BookSpec {
        name: "Nehemiah",
        abbreviation: "Neh",
        chapters: 13,
    },
    BookSpec {
        name: "Esther",
        abbreviation: "Esth",
        chapters: 10,
    },
    BookSpec {
        name: "Job",
        abbreviation: "Job",
        chapters: 42,
    },
    BookSpec {
        name: "Psalms",
        abbreviation: "Ps",
        chapters: 150,
    },
    BookSpec {
        name: "Proverbs",
        abbreviation: "Prov",
        chapters: 31,
    },
    BookSpec {
        name: "Ecclesiastes",
        abbreviation: "Eccl",
        chapters: 12,
    },
    BookSpec {
        name: "Song of Solomon",
        abbreviation: "Song",
        chapters: 8,
    },
    BookSpec {
        name: "Isaiah",
        abbreviation: "Isa",
        chapters: 66,
    },
    BookSpec {
        name: "Jeremiah",
        abbreviation: "Jer",
        chapters: 52,
    },
    BookSpec {
        name: "Lamentations",
        abbreviation: "Lam",
        chapters: 5,
    },
    BookSpec {
        name: "Ezekiel",
        abbreviation: "Ezek",
        chapters: 48,
    },
    BookSpec {
        name: "Daniel",
        abbreviation: "Dan",
        chapters: 12,
    },
    BookSpec {
        name: "Hosea",
        abbreviation: "Hos",
        chapters: 14,
    },
    BookSpec {
        name: "Joel",
        abbreviation: "Joel",
        chapters: 3,
    },
    BookSpec {
        name: "Amos",
        abbreviation: "Amos",
        chapters: 9,
    },
    BookSpec {
        name: "Obadiah",
        abbreviation: "Obad",
        chapters: 1,
    },
    BookSpec {
        name: "Jonah",
        abbreviation: "Jonah",
        chapters: 4,
    },
    BookSpec {
        name: "Micah",
        abbreviation: "Mic",
        chapters: 7,
    },
    BookSpec {
        name: "Nahum",
        abbreviation: "Nah",
        chapters: 3,
    },
    BookSpec {
        name: "Habakkuk",
        abbreviation: "Hab",
        chapters: 3,
    },
    BookSpec {
        name: "Zephaniah",
        abbreviation: "Zeph",
        chapters: 3,
    },
    BookSpec {
        name: "Haggai",
        abbreviation: "Hag",
        chapters: 2,
    },
    BookSpec {
        name: "Zechariah",
        abbreviation: "Zech",
        chapters: 14,
    },
    BookSpec {
        name: "Malachi",
        abbreviation: "Mal",
        chapters: 4,
    },
    BookSpec {
        name: "Matthew",
        abbreviation: "Matt",
        chapters: 28,
    },
    BookSpec {
        name: "Mark",
        abbreviation: "Mark",
        chapters: 16,
    },
    BookSpec {
        name: "Luke",
        abbreviation: "Luke",
        chapters: 24,
    },
    BookSpec {
        name: "John",
        abbreviation: "John",
        chapters: 21,
    },
    BookSpec {
        name: "Acts",
        abbreviation: "Acts",
        chapters: 28,
    },
    BookSpec {
        name: "Romans",
        abbreviation: "Rom",
        chapters: 16,
    },
    BookSpec {
        name: "1 Corinthians",
        abbreviation: "1 Cor",
        chapters: 16,
    },
    BookSpec {
        name: "2 Corinthians",
        abbreviation: "2 Cor",
        chapters: 13,
    },
    BookSpec {
        name: "Galatians",
        abbreviation: "Gal",
        chapters: 6,
    },
    BookSpec {
        name: "Ephesians",
        abbreviation: "Eph",
        chapters: 6,
    },
    BookSpec {
        name: "Philippians",
        abbreviation: "Phil",
        chapters: 4,
    },
    BookSpec {
        name: "Colossians",
        abbreviation: "Col",
        chapters: 4,
    },
    BookSpec {
        name: "1 Thessalonians",
        abbreviation: "1 Thess",
        chapters: 5,
    },
    BookSpec {
        name: "2 Thessalonians",
        abbreviation: "2 Thess",
        chapters: 3,
    },
    BookSpec {
        name: "1 Timothy",
        abbreviation: "1 Tim",
        chapters: 6,
    },
    BookSpec {
        name: "2 Timothy",
        abbreviation: "2 Tim",
        chapters: 4,
    },
    BookSpec {
        name: "Titus",
        abbreviation: "Titus",
        chapters: 3,
    },
    BookSpec {
        name: "Philemon",
        abbreviation: "Phlm",
        chapters: 1,
    },
    BookSpec {
        name: "Hebrews",
        abbreviation: "Heb",
        chapters: 13,
    },
    BookSpec {
        name: "James",
        abbreviation: "Jas",
        chapters: 5,
    },
    BookSpec {
        name: "1 Peter",
        abbreviation: "1 Pet",
        chapters: 5,
    },
    BookSpec {
        name: "2 Peter",
        abbreviation: "2 Pet",
        chapters: 3,
    },
    BookSpec {
        name: "1 John",
        abbreviation: "1 John",
        chapters: 5,
    },
    BookSpec {
        name: "2 John",
        abbreviation: "2 John",
        chapters: 1,
    },
    BookSpec {
        name: "3 John",
        abbreviation: "3 John",
        chapters: 1,
    },
    BookSpec {
        name: "Jude",
        abbreviation: "Jude",
        chapters: 1,
    },
    BookSpec {
        name: "Revelation",
        abbreviation: "Rev",
        chapters: 22,
    },
];

/// Verse count of every chapter in canonical order, indexed by
/// [`Book::chapter_start`] plus a zero-based chapter offset.
///
/// This table is the grid itself: the binary format stores it verbatim so a
/// file can be checked against the canon before it is trusted.
pub static VERSES_PER_CHAPTER: [u16; CHAPTER_COUNT] = [
    31, 25, 24, 26, 32, 22, 24, 22, 29, 32, 32, 20, 18, 24, 21, 16, 27, 33, 38, 18, //
    34, 24, 20, 67, 34, 35, 46, 22, 35, 43, 55, 32, 20, 31, 29, 43, 36, 30, 23, 23, //
    57, 38, 34, 34, 28, 34, 31, 22, 33, 26, // Genesis
    22, 25, 22, 31, 23, 30, 25, 32, 35, 29, //
    10, 51, 22, 31, 27, 36, 16, 27, 25, 26, //
    36, 31, 33, 18, 40, 37, 21, 43, 46, 38, //
    18, 35, 23, 35, 35, 38, 29, 31, 43, 38, // Exodus
    17, 16, 17, 35, 19, 30, 38, 36, 24, 20, //
    47, 8, 59, 57, 33, 34, 16, 30, 37, 27, //
    24, 33, 44, 23, 55, 46, 34, // Leviticus
    54, 34, 51, 49, 31, 27, 89, 26, 23, 36, //
    35, 16, 33, 45, 41, 50, 13, 32, 22, 29, //
    35, 41, 30, 25, 18, 65, 23, 31, 40, 16, //
    54, 42, 56, 29, 34, 13, // Numbers
    46, 37, 29, 49, 33, 25, 26, 20, 29, 22, //
    32, 32, 18, 29, 23, 22, 20, 22, 21, 20, //
    23, 30, 25, 22, 19, 19, 26, 68, 29, 20, //
    30, 52, 29, 12, // Deuteronomy
    18, 24, 17, 24, 15, 27, 26, 35, 27, 43, //
    23, 24, 33, 15, 63, 10, 18, 28, 51, 9, //
    45, 34, 16, 33, // Joshua
    36, 23, 31, 24, 31, 40, 25, 35, 57, 18, //
    40, 15, 25, 20, 20, 31, 13, 31, 30, 48, //
    25, // Judges
    22, 23, 18, 22, // Ruth
    28, 36, 21, 22, 12, 21, 17, 22, 27, 27, //
    15, 25, 23, 52, 35, 23, 58, 30, 24, 42, //
    15, 23, 29, 22, 44, 25, 12, 25, 11, 31, //
    13, // 1 Samuel
    27, 32, 39, 12, 25, 23, 29, 18, 13, 19, //
    27, 31, 39, 33, 37, 23, 29, 33, 43, 26, //
    22, 51, 39, 25, // 2 Samuel
    53, 46, 28, 34, 18, 38, 51, 66, 28, 29, //
    43, 33, 34, 31, 34, 34, 24, 46, 21, 43, //
    29, 53, // 1 Kings
    18, 25, 27, 44, 27, 33, 20, 29, 37, 36, //
    21, 21, 25, 29, 38, 20, 41, 37, 37, 21, //
    26, 20, 37, 20, 30, // 2 Kings
    54, 55, 24, 43, 26, 81, 40, 40, 44, 14, //
    47, 40, 14, 17, 29, 43, 27, 17, 19, 8, //
    30, 19, 32, 31, 31, 32, 34, 21, 30, // 1 Chronicles
    17, 18, 17, 22, 14, 42, 22, 18, 31, 19, //
    23, 16, 22, 15, 19, 14, 19, 34, 11, 37, //
    20, 12, 21, 27, 28, 23, 9, 27, 36, 27, //
    21, 33, 25, 33, 27, 23, // 2 Chronicles
    11, 70, 13, 24, 17, 22, 28, 36, 15, 44, // Ezra
    11, 20, 32, 23, 19, 19, 73, 18, 38, 39, //
    36, 47, 31, // Nehemiah
    22, 23, 15, 17, 14, 14, 10, 17, 32, 3, // Esther
    22, 13, 26, 21, 27, 30, 21, 22, 35, 22, //
    20, 25, 28, 22, 35, 22, 16, 21, 29, 29, //
    34, 30, 17, 25, 6, 14, 23, 28, 25, 31, //
    40, 22, 33, 37, 16, 33, 24, 41, 30, 24, //
    34, 17, // Job
    6, 12, 8, 8, 12, 10, 17, 9, 20, 18, //
    7, 8, 6, 7, 5, 11, 15, 50, 14, 9, //
    13, 31, 6, 10, 22, 12, 14, 9, 11, 12, //
    24, 11, 22, 22, 28, 12, 40, 22, 13, 17, //
    13, 11, 5, 26, 17, 11, 9, 14, 20, 23, //
    19, 9, 6, 7, 23, 13, 11, 11, 17, 12, //
    8, 12, 11, 10, 13, 20, 7, 35, 36, 5, //
    24, 20, 28, 23, 10, 12, 20, 72, 13, 19, //
    16, 8, 18, 12, 13, 17, 7, 18, 52, 17, //
    16, 15, 5, 23, 11, 13, 12, 9, 9, 5, //
    8, 28, 22, 35, 45, 48, 43, 13, 31, 7, //
    10, 10, 9, 8, 18, 19, 2, 29, 176, 7, //
    8, 9, 4, 8, 5, 6, 5, 6, 8, 8, //
    3, 18, 3, 3, 21, 26, 9, 8, 24, 13, //
    10, 7, 12, 15, 21, 10, 20, 14, 9, 6, // Psalms
    33, 22, 35, 27, 23, 35, 27, 36, 18, 32, //
    31, 28, 25, 35, 33, 33, 28, 24, 29, 30, //
    31, 29, 35, 34, 28, 28, 27, 28, 27, 33, //
    31, // Proverbs
    18, 26, 22, 16, 20, 12, 29, 17, 18, 20, //
    10, 14, // Ecclesiastes
    17, 17, 11, 16, 16, 13, 13, 14, // Song of Solomon
    31, 22, 26, 6, 30, 13, 25, 22, 21, 34, //
    16, 6, 22, 32, 9, 14, 14, 7, 25, 6, //
    17, 25, 18, 23, 12, 21, 13, 29, 24, 33, //
    9, 20, 24, 17, 10, 22, 38, 22, 8, 31, //
    29, 25, 28, 28, 25, 13, 15, 22, 26, 11, //
    23, 15, 12, 17, 13, 12, 21, 14, 21, 22, //
    11, 12, 19, 12, 25, 24, // Isaiah
    19, 37, 25, 31, 31, 30, 34, 22, 26, 25, //
    23, 17, 27, 22, 21, 21, 27, 23, 15, 18, //
    14, 30, 40, 10, 38, 24, 22, 17, 32, 24, //
    40, 44, 26, 22, 19, 32, 21, 28, 18, 16, //
    18, 22, 13, 30, 5, 28, 7, 47, 39, 46, //
    64, 34, // Jeremiah
    22, 22, 66, 22, 22, // Lamentations
    28, 10, 27, 17, 17, 14, 27, 18, 11, 22, //
    25, 28, 23, 23, 8, 63, 24, 32, 14, 49, //
    32, 31, 49, 27, 17, 21, 36, 26, 21, 26, //
    18, 32, 33, 31, 15, 38, 28, 23, 29, 49, //
    26, 20, 27, 31, 25, 24, 23, 35, // Ezekiel
    21, 49, 30, 37, 31, 28, 28, 27, 27, 21, //
    45, 13, // Daniel
    11, 23, 5, 19, 15, 11, 16, 14, 17, 15, //
    12, 14, 16, 9, // Hosea
    20, 32, 21, // Joel
    15, 16, 15, 13, 27, 14, 17, 14, 15, // Amos
    21, // Obadiah
    17, 10, 10, 11, // Jonah
    16, 13, 12, 13, 15, 16, 20, // Micah
    15, 13, 19, // Nahum
    17, 20, 19, // Habakkuk
    18, 15, 20, // Zephaniah
    15, 23, // Haggai
    21, 13, 10, 14, 11, 15, 14, 23, 17, 12, //
    17, 14, 9, 21, // Zechariah
    14, 17, 18, 6, // Malachi
    25, 23, 17, 25, 48, 34, 29, 34, 38, 42, //
    30, 50, 58, 36, 39, 28, 27, 35, 30, 34, //
    46, 46, 39, 51, 46, 75, 66, 20, // Matthew
    45, 28, 35, 41, 43, 56, 37, 38, 50, 52, //
    33, 44, 37, 72, 47, 20, // Mark
    80, 52, 38, 44, 39, 49, 50, 56, 62, 42, //
    54, 59, 35, 35, 32, 31, 37, 43, 48, 47, //
    38, 71, 56, 53, // Luke
    51, 25, 36, 54, 47, 71, 53, 59, 41, 42, //
    57, 50, 38, 31, 27, 33, 26, 40, 42, 31, //
    25, // John
    26, 47, 26, 37, 42, 15, 60, 40, 43, 48, //
    30, 25, 52, 28, 41, 40, 34, 28, 41, 38, //
    40, 30, 35, 27, 27, 32, 44, 31, // Acts
    32, 29, 31, 25, 21, 23, 25, 39, 33, 21, //
    36, 21, 14, 23, 33, 27, // Romans
    31, 16, 23, 21, 13, 20, 40, 13, 27, 33, //
    34, 31, 13, 40, 58, 24, // 1 Corinthians
    24, 17, 18, 18, 21, 18, 16, 24, 15, 18, //
    33, 21, 14, // 2 Corinthians
    24, 21, 29, 31, 26, 18, // Galatians
    23, 22, 21, 32, 33, 24, // Ephesians
    30, 30, 21, 23, // Philippians
    29, 23, 25, 18, // Colossians
    10, 20, 13, 18, 28, // 1 Thessalonians
    12, 17, 18, // 2 Thessalonians
    20, 15, 16, 16, 25, 21, // 1 Timothy
    18, 26, 17, 22, // 2 Timothy
    16, 15, 15, // Titus
    25, // Philemon
    14, 18, 19, 16, 14, 20, 28, 13, 28, 39, //
    40, 29, 25, // Hebrews
    27, 26, 18, 17, 20, // James
    25, 25, 22, 19, 14, // 1 Peter
    21, 22, 18, // 2 Peter
    10, 29, 24, 21, 21, // 1 John
    13, // 2 John
    14, // 3 John
    25, // Jude
    20, 29, 22, 11, 14, 17, 17, 13, 21, 11, //
    19, 17, 18, 20, 8, 21, 18, 24, 21, 15, //
    27, 21, // Revelation
];

/// Every book, in canonical order.
pub static BOOKS: [Book; BOOK_COUNT] = build_books();

const fn build_books() -> [Book; BOOK_COUNT] {
    let mut books = [Book {
        index: 0,
        name: "",
        abbreviation: "",
        chapters: 0,
        chapter_start: 0,
        verse_start: 0,
    }; BOOK_COUNT];
    let mut chapter_start: u16 = 0;
    let mut verse_start: u16 = 0;
    let mut i = 0;
    while i < BOOK_COUNT {
        let spec = &BOOK_SPECS[i];
        books[i] = Book {
            index: i as u8,
            name: spec.name,
            abbreviation: spec.abbreviation,
            chapters: spec.chapters,
            chapter_start,
            verse_start,
        };
        chapter_start += spec.chapters;
        let mut chapter = 0;
        while chapter < spec.chapters {
            verse_start += VERSES_PER_CHAPTER[(books[i].chapter_start + chapter) as usize];
            chapter += 1;
        }
        i += 1;
    }
    books
}

/// A spelling a reader may type that is not a book name or abbreviation.
const ALIASES: &[(&str, u8)] = &[
    ("gen", 0),
    ("ge", 0),
    ("ex", 1),
    ("exo", 1),
    ("lv", 2),
    ("nm", 3),
    ("nu", 3),
    ("dt", 4),
    ("jdg", 6),
    ("jg", 6),
    ("jud", 6),
    ("ru", 7),
    ("1sa", 8),
    ("1sam", 8),
    ("2sa", 9),
    ("2sam", 9),
    ("1kg", 10),
    ("1ki", 10),
    ("1kgs", 10),
    ("2kg", 11),
    ("2ki", 11),
    ("2kgs", 11),
    ("1ch", 12),
    ("1chr", 12),
    ("2ch", 13),
    ("2chr", 13),
    ("ezr", 14),
    ("est", 16),
    ("jb", 17),
    ("ps", 18),
    ("psa", 18),
    ("psalm", 18),
    ("psalms", 18),
    ("pr", 19),
    ("ec", 20),
    ("ecc", 20),
    ("qoh", 20),
    ("song", 21),
    ("songs", 21),
    ("sos", 21),
    ("canticles", 21),
    ("is", 22),
    ("eze", 25),
    ("dn", 26),
    ("joe", 28),
    ("jl", 28),
    ("am", 29),
    ("amo", 29),
    ("ob", 30),
    ("oba", 30),
    ("jon", 31),
    ("mi", 32),
    ("na", 33),
    ("zep", 35),
    ("zec", 37),
    ("mt", 39),
    ("mk", 40),
    ("mrk", 40),
    ("lk", 41),
    ("luk", 41),
    ("jn", 42),
    ("jhn", 42),
    ("joh", 42),
    ("ac", 43),
    ("ro", 44),
    ("1co", 45),
    ("1cor", 45),
    ("2co", 46),
    ("2cor", 46),
    ("ga", 47),
    ("php", 49),
    ("1th", 51),
    ("1thess", 51),
    ("2th", 52),
    ("2thess", 52),
    ("1ti", 53),
    ("1tim", 53),
    ("2ti", 54),
    ("2tim", 54),
    ("tit", 55),
    ("phm", 56),
    ("phlm", 56),
    ("jas", 58),
    ("jam", 58),
    ("1pe", 59),
    ("1pet", 59),
    ("2pe", 60),
    ("2pet", 60),
    ("1jn", 61),
    ("1john", 61),
    ("2jn", 62),
    ("2john", 62),
    ("3jn", 63),
    ("3john", 63),
    ("jude", 64),
    ("re", 65),
    ("rev", 65),
];

/// The book at a canonical index, or `None` when the index is past Revelation.
pub fn book(index: u8) -> Option<&'static Book> {
    BOOKS.get(usize::from(index))
}

/// Number of verses in one chapter, or `None` when the chapter does not exist.
pub fn verses_in_chapter(book: u8, chapter: u16) -> Option<u16> {
    let book = self::book(book)?;
    if chapter == 0 || chapter > book.chapters {
        return None;
    }
    Some(VERSES_PER_CHAPTER[usize::from(book.chapter_start + chapter - 1)])
}

/// Canonical index of the first verse in a chapter, or `None` when the chapter
/// does not exist.
pub fn chapter_verse_start(book: u8, chapter: u16) -> Option<u32> {
    let book = self::book(book)?;
    if chapter == 0 || chapter > book.chapters {
        return None;
    }
    let mut start = u32::from(book.verse_start);
    for offset in 0..chapter - 1 {
        start += u32::from(VERSES_PER_CHAPTER[usize::from(book.chapter_start + offset)]);
    }
    Some(start)
}

/// Number of verses in a whole book.
pub fn book_verse_count(book: u8) -> u16 {
    let start = self::book(book).map_or(0, |book| u32::from(book.verse_start));
    let next = self::book(book.saturating_add(1))
        .map_or(VERSE_COUNT as u32, |next| u32::from(next.verse_start));
    u16::try_from(next - start).unwrap_or(0)
}

/// Resolves the book part of a reference: an exact name, abbreviation, or alias
/// first, then a spelling within two edits of one.
pub fn find_book(text: &str) -> Option<&'static Book> {
    let needle = normalize(text);
    if needle.is_empty() {
        return None;
    }

    let exact = BOOKS
        .iter()
        .find(|book| normalize(book.name) == needle || normalize(book.abbreviation) == needle)
        .or_else(|| {
            ALIASES
                .iter()
                .find(|(alias, _)| *alias == needle)
                .and_then(|(_, index)| self::book(*index))
        });
    if let Some(book) = exact {
        return Some(book);
    }

    if needle.len() < 2 {
        return None;
    }
    let prefix = BOOKS.iter().find(|book| {
        let name = normalize(book.name);
        let abbreviation = normalize(book.abbreviation);
        (name.starts_with(&needle) && needle.len() >= 3) || abbreviation.starts_with(&needle)
    });
    if let Some(book) = prefix {
        return Some(book);
    }

    if needle.chars().count() < 4 {
        return None;
    }
    let mut best: Option<(usize, usize, u8)> = None;
    for book in &BOOKS {
        for candidate in [normalize(book.name), normalize(book.abbreviation)] {
            let distance = edit_distance(&needle, &candidate);
            if distance > 2 {
                continue;
            }
            let key = (
                distance,
                usize::MAX - common_prefix(&needle, &candidate),
                book.index,
            );
            if best.is_none_or(|current| key < current) {
                best = Some(key);
            }
        }
    }
    best.and_then(|(_, _, index)| self::book(index))
}

/// Lower-cased, space-free form used for every book comparison.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace() && *character != '.')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Longest common prefix in characters.
fn common_prefix(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(a, b)| a == b)
        .count()
}

/// Levenshtein distance, bounded by the caller's tolerance in practice because
/// both sides are short book names.
fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];
    for (row, left_char) in left.chars().enumerate() {
        current[0] = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current[column + 1] = substitution
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_match_the_canon() {
        assert_eq!(BOOKS.len(), BOOK_COUNT);
        assert_eq!(BOOK_COUNT, 66);
        let chapters: u16 = BOOKS.iter().map(|book| book.chapters).sum();
        assert_eq!(usize::from(chapters), CHAPTER_COUNT);
        assert_eq!(VERSES_PER_CHAPTER.len(), CHAPTER_COUNT);
        let verses: u16 = VERSES_PER_CHAPTER.iter().sum();
        assert_eq!(usize::from(verses), VERSE_COUNT);
    }

    #[test]
    fn book_offsets_follow_the_grid() {
        let mut chapter_start = 0;
        let mut verse_start = 0u32;
        for (index, book) in BOOKS.iter().enumerate() {
            assert_eq!(usize::from(book.index), index);
            assert_eq!(usize::from(book.chapter_start), chapter_start);
            assert_eq!(u32::from(book.verse_start), verse_start);
            chapter_start += usize::from(book.chapters);
            verse_start += u32::from(book_verse_count(book.index));
        }
        assert_eq!(chapter_start, CHAPTER_COUNT);
        assert_eq!(verse_start as usize, VERSE_COUNT);
    }

    #[test]
    fn chapters_and_verses_are_one_based_and_bounded() {
        assert_eq!(verses_in_chapter(0, 1), Some(31));
        assert_eq!(verses_in_chapter(0, 0), None);
        assert_eq!(verses_in_chapter(0, 51), None);
        assert_eq!(verses_in_chapter(65, 22), Some(21));
        assert_eq!(chapter_verse_start(0, 1), Some(0));
        assert_eq!(chapter_verse_start(0, 2), Some(31));
        assert_eq!(chapter_verse_start(65, 22), Some(31081));
        assert_eq!(book_verse_count(0), 1533);
        assert_eq!(book_verse_count(65), 404);
    }

    #[test]
    fn books_resolve_by_name_abbreviation_and_typo() {
        assert_eq!(find_book("Genesis").map(|book| book.index), Some(0));
        assert_eq!(find_book("ps").map(|book| book.index), Some(18));
        assert_eq!(find_book("1 Cor").map(|book| book.index), Some(45));
        assert_eq!(find_book("revalation").map(|book| book.index), Some(65));
        assert_eq!(find_book("jn").map(|book| book.index), Some(42));
    }
}
