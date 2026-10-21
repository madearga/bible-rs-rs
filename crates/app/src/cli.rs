// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The command line interface, which scripts the same core crate the window
//! uses.
//!
//! Every verb reads and writes the files in the data folder, so a test can
//! point `BIBLE_DATA_DIR` at a scratch folder and assert on the output and the
//! exit code without a window.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use bible_core::format::{self, MAGIC};
use bible_core::study::Store;
use bible_core::{Reference, Translation, TranslationBuilder, TranslationMeta, books};

use crate::{BUNDLED_TRANSLATIONS, Library, check_translation_id, data_dir};

/// What to print when a caller asks for help or names a verb the app does not
/// have.
pub const USAGE: &str = "\
usage: bible <verb> [arguments]

verbs:
  read <reference> [--translation <id>]   print a verse or a chapter
  search <query> [--translation <id>] [--limit <n>]
                                          print the verses a query matches
  import <path> [--id <id>]               install a translation file
  translations                            list the translations
  export <path> [--format json|markdown]  write the study data out
  import-study <path>                     read exported study data back

with no arguments the reader window opens.";

/// Runs one command line invocation and returns the process exit code.
///
/// The code is zero when the verb succeeded, one when it failed, and two when
/// the arguments did not name a verb or were missing.
pub fn run(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(()) => 0,
        Err(error) => {
            if let Some(usage) = error.downcast_ref::<UsageError>() {
                eprintln!("{usage}\n\n{USAGE}");
                2
            } else {
                eprintln!("{error:#}");
                1
            }
        }
    }
}

/// Splits an argument list into verbs, positionals, and `--flag value` pairs.
fn dispatch(args: &[String]) -> Result<()> {
    let Some(verb) = args.first() else {
        return Err(UsageError("no verb given".to_owned()).into());
    };
    let (positional, flags) = split(&args[1..])?;
    let mut library = Library::discover();
    match verb.as_str() {
        "read" => read(&library, &positional, &flags),
        "search" => search(&library, &positional, &flags),
        "import" => import(&mut library, &positional, &flags),
        "translations" => translations(&library),
        "export" => export(&positional, &flags),
        "import-study" => import_study(&positional),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(UsageError(format!("unknown verb {other:?}")).into()),
    }
}

/// The error that makes the CLI print its usage and exit with code two.
#[derive(Debug)]
struct UsageError(String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

/// Separates positional arguments from `--flag value` pairs.
fn split(args: &[String]) -> Result<(Vec<String>, BTreeMap<String, String>)> {
    let mut positional = Vec::new();
    let mut flags = BTreeMap::new();
    let mut rest = args.iter();
    while let Some(argument) = rest.next() {
        if let Some(name) = argument.strip_prefix("--") {
            let value = rest
                .next()
                .ok_or_else(|| UsageError(format!("--{name} needs a value")))?;
            flags.insert(name.to_owned(), value.clone());
            continue;
        }
        if argument.starts_with('-') && argument.len() > 1 {
            return Err(UsageError(format!("unknown option {argument:?}")).into());
        }
        positional.push(argument.clone());
    }
    Ok((positional, flags))
}

/// Prints a verse or a chapter in the loaded translation.
fn read(library: &Library, positional: &[String], flags: &BTreeMap<String, String>) -> Result<()> {
    let Some(input) = positional.first() else {
        return Err(UsageError("read needs a reference".to_owned()).into());
    };
    let reference = Reference::parse(input)?;
    let translation = library.load(flags.get("translation").map(String::as_str))?;
    let abbreviation = translation.meta().abbreviation.clone();
    match reference.verse {
        Some(_) => {
            let verse = translation
                .verse(&reference)
                .with_context(|| format!("{reference} is not in the translation grid"))?;
            println!("{reference} ({abbreviation})");
            println!("{}", translation.verse_text(verse));
        }
        None => {
            let chapter = translation
                .chapter(reference.book, reference.chapter)
                .with_context(|| format!("{reference} is not in the translation grid"))?;
            println!("{reference} ({abbreviation})");
            for verse in chapter {
                let text = translation.verse_text(verse);
                if !text.trim().is_empty() {
                    println!("{}\t{}", verse.number, text);
                }
            }
        }
    }
    Ok(())
}

/// Prints every verse of the loaded translation that matches a query.
fn search(
    library: &Library,
    positional: &[String],
    flags: &BTreeMap<String, String>,
) -> Result<()> {
    let Some(query) = positional.first() else {
        return Err(UsageError("search needs a query".to_owned()).into());
    };
    let needle = query.to_lowercase();
    if needle.trim().is_empty() {
        return Err(UsageError("search needs a non-empty query".to_owned()).into());
    }
    let limit = match flags.get("limit") {
        Some(value) => Some(
            value
                .parse::<usize>()
                .map_err(|_| UsageError(format!("the limit {value:?} is not a number")))?,
        ),
        None => None,
    };
    let translation = library.load(flags.get("translation").map(String::as_str))?;
    let mut matches = 0usize;
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let Some(verses) = translation.chapter(book.index, chapter) else {
                continue;
            };
            for verse in verses {
                if limit.is_some_and(|limit| matches >= limit) {
                    return Ok(());
                }
                let text = translation.verse_text(verse);
                if text.to_lowercase().contains(&needle) {
                    let reference = Reference {
                        book: book.index,
                        chapter,
                        verse: Some(verse.number),
                    };
                    println!("{reference}\t{text}");
                    matches += 1;
                }
            }
        }
    }
    if matches == 0 {
        println!("no matches for {query:?}");
    }
    Ok(())
}

/// Prints the translations the app ships with and what is installed here.
///
/// The seven names and abbreviations come from each file's own header, so the
/// list cannot claim a name the embedded text does not carry.
fn translations(library: &Library) -> Result<()> {
    println!("id\tabbreviation\tname\tstatus");
    for bundled in &BUNDLED_TRANSLATIONS {
        let meta = bundled.meta()?;
        let status = if library.find(bundled.id()).is_some() {
            "bundled, installed locally"
        } else {
            "bundled"
        };
        println!(
            "{}\t{}\t{}\t{}",
            meta.id, meta.abbreviation, meta.name, status
        );
    }
    for entry in library.installed() {
        if BUNDLED_TRANSLATIONS
            .iter()
            .any(|bundled| bundled.id() == entry.meta.id)
        {
            continue;
        }
        println!(
            "{}\t{}\t{}\tinstalled",
            entry.meta.id, entry.meta.abbreviation, entry.meta.name
        );
    }
    Ok(())
}

/// Installs a translation file into the data folder.
///
/// A binary translation is decoded and re-encoded so a corrupt file is rejected
/// before it lands in the library. A text file in the documented
/// `Reference<TAB>text` form is converted onto the canonical grid first.
fn import(
    library: &mut Library,
    positional: &[String],
    flags: &BTreeMap<String, String>,
) -> Result<()> {
    let Some(path) = positional.first() else {
        return Err(UsageError("import needs a file path".to_owned()).into());
    };
    let path = PathBuf::from(path);
    let bytes = fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let translation = if bytes.starts_with(&MAGIC) {
        format::decode(&bytes)
            .with_context(|| format!("{} is not a translation", path.display()))?
    } else {
        let source = String::from_utf8(bytes)
            .with_context(|| format!("{} is neither binary nor UTF-8 text", path.display()))?;
        let id = match flags.get("id") {
            Some(id) => id.clone(),
            None => path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "imported".to_owned()),
        };
        check_translation_id(&id).with_context(|| format!("cannot import {}", path.display()))?;
        parse_text(&source, &id, &path)?
    };
    let installed = library.install(&translation)?;
    println!(
        "installed {} ({} verses) at {}",
        translation.meta().id,
        translation.verses().len(),
        installed.display()
    );
    Ok(())
}

/// Builds a full-grid translation from `Reference<TAB>text` lines.
///
/// This is the plain-text path the CLI accepts. Every verse the file does not
/// mention is stored as an empty string, which is how the grid holds a verse a
/// translation does not print.
fn parse_text(source: &str, id: &str, path: &Path) -> Result<Translation> {
    let mut meta = TranslationMeta::new(id, id);
    meta.abbreviation = id.to_uppercase();
    meta.language = "unknown".to_owned();
    meta.licence = "Imported by the user".to_owned();
    meta.provenance = path.display().to_string();
    meta.origin = "imported".to_owned();
    meta.book_titles = books::books()
        .iter()
        .map(|book| book.name.to_owned())
        .collect();

    let mut texts: BTreeMap<(u8, u8, u8), String> = BTreeMap::new();
    for (number, line) in source.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (reference, text) = line
            .split_once('\t')
            .with_context(|| format!("line {} is not `Reference<TAB>text`", number + 1))?;
        let reference = Reference::parse(reference)
            .with_context(|| format!("line {} has no usable reference", number + 1))?;
        let Some(verse) = reference.verse else {
            bail!(
                "line {} names a chapter, not a verse: {reference}",
                number + 1
            );
        };
        texts.insert((reference.book, reference.chapter, verse), text.to_owned());
    }
    if texts.is_empty() {
        bail!("{} has no verse lines", path.display());
    }

    let mut builder = TranslationBuilder::new(meta);
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter)
                .with_context(|| format!("{} {chapter} is off the grid", book.name))?;
            for verse in 1..=count {
                let key = (book.index, chapter, verse as u8);
                let text = texts.get(&key).map(String::as_str).unwrap_or("");
                builder
                    .push_verse(verse as u8, text)
                    .with_context(|| format!("cannot place {} {chapter}:{verse}", book.name))?;
            }
        }
    }
    builder.finish()
}

/// Writes the study data out as JSON or Markdown from the same SQLite store the
/// window opens. The export sees committed database data; a GUI store that is
/// still holding a throttled settings write in memory is not included until it
/// flushes or closes.
fn export(positional: &[String], flags: &BTreeMap<String, String>) -> Result<()> {
    let Some(path) = positional.first() else {
        return Err(UsageError("export needs an output path".to_owned()).into());
    };
    let format = flags.get("format").map(String::as_str).unwrap_or("json");
    if !matches!(format, "json" | "markdown" | "md") {
        return Err(UsageError(format!("no export format called {format:?}")).into());
    }
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    if is_live_database(&path) {
        bail!(
            "refusing to overwrite the live study database at {}",
            path.display()
        );
    }

    let data_dir = data_dir();
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("cannot create {}", data_dir.display()))?;
    let store = Store::open(data_dir.join("study.db"))?;
    // Opening the store can create the database, the WAL, and the lock file.
    // A link that was dangling before that point is an alias only now.
    if is_live_database(&path) {
        store.close()?;
        bail!(
            "refusing to overwrite the live study database at {}",
            path.display()
        );
    }
    let study = store.snapshot();
    match format {
        "json" => write_export(&path, &store.export_json()?)?,
        _ => write_export(&path, &store.export_markdown())?,
    }
    store.close()?;
    println!(
        "exported {} highlights, {} notes, {} bookmarks to {}",
        study.highlights.len(),
        study.notes.len(),
        study.bookmarks.len(),
        path.display()
    );
    Ok(())
}

/// Reads exported study data back into the same SQLite store the window opens.
///
/// The core store parses and checks the whole document before it runs the one
/// transaction that replaces the records, so a malformed file fails without
/// touching what is already there.
fn import_study(positional: &[String]) -> Result<()> {
    let Some(path) = positional.first() else {
        return Err(UsageError("import-study needs a file path".to_owned()).into());
    };
    let path = PathBuf::from(path);
    let text =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let data_dir = data_dir();
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("cannot create {}", data_dir.display()))?;
    let mut store = Store::open(data_dir.join("study.db"))?;
    store.import_json(&text)?;
    let study = store.snapshot();
    store.close()?;
    println!(
        "imported {} highlights, {} notes, {} bookmarks from {}",
        study.highlights.len(),
        study.notes.len(),
        study.bookmarks.len(),
        path.display()
    );
    Ok(())
}

/// Whether `path` names the GUI's live study database, or one of the SQLite
/// side files that sit beside it.
fn is_live_database(path: &Path) -> bool {
    live_store_files().iter().any(|live| same_file(path, live))
}

/// The database, its SQLite side files, and its lifetime lock.
fn live_store_files() -> Vec<PathBuf> {
    let database = data_dir().join("study.db");
    let mut files = vec![database];
    for suffix in ["-wal", "-shm", ".lock"] {
        let mut name = files[0].as_os_str().to_owned();
        name.push(suffix);
        files.push(PathBuf::from(name));
    }
    files
}

/// Whether two paths name the same file, following symlinks (including a
/// dangling link to a file that does not exist yet) and hard links.
fn same_file(left: &Path, right: &Path) -> bool {
    if let (Some(left), Some(right)) = (resolve(left), resolve(right))
        && left == right
    {
        return true;
    }
    same_existing_file(left, right)
}

/// Compares the identity of two existing files, which is how a hard link is
/// recognised as an alias.
#[cfg(unix)]
fn same_existing_file(left: &Path, right: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let identity = |path: &Path| fs::metadata(path).ok().map(|meta| (meta.dev(), meta.ino()));
    matches!(
        (identity(left), identity(right)),
        (Some(left), Some(right)) if left == right
    )
}

#[cfg(not(unix))]
fn same_existing_file(_left: &Path, _right: &Path) -> bool {
    false
}

/// Resolves a path against the current folder and follows every symlink even
/// when the final target does not exist yet, so two spellings of one path
/// compare equal.
fn resolve(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    for _ in 0..64 {
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = fs::read_link(&current).ok()?;
                current = if target.is_absolute() {
                    target
                } else {
                    current.parent()?.join(target)
                };
            }
            Ok(_) => return fs::canonicalize(&current).ok().or(Some(current)),
            Err(_) => {
                let parent = current.parent()?;
                let name = current.file_name()?;
                return Some(fs::canonicalize(parent).ok()?.join(name));
            }
        }
    }
    None
}

/// Writes an export through a new file of our own and renames it into place, so
/// an existing alias can never be truncated by the write.
fn write_export(path: &Path, contents: &str) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("{} has no file name", path.display()))?;
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut temporary_name = OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(".tmp-{}-{unique}", std::process::id()));
    let temporary = parent.join(temporary_name);
    write_export_at(path, &temporary, contents)
}

/// Writes an export through the temporary path it is given and renames it into
/// place. Only a temporary file this call created is cleaned up on failure.
fn write_export_at(path: &Path, temporary: &Path, contents: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .with_context(|| format!("cannot create {}", temporary.display()))?;

    let write = file
        .write_all(contents.as_bytes())
        .with_context(|| format!("cannot write {}", temporary.display()))
        .and_then(|()| {
            file.sync_all()
                .with_context(|| format!("cannot flush {}", temporary.display()))
        });
    drop(file);

    let result = write.and_then(|()| {
        fs::rename(temporary, path).with_context(|| format!("cannot write {}", path.display()))
    });
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::write_export_at;
    use std::path::PathBuf;

    /// A scratch folder for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bible-cli-unit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch folder");
        dir
    }

    #[test]
    fn a_temporary_file_this_call_did_not_create_survives_a_collision() {
        let dir = scratch("export-temp-collision");
        let destination = dir.join("out.json");
        let temporary = dir.join(".out.json.tmp-sentinel");
        std::fs::write(&temporary, "owned sentinel").expect("write the sentinel");

        let error = write_export_at(&destination, &temporary, "exported")
            .expect_err("an existing temporary path is rejected");
        assert!(error.to_string().contains("cannot create"), "{error:#}");
        assert_eq!(
            std::fs::read_to_string(&temporary).expect("the sentinel remains"),
            "owned sentinel"
        );
        assert!(!destination.exists(), "the destination was not written");
    }
}
