// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The build tasks for the Bible reader.
//!
//! Three tasks live here.
//!
//! `build-data` fetches the seven public-domain USFM archives from eBible.org,
//! checks every archive against the SHA-256 pinned in this file, imports it with
//! the same importer users get, and writes the binary translation files that ship
//! with the app. Nothing is written until every archive has passed its digest
//! check and every translation has converted, so a failure never leaves a partial
//! or an unverified file behind.
//!
//! `verify-data` reads the written files back and checks the grid, the first
//! verse of Genesis, the last verse of Revelation, and the archive each file came
//! from.
//!
//! `sample-bbl` regenerates the committed synthetic sample page from filler
//! words, so the sample has a generator in the repository and the committed blob
//! is exactly its output.
//!
//! ```text
//! cargo run -p xtask -- build-data
//! cargo run -p xtask -- verify-data
//! cargo run -p xtask -- sample-bbl
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use bible_core::format::{decode, encode};
use bible_core::{
    BOOK_COUNT, MarkKind, SpanStyle, TranslationBuilder, TranslationMeta, VERSE_COUNT, books,
    import::import_path,
};
use sha2::{Digest, Sha256};

/// Where the public-domain archives are published.
const SCRIPTURES: &str = "https://ebible.org/Scriptures";

/// Where downloaded archives are kept. Under `target`, so git never sees them.
const CACHE_DIR: &str = "target/data-cache";

/// Where the built translation files live.
const OUT_DIR: &str = "data/translations";

/// The committed synthetic sample page.
const SAMPLE_PATH: &str = "assets/sample/passage.bbl";

/// The zstd level the bundled data is compressed with.
///
/// The bundled files are shipped inside the app and decoded on first use, so the
/// build spends the time to get the smallest body it can.
const BUNDLED_LEVEL: i32 = 19;

/// One bundled translation and everything needed to fetch, verify, and check it.
struct Source {
    /// Identifier used for the file name, the metadata, and settings.
    id: &'static str,
    /// Short label shown in the translation list.
    abbreviation: &'static str,
    /// Full display name.
    name: &'static str,
    /// Language the text is in.
    language: &'static str,
    /// Year of the edition that was imported.
    edition: &'static str,
    /// One-line licence summary shown to the reader.
    licence: &'static str,
    /// Copyright notice that travels with the text.
    copyright: &'static str,
    /// File stem of the archive on eBible.org, without the `_usfm.zip` suffix.
    archive: &'static str,
    /// SHA-256 of the archive, checked before the archive is used.
    sha256: &'static str,
    /// Genesis 1:1 exactly as this translation prints it.
    genesis: &'static str,
    /// Revelation 22:21 exactly as this translation prints it.
    revelation: &'static str,
}

/// The seven public-domain translations that ship with the app.
///
/// The archive URLs were resolved against eBible.org: the English identifiers
/// carry an `eng` prefix and are not the same as the app's short identifiers.
const SOURCES: [Source; 7] = [
    Source {
        id: "kjv",
        abbreviation: "KJV",
        name: "King James Version",
        language: "English",
        edition: "1611",
        licence: "Public domain",
        copyright: "Public domain. The Authorized (King James) Version of 1611 is in the \
                    public domain; in the United Kingdom the Crown holds a letters patent \
                    over printing it.",
        archive: "eng-kjv2006",
        sha256: "16b08c9248b3600662f66f1e055f61fc3a6c7275e347236ee45feb35dfb20cb3",
        genesis: "In the beginning God created the heaven and the earth.",
        revelation: "The grace of our Lord Jesus Christ be with you all. Amen.",
    },
    Source {
        id: "web",
        abbreviation: "WEB",
        name: "World English Bible",
        language: "English",
        edition: "2000",
        licence: "Public domain",
        copyright: "Public domain. The World English Bible was dedicated to the public \
                    domain by its publisher, eBible.org.",
        archive: "engwebp",
        sha256: "732c17d3325563fc5d8d98157a6c3d128c887f6b8e5a5ad0f71f58e5d5eebfcb",
        genesis: "In the beginning, God created the heavens and the earth.",
        revelation: "The grace of the Lord Jesus Christ be with all the saints. Amen.",
    },
    Source {
        id: "bsb",
        abbreviation: "BSB",
        name: "Berean Standard Bible",
        language: "English",
        edition: "2020",
        licence: "Public domain",
        copyright: "Public domain. The Berean Standard Bible was dedicated to the public \
                    domain by its publisher.",
        archive: "engbsb",
        sha256: "e4830de52921a22c90b7fef725b2490f3b90c16a3dd8f800b82c859814e52f96",
        genesis: "In the beginning God created the heavens and the earth.",
        revelation: "The grace of the Lord Jesus be with all the saints. Amen.",
    },
    Source {
        id: "asv",
        abbreviation: "ASV",
        name: "American Standard Version",
        language: "English",
        edition: "1901",
        licence: "Public domain",
        copyright: "Public domain. The American Standard Version of 1901 is in the public \
                    domain.",
        archive: "eng-asv",
        sha256: "fa116257643253288c48a46f499faaaa501dfdd8381c78e1c67b745a9e1a999a",
        genesis: "In the beginning God created the heavens and the earth.",
        revelation: "The grace of the Lord Jesus be with the saints. Amen.",
    },
    Source {
        id: "ylt",
        abbreviation: "YLT",
        name: "Young's Literal Translation",
        language: "English",
        edition: "1862",
        licence: "Public domain",
        copyright: "Public domain. Young's Literal Translation of 1862 is in the public \
                    domain.",
        archive: "engylt",
        sha256: "63ef17706cc59cdbf7921627a40dc819e59ae2903d6f85ab23efc49d6da723f2",
        genesis: "In the beginning of God's preparing the heavens and the earth —",
        revelation: "The grace of our Lord Jesus Christ [is] with you all. Amen.",
    },
    Source {
        id: "darby",
        abbreviation: "DARBY",
        name: "Darby Bible",
        language: "English",
        edition: "1890",
        licence: "Public domain",
        copyright: "Public domain. The Darby Translation of 1890 is in the public domain.",
        archive: "engDBY",
        sha256: "2aadc6df9a99c04e32a9a167a708f8c25614fc781f3ff8231787f7630ca2d000",
        genesis: "In the beginning God created the heavens and the earth.",
        revelation: "The grace of the Lord Jesus Christ [be] with all the saints.",
    },
    Source {
        id: "webster",
        abbreviation: "WEBSTER",
        name: "Webster Bible",
        language: "English",
        edition: "1833",
        licence: "Public domain",
        copyright: "Public domain. Noah Webster's revision of 1833 is in the public domain.",
        archive: "engwebster",
        sha256: "34c9b0c8272c00f183de9f856ded411c70e22b148f5d164543f20e2987a9a5c7",
        genesis: "In the beginning God created the heaven and the earth.",
        revelation: "The grace of our Lord Jesus Christ be with you all. Amen.",
    },
];

fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("build-data") => build_data(),
        Some("verify-data") => verify_data(),
        Some("sample-bbl") => sample_bbl(),
        Some(other) => {
            print_usage();
            bail!("unknown task {other}")
        }
        None => {
            print_usage();
            bail!("a task is required")
        }
    }
}

/// Prints the tasks this binary runs.
fn print_usage() {
    eprintln!(
        "usage: cargo run -p xtask -- <task>\n\
         \n\
         tasks:\n\
         \x20 build-data   fetch, verify, import, and write the seven bundled translations\n\
         \x20 verify-data  decode the written translations and check the grid, witness verses, and archive digests\n\
         \x20 sample-bbl   regenerate assets/sample/passage.bbl from filler words"
    );
}

/// The repository root, found from this crate's manifest directory.
fn repo_root() -> Result<PathBuf> {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate_dir
        .parent()
        .context("the xtask crate has no parent directory")?;
    Ok(root.to_path_buf())
}

/// The URL an archive is fetched from.
fn archive_url(source: &Source) -> String {
    format!("{SCRIPTURES}/{}_usfm.zip", source.archive)
}

/// Builds every bundled translation from the pinned archives.
///
/// The task runs in two passes on purpose. The first pass downloads and verifies
/// all seven archives, and the second converts and writes. A digest mismatch
/// therefore stops the build before a single file is written, so no partial and
/// no unverified data can appear in `data/translations`.
fn build_data() -> Result<()> {
    let root = repo_root()?;
    let cache = root.join(CACHE_DIR);
    let out = root.join(OUT_DIR);
    fs::create_dir_all(&cache)
        .with_context(|| format!("cannot create the cache folder {}", cache.display()))?;
    fs::create_dir_all(&out)
        .with_context(|| format!("cannot create the output folder {}", out.display()))?;

    let mut verified = Vec::with_capacity(SOURCES.len());
    for source in &SOURCES {
        let path = fetch(source, &cache)?;
        println!(
            "verified {:<8} {} sha256 {}",
            source.id,
            archive_url(source),
            source.sha256
        );
        verified.push((source, path));
    }

    let mut built = Vec::with_capacity(verified.len());
    for (source, path) in verified {
        let meta = meta_for(source);
        let translation = import_path(&path, meta, None).with_context(|| {
            format!(
                "{} is not importable from {}",
                source.name,
                archive_url(source)
            )
        })?;
        if translation.verses().len() != VERSE_COUNT {
            bail!(
                "{} imported {} verses, the canonical grid has {VERSE_COUNT}",
                source.name,
                translation.verses().len()
            );
        }
        let bytes = encode_at_level(&translation, BUNDLED_LEVEL)
            .with_context(|| format!("{} cannot be encoded", source.name))?;
        built.push((source, bytes));
    }

    let mut total = 0usize;
    for (source, bytes) in &built {
        let path = out.join(format!("{}.bbl", source.id));
        write_atomically(&path, bytes)?;
        total += bytes.len();
        println!(
            "wrote    {:<8} {} ({} bytes)",
            source.id,
            path.display(),
            bytes.len()
        );
    }

    write_readme(&root)?;
    println!("built {} translations, {total} bytes in total", built.len());
    Ok(())
}

/// Encodes a translation with the core encoder and recompresses its body at
/// `level`.
///
/// `bible_core::format::encode` fixes the zstd level for the reader's own saves,
/// which is the right choice there because a user waits for it. The bundled data
/// is written once by this task and read many times by the app, so it is worth a
/// slow setting. The column layout, the header, and the version still come from
/// the core encoder; only the compressed bytes are replaced, and the body length
/// in the header is updated to match. When the core encoder grows a level
/// argument this function goes away in favour of it.
fn encode_at_level(translation: &bible_core::Translation, level: i32) -> Result<Vec<u8>> {
    let encoded = encode(translation)?;
    let meta_len = u32::from_le_bytes(
        encoded
            .get(6..10)
            .context("the encoded file has no metadata length")?
            .try_into()
            .expect("four bytes"),
    ) as usize;
    let body_len_at = 10usize
        .checked_add(meta_len)
        .context("the metadata length overflows")?;
    let body_len = u32::from_le_bytes(
        encoded
            .get(body_len_at..body_len_at + 4)
            .context("the encoded file has no body length")?
            .try_into()
            .expect("four bytes"),
    ) as usize;
    let compressed = encoded
        .get(body_len_at + 4..)
        .context("the encoded file has no body")?;
    let body = zstd::stream::decode_all(compressed).context("the encoded body does not decode")?;
    if body.len() != body_len {
        bail!(
            "the encoded body is {} bytes but the header says {body_len}",
            body.len()
        );
    }

    let compressed = zstd::bulk::compress(&body, level)
        .with_context(|| format!("the body cannot be compressed at level {level}"))?;
    let mut out = Vec::with_capacity(body_len_at + 4 + compressed.len());
    out.extend_from_slice(&encoded[..body_len_at]);
    out.extend_from_slice(&(body_len as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// The metadata the app shows for a bundled translation.
///
/// The per-book titles are left empty here because the importer fills them from
/// the source's own book titles, and the red-letter, paragraph, and heading flags
/// start false because the importer raises them when the source carries the
/// markers.
fn meta_for(source: &Source) -> TranslationMeta {
    TranslationMeta {
        id: source.id.to_owned(),
        abbreviation: source.abbreviation.to_owned(),
        name: source.name.to_owned(),
        language: source.language.to_owned(),
        licence: source.licence.to_owned(),
        copyright: source.copyright.to_owned(),
        provenance: archive_url(source),
        origin: "bundled".to_owned(),
        book_titles: Vec::new(),
        red_letters: false,
        paragraphs: false,
        headings: false,
    }
}

/// Returns the archive for a source, downloading it when the cache is cold.
///
/// A cached archive that does not match the pin is thrown away and fetched
/// again. A freshly downloaded archive that does not match is a hard failure: the
/// partial file is removed, and the message names the archive and both digests.
fn fetch(source: &Source, cache: &Path) -> Result<PathBuf> {
    let path = cache.join(format!("{}_usfm.zip", source.archive));
    if path.is_file() {
        let actual = digest_of(&path)?;
        if actual.eq_ignore_ascii_case(source.sha256) {
            return Ok(path);
        }
        println!(
            "cached {} does not match the pin, fetching it again",
            path.display()
        );
        fs::remove_file(&path)
            .with_context(|| format!("cannot remove the stale archive {}", path.display()))?;
    }

    let partial = cache.join(format!("{}_usfm.zip.part", source.archive));
    if partial.is_file() {
        fs::remove_file(&partial)
            .with_context(|| format!("cannot remove the partial download {}", partial.display()))?;
    }

    let url = archive_url(source);
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--retry",
            "2",
        ])
        .arg("--output")
        .arg(&partial)
        .arg(&url)
        .status()
        .context("curl is needed to download the archives and it did not run")?;
    if !status.success() {
        let _ = fs::remove_file(&partial);
        bail!("downloading {url} failed with {status}");
    }

    let actual = digest_of(&partial)?;
    if !actual.eq_ignore_ascii_case(source.sha256) {
        let _ = fs::remove_file(&partial);
        bail!(
            "digest mismatch for {} ({}):\n  expected sha256 {}\n  actual   sha256 {}\n\
             nothing was written to {OUT_DIR}",
            source.name,
            url,
            source.sha256,
            actual
        );
    }

    fs::rename(&partial, &path)
        .with_context(|| format!("cannot move {} into place", partial.display()))?;
    println!(
        "downloaded {:<8} {} ({} bytes)",
        source.id,
        url,
        fs::metadata(&path)?.len()
    );
    Ok(path)
}

/// The lower-case hex SHA-256 of a file, read in one piece.
fn digest_of(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(digest(&bytes))
}

/// The lower-case hex SHA-256 of some bytes.
fn digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(hash.len() * 2);
    for byte in hash {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    out
}

/// Writes a file so that a reader sees either the whole old file or the whole new
/// one, never a half-written file.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_file_name(format!(
        "{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&temporary, bytes)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    fs::rename(&temporary, path)
        .with_context(|| format!("cannot move {} into place", temporary.display()))?;
    Ok(())
}

/// Checks the built files against the grid, the witness verses, and the pins.
///
/// Every translation gets one line and every failure is collected before the task
/// exits non-zero, so one bad file does not hide the others.
fn verify_data() -> Result<()> {
    let root = repo_root()?;
    let out = root.join(OUT_DIR);
    let cache = root.join(CACHE_DIR);
    let readme = fs::read_to_string(root.join("data/README.md")).unwrap_or_default();

    let mut failures = 0usize;
    let mut total = 0u64;
    for source in &SOURCES {
        match verify_one(source, &out, &cache, &readme) {
            Ok((line, size)) => {
                total += size;
                println!("{line}");
            }
            Err(error) => {
                failures += 1;
                println!("{:<8} FAIL  {error}", source.id);
            }
        }
    }

    if failures > 0 {
        bail!(
            "{failures} of {} translations failed verification",
            SOURCES.len()
        );
    }
    println!(
        "all {} translations verified, {total} bytes in total",
        SOURCES.len()
    );
    Ok(())
}

/// Verifies one built translation and returns its report line and file size.
fn verify_one(source: &Source, out: &Path, cache: &Path, readme: &str) -> Result<(String, u64)> {
    let path = out.join(format!("{}.bbl", source.id));
    let bytes = fs::read(&path).with_context(|| format!("{} is missing", path.display()))?;
    if bytes.get(..4) != Some(b"BIBL".as_slice()) {
        bail!("{} does not start with BIBL", path.display());
    }

    let translation =
        decode(&bytes).with_context(|| format!("{} does not decode", path.display()))?;
    if translation.meta().id != source.id {
        bail!(
            "the metadata id is {} but the file is {}",
            translation.meta().id,
            path.display()
        );
    }
    if translation.verses().len() != VERSE_COUNT {
        bail!(
            "the file holds {} verses, the canonical grid has {VERSE_COUNT}",
            translation.verses().len()
        );
    }

    let genesis = translation
        .verse_at(0, 1, 1)
        .context("Genesis 1:1 is missing from the grid")?;
    let genesis_text = translation.verse_text(genesis);
    if genesis_text != source.genesis {
        bail!(
            "Genesis 1:1 is {:?} but {:?} was expected",
            genesis_text,
            source.genesis
        );
    }

    let revelation = translation
        .verse_at(65, 22, 21)
        .context("Revelation 22:21 is missing from the grid")?;
    let revelation_text = translation.verse_text(revelation);
    if revelation_text != source.revelation {
        bail!(
            "Revelation 22:21 is {:?} but {:?} was expected",
            revelation_text,
            source.revelation
        );
    }

    let url = archive_url(source);
    if translation.meta().provenance != url {
        bail!(
            "the provenance is {:?} but {url} was expected",
            translation.meta().provenance
        );
    }
    if !readme.contains(source.sha256) {
        bail!("data/README.md does not pin the archive digest");
    }

    let archive = cache.join(format!("{}_usfm.zip", source.archive));
    let archive_status = if archive.is_file() {
        let actual = digest_of(&archive)?;
        if !actual.eq_ignore_ascii_case(source.sha256) {
            bail!(
                "the cached archive {} hashes to {actual} but {} is pinned",
                archive.display(),
                source.sha256
            );
        }
        "archive digest ok"
    } else {
        "archive not cached, pin ok"
    };

    let size = bytes.len() as u64;
    let line = format!(
        "{:<8} {:<28} {} verses  gen 1:1 ok  rev 22:21 ok  {:<26} {:>9} bytes",
        source.id,
        source.name,
        translation.verses().len(),
        archive_status,
        size
    );
    Ok((line, size))
}

/// Writes `data/README.md` from the pinned table, so the licences and digests the
/// reader is shown are the same ones the build verifies.
fn write_readme(root: &Path) -> Result<()> {
    let mut text = String::new();
    text.push_str("# Bundled translation data\n\n");
    text.push_str(
        "`data/translations/` holds the seven public-domain English translations that ship\n\
         with the app. Each file is the binary translation format: the four bytes `BIBL`, a\n\
         little-endian `u16` format version, a `u32` metadata length, the metadata as\n\
         uncompressed JSON, a `u32` body length, and the zstd-compressed column body.\n\n",
    );
    text.push_str(
        "**No copyrighted translation text is in this repository.** Only the seven\n\
         public-domain translations below are bundled, and the raw USFM they were built from\n\
         is never committed. The importer and the tests use generated words only.\n\n",
    );
    text.push_str("## The translations\n\n");
    text.push_str(
        "| Id | Abbreviation | Translation | Edition | Licence | Source archive | SHA-256 |\n\
         |---|---|---|---|---|---|---|\n",
    );
    for source in &SOURCES {
        text.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | <{}> | `{}` |\n",
            source.id,
            source.abbreviation,
            source.name,
            source.edition,
            source.licence,
            archive_url(source),
            source.sha256
        ));
    }

    text.push_str("\n## How the files were built\n\n");
    text.push_str(
        "```text\ncargo run -p xtask -- build-data\n```\n\n\
         The task downloads each archive from eBible.org into `target/data-cache`, checks\n\
         the archive's SHA-256 against the digest pinned in the table above, imports the\n\
         archive with the same USFM importer users get, lays it on the canonical grid, and\n\
         encodes it with the core crate's encoder. Nothing is written until all seven\n\
         archives have passed their digest check and all seven translations have converted,\n\
         so an interrupted or tampered build leaves the previous files untouched.\n\n\
         The task also rewrites this file from the same table, so a digest or a licence\n\
         cannot drift between the build and this document.\n\n",
    );
    text.push_str("## Verifying\n\n");
    text.push_str(
        "```text\ncargo run -p xtask -- verify-data\n```\n\n\
         The task decodes every file and checks that it starts with `BIBL`, that its\n\
         metadata id and provenance match the pins, that it holds all 31102 verses of the\n\
         canonical grid, that Genesis 1:1 and Revelation 22:21 read exactly as the edition\n\
         prints them, and that the cached archive still hashes to the pinned digest. It\n\
         prints one line per translation and exits non-zero when any check fails.\n\n",
    );
    text.push_str("## Licence\n\n");
    text.push_str(
        "All seven translations are in the public domain, and each file carries its\n\
         edition's notice in the metadata header so the notice travels with the text. The\n\
         build tooling in `xtask` is MIT licensed.\n",
    );

    let path = root.join("data/README.md");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    write_atomically(&path, text.as_bytes())
}

/// The filler words the synthetic sample is made of.
///
/// None of them is scripture. The sample exists so the reader has a typographic
/// page to paint on a machine with no translation installed.
const FILLER: [&str; 24] = [
    "amber", "birch", "cobalt", "dune", "ember", "fern", "granite", "harbor", "indigo", "juniper",
    "kelp", "lantern", "meadow", "nickel", "ochre", "pebble", "quartz", "river", "slate", "timber",
    "umber", "violet", "willow", "yarrow",
];

/// Regenerates the committed synthetic sample page.
///
/// The output depends only on the canonical grid and the filler table, so running
/// the task twice produces the same bytes and the committed blob is provably the
/// output of this command.
fn sample_bbl() -> Result<()> {
    let root = repo_root()?;
    let mut builder = TranslationBuilder::new(sample_meta());
    for book in 0..BOOK_COUNT as u8 {
        let Some(entry) = books::book(book) else {
            continue;
        };
        for chapter in 1..=entry.chapter_count {
            let verses = books::verses_in_chapter(book, chapter).unwrap_or(0);
            for verse in 1..=verses as u8 {
                let text = sample_verse(book, chapter, verse);
                builder.push_verse(verse, &text)?;
                add_sample_marks(&mut builder, book, chapter, verse, &text)?;
            }
        }
    }
    let translation = builder.finish()?;
    let bytes = encode_at_level(&translation, BUNDLED_LEVEL)?;
    let path = root.join(SAMPLE_PATH);
    write_atomically(&path, &bytes)?;
    println!(
        "wrote    {} ({} bytes, {} verses)",
        path.display(),
        bytes.len(),
        translation.verses().len()
    );
    Ok(())
}

/// The metadata carried by the synthetic sample.
fn sample_meta() -> TranslationMeta {
    let book_titles = books::books()
        .iter()
        .map(|book| book.name.to_owned())
        .collect();
    TranslationMeta {
        id: "sample".to_owned(),
        abbreviation: "SAMPLE".to_owned(),
        name: "Synthetic Sample".to_owned(),
        language: "English".to_owned(),
        licence: "Generated filler, not scripture".to_owned(),
        copyright: "Copyright (c) 2026 madearga".to_owned(),
        provenance: "generated by cargo run -p xtask -- sample-bbl".to_owned(),
        origin: "bundled sample".to_owned(),
        book_titles,
        red_letters: true,
        paragraphs: true,
        headings: true,
    }
}

/// One deterministic line of filler for a canonical verse.
///
/// The verse position seeds a small linear congruential generator, so the words
/// are stable from run to run and from machine to machine without any dependency
/// on a random number generator.
fn sample_verse(book: u8, chapter: u8, verse: u8) -> String {
    let mut seed = u32::from(book) * 7919 + u32::from(chapter) * 131 + u32::from(verse);
    let words = 6 + (seed % 5) as usize;
    let mut text = String::with_capacity(words * 9);
    for index in 0..words {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        if index > 0 {
            text.push(' ');
        }
        text.push_str(FILLER[(seed >> 16) as usize % FILLER.len()]);
    }
    text.push('.');
    text
}

/// Attaches structural marks and character styles to a sample verse.
///
/// Every chapter opens a paragraph, and John 3 carries one of every other mark and
/// every character style, so the render harness can show the whole typography of
/// the reader from the sample alone.
fn add_sample_marks(
    builder: &mut TranslationBuilder,
    book: u8,
    chapter: u8,
    verse: u8,
    text: &str,
) -> Result<()> {
    if verse == 1 {
        builder.mark(0, MarkKind::Paragraph, 0, "")?;
    }

    if book == 42 && chapter == 3 {
        match verse {
            1 => {
                builder.mark(0, MarkKind::Heading, 0, "A generated heading")?;
            }
            2 => {
                builder.mark(0, MarkKind::Superscription, 0, "A generated superscription")?;
                builder.mark(0, MarkKind::PoetryLine, 0, "")?;
                builder.mark(0, MarkKind::PoetryIndent, 2, "")?;
                builder.mark(0, MarkKind::StanzaBreak, 0, "")?;
            }
            3 => {
                let (start, end) = first_word(text);
                builder.span(SpanStyle::WordsOfChrist, start, end)?;
            }
            4 => {
                let (start, end) = first_word(text);
                builder.span(SpanStyle::TranslatorAddition, start, end)?;
            }
            5 => {
                let (start, end) = first_word(text);
                builder.span(SpanStyle::DivineName, start, end)?;
            }
            6 => {
                let (start, end) = first_word(text);
                builder.span(SpanStyle::Selah, start, end)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// The byte range of the first word of a line, for the sample's spans.
fn first_word(text: &str) -> (u16, u16) {
    let end = text.find(' ').unwrap_or(text.len());
    (0, u16::try_from(end).unwrap_or(0))
}

/// Keeps the reader honest about the canonical grid the sample is built on.
#[cfg(test)]
mod tests {
    use super::*;

    /// The sample is filler in the shape of the grid, and nothing else.
    #[test]
    fn the_sample_text_is_deterministic_and_short() {
        let first = sample_verse(0, 1, 1);
        assert_eq!(first, sample_verse(0, 1, 1));
        assert!(!first.is_empty());
        assert!(first.len() < 200);
        assert_ne!(first, sample_verse(0, 1, 2));
    }

    /// The pins describe a full set of distinct archives.
    #[test]
    fn every_pin_is_distinct_and_well_formed() {
        for (index, source) in SOURCES.iter().enumerate() {
            assert_eq!(source.sha256.len(), 64, "{} has a short digest", source.id);
            assert!(
                source
                    .sha256
                    .chars()
                    .all(|character| character.is_ascii_hexdigit()),
                "{} has a digest that is not hex",
                source.id
            );
            for other in &SOURCES[index + 1..] {
                assert_ne!(source.id, other.id);
                assert_ne!(source.archive, other.archive);
                assert_ne!(source.sha256, other.sha256);
            }
        }
    }

    /// The first word range the sample spans use sits inside the line.
    #[test]
    fn the_first_word_range_fits_the_line() {
        let text = "amber birch cobalt.";
        let (start, end) = first_word(text);
        assert_eq!(start, 0);
        assert_eq!(&text[start as usize..end as usize], "amber");
    }
}
