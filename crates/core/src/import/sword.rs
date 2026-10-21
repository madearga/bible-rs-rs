// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The SWORD module reader.
//!
//! A SWORD module is a folder or an archive holding a manifest under `mods.d`,
//! the module's testaments under `modules/texts`, and the module's own naming for
//! its books. The manifest declares the driver that stores the text, the
//! versification the text is numbered by, and whether the module is locked.
//!
//! Two drivers are read here. zText keeps a ten byte record per verse saying which
//! compressed block holds it, where in the block it starts, and how long it is; a
//! twelve byte record per block says where that block's compressed bytes are. RawText
//! keeps the text of every verse in one file with a newline after each, and a six byte
//! record per verse saying where each one starts. Both hand back verse text in the
//! order of their versification, which is then laid on the canonical grid.
//!
//! Markup inside the text is not shown to the reader as characters. The OSIS, ThML,
//! and GBF dialects are stripped here and their styles become spans.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::books;
use crate::model::{MarkKind, SpanStyle, Translation, TranslationMeta};

use super::{Document, VerseAccumulator, assemble, decode_text, zip};

/// The canonical index of Psalms, whose titles are superscriptions.
const PSALMS: u8 = 18;

/// The largest number of bytes one compressed block may expand to.
///
/// A real zText block holds one book, chapter, or verse, so a few hundred
/// kilobytes is already large. The bound stops a small hostile block from being
/// a decompression bomb.
const MAX_BLOCK_BYTES: usize = 4 * 1024 * 1024;

/// The largest total number of decoded block bytes one module may produce.
///
/// A whole module decodes to a few tens of megabytes at most, so this leaves
/// room for a large module without letting the block table multiply the bound.
const MAX_MODULE_BLOCK_BYTES: usize = 64 * 1024 * 1024;

/// The largest total decoded verse text one module may produce.
///
/// The bound is checked against the bytes taken from a decoded block before the
/// text is copied into a string, so repeated index records cannot multiply the
/// text budget either.
const MAX_MODULE_TEXT_BYTES: usize = 32 * 1024 * 1024;

/// The largest number of verse index records any supported versification needs.
///
/// The canonical grid holds 31,102 verses and the Apocrypha a module may append
/// adds a few thousand more, with one extra record per book and chapter
/// introduction. The bound is deliberately generous and still far below what a
/// hostile index could declare.
const MAX_VERSE_RECORDS: usize = 48_000;

/// The largest number of block table records a module may declare.
///
/// Every block holds at least one verse, so there can never be more blocks than
/// verse records.
const MAX_BLOCK_RECORDS: usize = MAX_VERSE_RECORDS;

/// The decoded bytes one module has produced so far, checked against the hard
/// budgets above.
#[derive(Debug, Default)]
struct DecodeBudget {
    /// Decoded bytes across every block of both testaments.
    blocks: usize,
    /// Verse text bytes taken from the decoded blocks.
    text: usize,
}

impl DecodeBudget {
    /// Adds one decoded block and refuses it when either budget would be passed.
    fn add_block(&mut self, bytes: usize) -> Result<()> {
        self.blocks = self.blocks.saturating_add(bytes);
        if self.blocks > MAX_MODULE_BLOCK_BYTES {
            bail!("the module's blocks expand past the {MAX_MODULE_BLOCK_BYTES} byte limit");
        }
        Ok(())
    }

    /// Reserves verse text before it is decoded and refuses it past the budget.
    fn take_text(&mut self, bytes: usize) -> Result<()> {
        self.text = self.text.saturating_add(bytes);
        if self.text > MAX_MODULE_TEXT_BYTES {
            bail!("the module's verse text passes the {MAX_MODULE_TEXT_BYTES} byte limit");
        }
        Ok(())
    }
}

/// The markup dialect a module's text is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Markup {
    /// OSIS elements such as `<q>` and `<transChange>`.
    Osis,
    /// ThML elements such as `<span class="add">`.
    Thml,
    /// GBF elements such as `<FI>` and `<FR>`.
    Gbf,
}

/// The driver that stores the module's verse text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Driver {
    /// Compressed blocks with a verse index.
    ZText,
    /// Plain verse text with an offset index.
    RawText,
}

/// How a zText module groups its verses into compressed blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockType {
    /// One block per book.
    Book,
    /// One block per chapter.
    Chapter,
    /// One block per verse.
    Verse,
}

impl BlockType {
    /// The letter the module puts in its index file names for this grouping.
    fn letter(self) -> char {
        match self {
            Self::Book => 'b',
            Self::Chapter => 'c',
            Self::Verse => 'v',
        }
    }
}

/// The versification a module numbers its verses by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Versification {
    /// The grid this program uses.
    Kjv,
    /// The grid plus the Apocrypha, which a module keeps inside its Old Testament.
    Kjva,
    /// A versification whose chapter verse counts differ from the grid in places.
    Nrsv,
    /// The NRSV versification plus the Apocrypha.
    Nrsva,
}

/// Reads a SWORD module into a translation.
///
/// `unlock_key` is the key the user owns for a locked module and is ignored for
/// an unlocked one.
pub(crate) fn import(
    path: &Path,
    meta: TranslationMeta,
    unlock_key: Option<&str>,
) -> Result<Translation> {
    let store = Store::open(path)?;
    let manifest = Manifest::find(&store, path)?;
    manifest.verify_unlock(unlock_key)?;

    let mut document = Document::new();
    let mut markup = manifest.markup;
    let mut budget = DecodeBudget::default();
    for (testament, books_range) in [("ot", 0u8..39u8), ("nt", 39u8..66)] {
        let entries = manifest.read_testament(&store, testament, &mut budget)?;
        if entries.is_empty() {
            continue;
        }
        let dialect = match markup {
            Some(dialect) => dialect,
            None => {
                let dialect = detect_markup(entries.first().map(String::as_str).unwrap_or(""));
                markup = Some(dialect);
                dialect
            }
        };
        assign(
            &mut document,
            &entries,
            books_range,
            manifest.versification,
            dialect,
        )?;
    }

    if document.is_empty() {
        bail!("the SWORD module holds no verse the canonical grid knows");
    }

    let mut meta = meta;
    if meta.name.trim().is_empty()
        && let Some(description) = manifest.entries.get("Description")
    {
        meta.name = description.clone();
    }
    if meta.language.trim().is_empty()
        && let Some(language) = manifest.entries.get("Lang")
    {
        meta.language = language.clone();
    }
    if meta.provenance.trim().is_empty() {
        meta.provenance = path.display().to_string();
    }
    assemble(document, meta)
}

/// Where a module's files are read from, either an archive in memory or a folder.
#[derive(Debug)]
enum Store {
    /// Files read out of an archive, keyed by their path inside it spelled in lowercase.
    Memory(BTreeMap<String, Vec<u8>>),
    /// Files read from disk, with every path relative to the module root.
    Disk(PathBuf),
}

/// Whether a path stays inside the module it is used from.
///
/// Only normal components and the current-folder component are accepted, so an
/// absolute path, a parent traversal, and a platform prefix are all refused
/// before they are joined onto the module root.
fn is_safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Resolves a module file under the module root and verifies it stays there.
///
/// Returns `None` when the module does not hold the file. A path that is not
/// relative, or that resolves outside the root through a link, is an error.
fn resolve_module_file(root: &Path, name: &str) -> Result<Option<PathBuf>> {
    let relative = Path::new(name);
    if !is_safe_relative(relative) {
        bail!("the module path {name:?} is not a relative path inside the module");
    }
    let candidate = root.join(relative);
    if !candidate.is_file() {
        return Ok(None);
    }
    let resolved = candidate
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", candidate.display()))?;
    if !resolved.starts_with(root) {
        bail!(
            "the module path {name:?} resolves outside the module root at {}",
            resolved.display()
        );
    }
    Ok(Some(resolved))
}

impl Store {
    /// Opens a module that is a zip, a folder, or a manifest file.
    fn open(path: &Path) -> Result<Self> {
        if path.is_dir() {
            let root = module_root(path)?;
            let root = root
                .canonicalize()
                .with_context(|| format!("cannot resolve the module root {}", root.display()))?;
            return Ok(Self::Disk(root));
        }
        if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("conf"))
        {
            let root = path
                .parent()
                .and_then(Path::parent)
                .with_context(|| format!("{} is not inside a mods.d folder", path.display()))?;
            let root = root
                .canonicalize()
                .with_context(|| format!("cannot resolve the module root {}", root.display()))?;
            return Ok(Self::Disk(root));
        }

        let entries = zip::read_archive(path)?;
        let mut files = BTreeMap::new();
        for entry in entries {
            let name = entry.name.trim_start_matches("./").to_ascii_lowercase();
            files.insert(name, entry.bytes);
        }
        if files.is_empty() {
            bail!("{} is an empty archive", path.display());
        }
        Ok(Self::Memory(files))
    }

    /// Reads one file of the module, or `None` when the module does not have it.
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let name = name.trim_start_matches("./");
        match self {
            Self::Memory(files) => Ok(files.get(&name.to_ascii_lowercase()).cloned()),
            Self::Disk(root) => {
                let Some(path) = resolve_module_file(root, name)? else {
                    return Ok(None);
                };
                Ok(Some(std::fs::read(&path).with_context(|| {
                    format!("cannot read {}", path.display())
                })?))
            }
        }
    }

    /// Every path the module holds under a prefix, in name order.
    fn list(&self, prefix: &str) -> Vec<String> {
        let prefix = prefix.trim_start_matches("./").to_ascii_lowercase();
        match self {
            Self::Memory(files) => files
                .keys()
                .filter(|name| name.starts_with(&prefix))
                .cloned()
                .collect(),
            Self::Disk(root) => {
                let base = prefix.trim_end_matches('/');
                let mut found = Vec::new();
                collect_files(&root.join(base), &format!("{base}/"), &mut found);
                found.sort();
                found
            }
        }
    }
}

/// Walks a folder and returns the relative paths under it.
fn collect_files(directory: &Path, prefix: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = format!(
            "{prefix}{}",
            entry.file_name().to_string_lossy().to_ascii_lowercase()
        );
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(&path, &format!("{name}/"), found);
        } else {
            found.push(name);
        }
    }
}

/// Finds the folder that holds `mods.d`, walking up a little from a given folder.
fn module_root(path: &Path) -> Result<PathBuf> {
    let mut candidate = path.to_path_buf();
    for _ in 0..4 {
        if candidate.join("mods.d").is_dir() {
            return Ok(candidate);
        }
        if !candidate.pop() {
            break;
        }
    }
    Ok(path.to_path_buf())
}

/// The parts of a module manifest this reader uses.
#[derive(Debug)]
struct Manifest {
    /// Every key and value in the module's section of the manifest.
    entries: BTreeMap<String, String>,
    /// The driver that stores the text.
    driver: Driver,
    /// The versification the text is numbered by.
    versification: Versification,
    /// The folder the module's testaments live in, relative to the module root.
    data_path: String,
    /// The compression of the blocks, which only zText uses.
    compression: String,
    /// How the module groups its verses into compressed blocks.
    block_type: BlockType,
    /// The markup dialect the manifest names, when it names one.
    markup: Option<Markup>,
}

impl Manifest {
    /// Reads the manifest out of the module and checks it describes a supported module.
    fn find(store: &Store, path: &Path) -> Result<Self> {
        let name = if path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("conf"))
        {
            let file = path
                .file_name()
                .map(|name| name.to_string_lossy().to_ascii_lowercase())
                .context("the manifest has no file name")?;
            format!("mods.d/{file}")
        } else {
            let manifests = store
                .list("mods.d/")
                .into_iter()
                .filter(|name| name.ends_with(".conf"))
                .collect::<Vec<_>>();
            match manifests.len() {
                0 => bail!("{} holds no mods.d manifest", path.display()),
                1 => manifests[0].clone(),
                _ => bail!(
                    "{} holds several modules, point the import at one of them: {}",
                    path.display(),
                    manifests.join(", ")
                ),
            }
        };

        let bytes = store
            .read(&name)?
            .with_context(|| format!("{name} is missing from the module"))?;
        let text = decode_text(&bytes)?;
        let entries = parse_manifest(&text, &name)?;

        let driver = match entries
            .get("ModDrv")
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("ztext") => Driver::ZText,
            Some("rawtext") => Driver::RawText,
            Some(other) => {
                bail!("the module uses the {other} driver, only zText and RawText are supported")
            }
            None => bail!("the module manifest does not declare a ModDrv"),
        };

        let versification = match entries
            .get("Versification")
            .map(|value| value.to_ascii_uppercase())
            .as_deref()
        {
            Some("KJV" | "KJV_STRONGS") | None => Versification::Kjv,
            Some("KJVA") => Versification::Kjva,
            Some("NRSV") => Versification::Nrsv,
            Some("NRSVA") => Versification::Nrsva,
            Some(other) => {
                bail!("the module declares the {other} versification, which is not supported")
            }
        };

        let data_path = entries
            .get("DataPath")
            .map(|value| {
                value
                    .trim()
                    .trim_start_matches("./")
                    .trim_end_matches('/')
                    .to_string()
            })
            .unwrap_or_else(|| guess_data_path(store, &name));
        if !is_safe_relative(Path::new(&data_path)) {
            bail!("the module DataPath {data_path:?} is not a relative path inside the module");
        }

        let compression = entries
            .get("CompressType")
            .map(|value| value.to_ascii_uppercase())
            .unwrap_or_else(|| "LZSS".to_string());

        let block_type = match entries
            .get("BlockType")
            .map(|value| value.to_ascii_uppercase())
            .as_deref()
        {
            Some("VERSE") => BlockType::Verse,
            Some("BOOK") => BlockType::Book,
            Some("CHAPTER") | None => BlockType::Chapter,
            Some(other) => {
                bail!("the module declares the {other} block type, which is not supported")
            }
        };

        let markup = entries
            .get("Markup")
            .or_else(|| entries.get("SourceType"))
            .map(|value| value.to_ascii_lowercase())
            .and_then(|value| match value.as_str() {
                "osis" => Some(Markup::Osis),
                "thml" => Some(Markup::Thml),
                "gbf" => Some(Markup::Gbf),
                _ => None,
            });

        Ok(Self {
            entries,
            driver,
            versification,
            data_path,
            compression,
            block_type,
            markup,
        })
    }

    /// Reports whether the module is locked, and refuses to read one it cannot decipher.
    ///
    /// A manifest locks a module by leaving its cipher key empty, and SWORD itself
    /// does not store a check value: a frontend installs the user's key and decides
    /// the key is right by seeing whether the first entry comes out as text. The
    /// importer reaches the same point and stops, because it does not decipher, so a
    /// locked module is refused with the reason rather than read as garbage.
    fn verify_unlock(&self, unlock_key: Option<&str>) -> Result<()> {
        let locked = self
            .entries
            .get("CipherKey")
            .is_some_and(|value| value.trim().is_empty());
        if !locked {
            return Ok(());
        }
        if unlock_key.is_some_and(|key| !key.trim().is_empty()) {
            bail!(
                "this SWORD module is enciphered and this importer does not decipher it yet, so the text cannot be read"
            );
        }
        bail!("this SWORD module is locked and needs the unlock key the user owns")
    }

    /// Reads one testament's verse text in the module's own order.
    fn read_testament(
        &self,
        store: &Store,
        testament: &str,
        budget: &mut DecodeBudget,
    ) -> Result<Vec<String>> {
        match self.driver {
            Driver::ZText => self.read_ztext(store, testament, budget),
            Driver::RawText => self.read_rawtext(store, testament, budget),
        }
    }

    /// Reads the compressed blocks a zText testament holds and slices out its verses.
    ///
    /// Every block is decompressed through a hard output limit and the module's
    /// decoded byte budget, and the block table's declared size is checked only as
    /// an early refusal, never trusted as the size the decoder produces.
    fn read_ztext(
        &self,
        store: &Store,
        testament: &str,
        budget: &mut DecodeBudget,
    ) -> Result<Vec<String>> {
        let Some((index, sizes, body)) = self.ztext_files(store, testament)? else {
            return Ok(Vec::new());
        };

        if !self.compression.eq_ignore_ascii_case("ZIP") {
            bail!(
                "the module compresses its blocks with {}, and this importer only decompresses ZIP",
                self.compression
            );
        }

        let block_bounds = block_bounds(&sizes)?;
        let mut blocks: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
        let mut texts = Vec::new();
        let empty = Vec::new();

        for record in verse_records(&index)? {
            if let std::collections::btree_map::Entry::Vacant(slot) = blocks.entry(record.block) {
                let (start, end, declared) =
                    *block_bounds.get(record.block as usize).with_context(|| {
                        format!(
                            "verse index names block {} which the module lacks",
                            record.block
                        )
                    })?;
                if declared as usize > MAX_BLOCK_BYTES {
                    bail!(
                        "block {} declares {declared} bytes, the limit is {MAX_BLOCK_BYTES}",
                        record.block
                    );
                }
                if end as usize > body.len() {
                    bail!(
                        "block {} runs past the end of the compressed data",
                        record.block
                    );
                }
                let slice = &body[start as usize..end as usize];
                let mut decoded = Vec::new();
                flate2::read::ZlibDecoder::new(slice)
                    .take(MAX_BLOCK_BYTES as u64 + 1)
                    .read_to_end(&mut decoded)
                    .with_context(|| format!("block {} cannot be decompressed", record.block))?;
                if decoded.len() > MAX_BLOCK_BYTES {
                    bail!(
                        "block {} expands past the {MAX_BLOCK_BYTES} byte limit",
                        record.block
                    );
                }
                budget.add_block(decoded.len())?;
                slot.insert(decoded);
            }
            let block = blocks.get(&record.block).unwrap_or(&empty);
            let start = record.offset as usize;
            let end = start.checked_add(record.length as usize).with_context(|| {
                format!("verse index record in block {} overflows", record.block)
            })?;
            if end > block.len() {
                bail!(
                    "verse index record points past the end of block {} at {start} plus {} bytes",
                    record.block,
                    record.length
                );
            }
            let slice = &block[start..end];
            budget.take_text(slice.len())?;
            texts.push(decode_text(slice).unwrap_or_default());
        }

        Ok(texts)
    }

    /// Finds the three files a zText testament is made of.
    ///
    /// The letter in the middle of each name comes from the module's block type, so
    /// the module's own letter is tried first and the others after it, which keeps a
    /// manifest that declares the wrong grouping readable.
    fn ztext_files(&self, store: &Store, testament: &str) -> Result<Option<ZTextFiles>> {
        let mut letters = vec![self.block_type.letter()];
        for letter in ['b', 'c', 'v'] {
            if !letters.contains(&letter) {
                letters.push(letter);
            }
        }

        for letter in letters {
            let base = format!("{}/{testament}.{letter}", self.data_path);
            let (Some(index), Some(sizes), Some(body)) = (
                store.read(&format!("{base}zv"))?,
                store.read(&format!("{base}zs"))?,
                store.read(&format!("{base}zz"))?,
            ) else {
                continue;
            };
            return Ok(Some((index, sizes, body)));
        }
        Ok(None)
    }

    /// Reads a RawText testament, whose text is one newline-terminated entry per verse.
    fn read_rawtext(
        &self,
        store: &Store,
        testament: &str,
        budget: &mut DecodeBudget,
    ) -> Result<Vec<String>> {
        let base = format!("{}/{testament}", self.data_path);
        let Some(body) = store.read(&base)? else {
            return Ok(Vec::new());
        };

        match store.read(&format!("{base}.vss"))? {
            Some(index) => {
                if !index.len().is_multiple_of(6) {
                    bail!(
                        "the {testament} verse index is {} bytes, which is not a whole number of six byte records",
                        index.len()
                    );
                }
                if index.len() / 6 > MAX_VERSE_RECORDS {
                    bail!(
                        "the {testament} verse index holds {} records, the limit is {MAX_VERSE_RECORDS}",
                        index.len() / 6
                    );
                }
                let mut texts = Vec::with_capacity(index.len() / 6);
                for chunk in index.as_chunks::<6>().0 {
                    let start =
                        u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
                    let length = u16::from_le_bytes([chunk[4], chunk[5]]) as usize;
                    let end = start.saturating_add(length).min(body.len());
                    let slice = body.get(start..end).unwrap_or_default();
                    budget.take_text(slice.len())?;
                    texts.push(decode_text(slice)?);
                }
                Ok(texts)
            }
            None => bail!(
                "the {testament} testament has no verse index, so its verses cannot be located"
            ),
        }
    }
}

/// The verse index, block table, and compressed blocks of one zText testament.
type ZTextFiles = (Vec<u8>, Vec<u8>, Vec<u8>);

/// One verse's place inside a zText testament.
#[derive(Clone, Copy, Debug)]
struct VerseRecord {
    /// Which compressed block holds the verse.
    block: u32,
    /// Where in the block the verse starts.
    offset: u32,
    /// How many bytes of the block the verse covers.
    length: u32,
}

/// Reads the verse index, which is one ten byte record per verse.
///
/// The record names the compressed block that holds the verse, where in the block
/// the verse starts, and how many bytes of it the verse covers.
fn verse_records(index: &[u8]) -> Result<Vec<VerseRecord>> {
    if !index.len().is_multiple_of(10) {
        bail!(
            "the verse index is {} bytes, which is not a whole number of ten byte records",
            index.len()
        );
    }
    let count = index.len() / 10;
    if count > MAX_VERSE_RECORDS {
        bail!("the verse index holds {count} records, the limit is {MAX_VERSE_RECORDS}");
    }
    let mut records = Vec::with_capacity(count);
    for chunk in index.as_chunks::<10>().0 {
        records.push(VerseRecord {
            block: u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
            offset: u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]),
            length: u32::from(u16::from_le_bytes([chunk[8], chunk[9]])),
        });
    }
    Ok(records)
}

/// Reads the block table, which is one twelve byte record per compressed block.
///
/// The record gives where the block's compressed bytes start and how long they are,
/// and returns the size the block claims to expand to. The claimed size is used only
/// as an early refusal, never as the real decompressed length.
fn block_bounds(sizes: &[u8]) -> Result<Vec<(u32, u32, u32)>> {
    if !sizes.len().is_multiple_of(12) {
        bail!(
            "the block table is {} bytes, which is not a whole number of twelve byte records",
            sizes.len()
        );
    }
    let count = sizes.len() / 12;
    if count > MAX_BLOCK_RECORDS {
        bail!("the block table holds {count} records, the limit is {MAX_BLOCK_RECORDS}");
    }
    let mut bounds = Vec::with_capacity(count);
    for chunk in sizes.as_chunks::<12>().0 {
        let start = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let length = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        let declared = u32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
        let end = start
            .checked_add(length)
            .context("a block in the table runs past the end of the archive")?;
        bounds.push((start, end, declared));
    }
    Ok(bounds)
}

/// Lays a testament's verse text onto the canonical grid.
///
/// A module's verse index is one record per entry of its versification. That
/// includes two records a testament leads with and one record for every book and
/// chapter introduction, so the record for a verse is computed by walking the
/// canonical books and counting the introductions. The walk also keeps the
/// Apocrypha that a KJVA or NRSVA module appends after Malachi out of the grid.
fn assign(
    document: &mut Document,
    entries: &[String],
    books_range: std::ops::Range<u8>,
    versification: Versification,
    markup: Markup,
) -> Result<()> {
    // A testament leads with two records of its own before the first book.
    let mut index = 2usize;
    let mut stated = 0usize;

    for book in books_range {
        let Some(entry) = books::book(book) else {
            continue;
        };
        index += 1;
        for chapter in 1..=entry.chapter_count {
            index += 1;
            let count = books::verses_in_chapter(book, chapter).unwrap_or(0);
            for verse in 1..=count as u8 {
                if let Some(text) = entries.get(index) {
                    stated += 1;
                    if !text.trim().is_empty() {
                        let mut accumulator = VerseAccumulator::new(book, chapter, verse, verse);
                        if strip_markup(markup, text, &mut accumulator) {
                            document.note_red_letters();
                        }
                        document.insert(book, chapter, verse, accumulator.finish());
                    }
                }
                index += 1;
            }
        }
    }

    if stated == 0 {
        bail!(
            "the module's {} versification holds fewer verse entries than the canonical grid",
            match versification {
                Versification::Kjv | Versification::Kjva => "KJV",
                Versification::Nrsv | Versification::Nrsva => "NRSV",
            }
        );
    }
    Ok(())
}

/// Guesses the markup dialect from the shape of the text when the manifest is silent.
fn detect_markup(entry: &str) -> Markup {
    let lower = entry.to_ascii_lowercase();
    if lower.contains("<scripref") || lower.contains("<span") || lower.contains("<div") {
        Markup::Thml
    } else if lower.contains("<fi>") || lower.contains("<fr>") || lower.contains("<w>") {
        Markup::Gbf
    } else {
        Markup::Osis
    }
}

/// Guesses where a module keeps its testaments when the manifest omits `DataPath`.
fn guess_data_path(store: &Store, manifest_name: &str) -> String {
    let module = manifest_name
        .rsplit('/')
        .next()
        .unwrap_or(manifest_name)
        .trim_end_matches(".conf")
        .to_ascii_lowercase();
    for driver in ["ztext", "rawtext"] {
        let candidate = format!("modules/texts/{driver}/{module}");
        if !store.list(&format!("{candidate}/")).is_empty() {
            return candidate;
        }
    }
    format!("modules/texts/ztext/{module}")
}

/// Reads the module section of a `mods.d` manifest.
///
/// The section that declares a driver is the module; a manifest that declares
/// several is refused because one file is one translation.
fn parse_manifest(text: &str, name: &str) -> Result<BTreeMap<String, String>> {
    let mut sections: Vec<(String, BTreeMap<String, String>)> = Vec::new();

    for line in text.split('\n') {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(section) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            sections.push((section.trim().to_string(), BTreeMap::new()));
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some((_, entries)) = sections.last_mut() else {
            continue;
        };
        entries.insert(key.trim().to_string(), value.trim().to_string());
    }

    let mut modules = sections
        .into_iter()
        .filter(|(_, entries)| entries.contains_key("ModDrv"))
        .collect::<Vec<_>>();
    if modules.is_empty() {
        bail!("{name} declares no module, so it does not describe a translation");
    }
    if modules.len() > 1 {
        bail!(
            "{name} declares {} modules, point the import at one of them",
            modules.len()
        );
    }
    Ok(modules.remove(0).1)
}

/// What one open tag is doing to the text inside it.
#[derive(Clone, Copy, Debug)]
enum Entry {
    /// A tag that changes nothing about the text.
    Plain,
    /// A styled range that closes with this tag.
    Style(SpanStyle, u16),
    /// A title, whose text becomes a mark payload rather than verse text.
    Title(u16),
}

/// Strips one verse's markup, keeping its text and the styles the model carries.
///
/// The scanner is tolerant on purpose: module text is not always well-formed, and
/// a tag that never closes leaves its text in place rather than failing the import.
/// A title inside a verse is a Psalm superscription or a section heading, so its
/// text becomes a mark payload instead of being glued to the verse.
fn strip_markup(dialect: Markup, source: &str, accumulator: &mut VerseAccumulator) -> bool {
    let mut stack: Vec<Entry> = Vec::new();
    let mut skip = 0usize;
    let mut pos = 0usize;
    let mut red_letters = false;
    let mut title = String::new();
    let mut in_title = false;

    while pos < source.len() {
        let rest = &source[pos..];
        let Some(open) = rest.find('<') else {
            push(rest, skip, in_title, &mut title, accumulator);
            break;
        };
        if open > 0 {
            push(&rest[..open], skip, in_title, &mut title, accumulator);
        }
        pos += open;

        let Some(close) = source[pos..].find('>') else {
            // A tag that never closes is text, not structure.
            push(&source[pos..], skip, in_title, &mut title, accumulator);
            break;
        };
        let tag = &source[pos..=pos + close];
        pos += close + 1;

        let (closing, self_closing, name, attributes) = split_tag(tag);
        if name.is_empty() {
            continue;
        }

        if closing {
            if skip > 0 {
                if is_skip_tag(dialect, &name) {
                    skip -= 1;
                }
                continue;
            }
            match stack.pop() {
                Some(Entry::Style(style, start)) => accumulator.close_span(style, start),
                Some(Entry::Title(offset)) => {
                    in_title = false;
                    let payload = std::mem::take(&mut title);
                    let kind = if accumulator.book == PSALMS {
                        MarkKind::Superscription
                    } else {
                        MarkKind::Heading
                    };
                    accumulator.add_mark_at(offset, kind, 0, &payload);
                }
                _ => {}
            }
            continue;
        }

        if is_skip_tag(dialect, &name) {
            if !self_closing {
                skip += 1;
            }
            continue;
        }

        if name == "title" || name == "h" {
            if !self_closing {
                title.clear();
                in_title = true;
                stack.push(Entry::Title(accumulator.offset()));
            }
            continue;
        }

        let style = style_of(dialect, &name, &attributes);
        if style == Some(SpanStyle::WordsOfChrist) {
            red_letters = true;
        }
        if self_closing {
            continue;
        }
        match style {
            Some(style) => stack.push(Entry::Style(style, accumulator.offset())),
            None => stack.push(Entry::Plain),
        }
    }

    // A style left open at the end of the verse closes at the end of the text.
    for entry in stack.into_iter().rev() {
        match entry {
            Entry::Style(style, start) => accumulator.close_span(style, start),
            Entry::Title(offset) => {
                let kind = if accumulator.book == PSALMS {
                    MarkKind::Superscription
                } else {
                    MarkKind::Heading
                };
                accumulator.add_mark_at(offset, kind, 0, &title);
            }
            Entry::Plain => {}
        }
    }
    red_letters
}

/// Routes a run of text to the title being collected or to the verse itself.
fn push(
    text: &str,
    skip: usize,
    in_title: bool,
    title: &mut String,
    accumulator: &mut VerseAccumulator,
) {
    if skip > 0 {
        return;
    }
    if in_title {
        title.push_str(text);
    } else {
        accumulator.push_text(text);
    }
}

/// Splits a tag into its name, whether it closes, whether it is self-closing, and its attributes.
fn split_tag(tag: &str) -> (bool, bool, String, String) {
    let inner = tag.trim_start_matches('<').trim_end_matches('>').trim();
    let closing = inner.starts_with('/');
    let inner = inner.trim_start_matches('/').trim();
    let self_closing = inner.ends_with('/');
    let inner = inner.trim_end_matches('/').trim();
    let (name, attributes) = match inner.find(char::is_whitespace) {
        Some(split) => (&inner[..split], inner[split..].trim()),
        None => (inner, ""),
    };
    (
        closing,
        self_closing,
        name.to_ascii_lowercase(),
        attributes.to_string(),
    )
}

/// Whether a tag and everything inside it is apparatus the reader does not show.
fn is_skip_tag(dialect: Markup, name: &str) -> bool {
    match dialect {
        Markup::Osis => matches!(name, "note" | "rdg" | "rdggrp" | "ref"),
        Markup::Thml => matches!(name, "note" | "scripref"),
        Markup::Gbf => matches!(name, "fn" | "fr" | "ft" | "fo" | "fs" | "fi" | "cm" | "rf"),
    }
}

/// The style a tag applies, when the model carries one for it.
fn style_of(dialect: Markup, name: &str, attributes: &str) -> Option<SpanStyle> {
    match dialect {
        Markup::Osis => match name {
            "transchange" => Some(SpanStyle::TranslatorAddition),
            "divinename" => Some(SpanStyle::DivineName),
            "q" if attributes.to_ascii_lowercase().contains("jesus") => {
                Some(SpanStyle::WordsOfChrist)
            }
            _ => None,
        },
        Markup::Thml => match name {
            "span" | "div" if attributes.to_ascii_lowercase().contains("add") => {
                Some(SpanStyle::TranslatorAddition)
            }
            "span" | "div" if attributes.to_ascii_lowercase().contains("divine") => {
                Some(SpanStyle::DivineName)
            }
            _ => None,
        },
        Markup::Gbf => match name {
            "j" | "w" => Some(SpanStyle::WordsOfChrist),
            _ => None,
        },
    }
}
