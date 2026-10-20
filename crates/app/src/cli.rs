// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The command line interface, which scripts the same core crate the window
//! uses.
//!
//! Every verb reads and writes the files in the data folder, so a test can
//! point `BIBLE_DATA_DIR` at a scratch folder and assert on the output and the
//! exit code without a window.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bible_core::format::{self, MAGIC};
use bible_core::{Reference, Translation, TranslationBuilder, TranslationMeta, books};

use crate::{BUNDLED_TRANSLATIONS, Library, StudyData, data_dir};

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

/// Writes the study data out as JSON or Markdown.
fn export(positional: &[String], flags: &BTreeMap<String, String>) -> Result<()> {
    let Some(path) = positional.first() else {
        return Err(UsageError("export needs an output path".to_owned()).into());
    };
    let format = flags.get("format").map(String::as_str).unwrap_or("json");
    let study = StudyData::load(&data_dir().join("study.json"))?;
    let path = PathBuf::from(path);
    match format {
        "json" => study.save(&path)?,
        "markdown" | "md" => {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)
                    .with_context(|| format!("cannot create {}", parent.display()))?;
            }
            fs::write(&path, study.to_markdown())
                .with_context(|| format!("cannot write {}", path.display()))?;
        }
        other => {
            return Err(UsageError(format!("no export format called {other:?}")).into());
        }
    }
    println!(
        "exported {} highlights, {} notes, {} bookmarks to {}",
        study.highlights.len(),
        study.notes.len(),
        study.bookmarks.len(),
        path.display()
    );
    Ok(())
}

/// Reads exported study data back into the data folder.
fn import_study(positional: &[String]) -> Result<()> {
    let Some(path) = positional.first() else {
        return Err(UsageError("import-study needs a file path".to_owned()).into());
    };
    let path = PathBuf::from(path);
    let study = StudyData::load(&path)?;
    let target = data_dir().join("study.json");
    study.save(&target)?;
    println!(
        "imported {} highlights, {} notes, {} bookmarks from {}",
        study.highlights.len(),
        study.notes.len(),
        study.bookmarks.len(),
        path.display()
    );
    Ok(())
}
