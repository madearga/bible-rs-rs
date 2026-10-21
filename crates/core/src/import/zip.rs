// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! A zip reader that refuses the things an archive must never be allowed to do.
//!
//! An imported archive comes from the user's disk and may be hostile. This
//! reader refuses an entry whose path would escape the root it is extracted
//! into, an entry with an absolute path, a symlink, and an entry that expands
//! far beyond the space it takes up in the archive. A malformed archive becomes
//! an error with context, never a panic.

use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Largest compressed archive file this reader accepts, in bytes.
///
/// The bound is checked before the file is read, so a huge file cannot be
/// pulled into memory just to discover that it is not a zip.
pub const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;

/// Largest uncompressed size allowed for a single entry, in bytes.
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;

/// Largest total uncompressed size allowed for one archive, in bytes.
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// Most entries allowed in one archive.
pub const MAX_ENTRIES: usize = 8192;

/// Largest ratio of expanded size to stored size allowed for one entry.
pub const MAX_EXPANSION_RATIO: u64 = 200;

/// One file read out of an archive, with its path inside the archive.
#[derive(Clone, Debug)]
pub struct ArchiveEntry {
    /// Path of the entry inside the archive, always relative and free of `..`.
    pub name: String,
    /// The entry's uncompressed bytes.
    pub bytes: Vec<u8>,
}

/// Reads every file in an archive at `path` into memory.
///
/// Use this when the caller wants the files and not the archive's layout, such
/// as importing a zip of USFM documents or a zipped SWORD module.
pub fn read_archive(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let bytes = read_archive_file(path)?;
    read_archive_bytes(&bytes).with_context(|| format!("{} is not a usable zip", path.display()))
}

/// Lists the entry names of an archive without decompressing any of them.
///
/// Used to tell a zip of USFM documents from a zipped SWORD module before either
/// importer commits to reading it.
pub fn entry_names(path: &Path) -> Result<Vec<String>> {
    let bytes = read_archive_file(path)?;
    let mut archive = open(&bytes)?;
    if archive.len() > MAX_ENTRIES {
        bail!(
            "archive holds {} entries, more than the {MAX_ENTRIES} allowed",
            archive.len()
        );
    }
    let mut names = Vec::new();
    let mut seen = 0usize;
    for index in 0..archive.len() {
        let file = archive
            .by_index_raw(index)
            .with_context(|| format!("entry {index} cannot be read"))?;
        seen += 1;
        if seen > MAX_ENTRIES {
            bail!("archive holds more than {MAX_ENTRIES} entries, refusing to list it further");
        }
        if file.is_dir() {
            continue;
        }
        if let Ok(name) = safe_name(file.enclosed_name(), file.name()) {
            names.push(name);
        }
    }
    Ok(names)
}

/// Reads every file in an archive already held in memory.
pub fn read_archive_bytes(bytes: &[u8]) -> Result<Vec<ArchiveEntry>> {
    let mut archive = open(bytes)?;
    if archive.len() > MAX_ENTRIES {
        bail!(
            "archive holds {} entries, more than the {MAX_ENTRIES} allowed",
            archive.len()
        );
    }
    let mut entries = Vec::new();
    let mut total = 0u64;
    let mut seen = 0usize;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .with_context(|| format!("entry {index} cannot be read"))?;
        seen += 1;
        if seen > MAX_ENTRIES {
            bail!("archive holds more than {MAX_ENTRIES} entries, refusing to read it further");
        }
        let name = safe_name(file.enclosed_name(), file.name())?;
        reject_symlink(&file, &name)?;
        if file.is_dir() {
            continue;
        }
        let declared = file.size();
        let stored = file.compressed_size();
        check_expansion(&name, declared, stored)?;
        total = total
            .checked_add(declared)
            .context("archive expands past what a machine can hold")?;
        if total > MAX_TOTAL_BYTES {
            bail!(
                "archive expands to more than {MAX_TOTAL_BYTES} bytes, refusing to read it further"
            );
        }
        let body = read_limited(&mut file, &name, declared)?;
        entries.push(ArchiveEntry { name, bytes: body });
    }

    Ok(entries)
}

/// Extracts an archive under `root`, refusing anything that would land outside it.
///
/// Returns the paths written, all of which are inside `root`. The root is
/// created when it does not exist.
pub fn extract_archive(path: &Path, root: &Path) -> Result<Vec<PathBuf>> {
    let bytes = read_archive_file(path)?;
    let mut archive = open(&bytes)?;
    if archive.len() > MAX_ENTRIES {
        bail!(
            "archive holds {} entries, more than the {MAX_ENTRIES} allowed",
            archive.len()
        );
    }
    std::fs::create_dir_all(root).with_context(|| format!("cannot create {}", root.display()))?;
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", root.display()))?;

    let mut written = Vec::new();
    let mut total = 0u64;
    let mut seen = 0usize;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .with_context(|| format!("entry {index} cannot be read"))?;
        seen += 1;
        if seen > MAX_ENTRIES {
            bail!("archive holds more than {MAX_ENTRIES} entries, refusing to extract it further");
        }
        let name = safe_name(file.enclosed_name(), file.name())?;
        reject_symlink(&file, &name)?;
        let target = prepare_target(&root, &name)?;
        if file.is_dir() {
            match std::fs::symlink_metadata(&target) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => bail!(
                    "archive folder {:?} collides with a file at {}",
                    name,
                    target.display()
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(&target)
                        .with_context(|| format!("cannot create {}", target.display()))?;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("cannot inspect {}", target.display()));
                }
            }
            continue;
        }
        let declared = file.size();
        check_expansion(&name, declared, file.compressed_size())?;
        total = total
            .checked_add(declared)
            .context("archive expands past what a machine can hold")?;
        if total > MAX_TOTAL_BYTES {
            bail!(
                "archive expands to more than {MAX_TOTAL_BYTES} bytes, refusing to extract it further"
            );
        }

        let body = read_limited(&mut file, &name, declared)?;
        // Exclusive creation is the point: an existing symlink or hard link at
        // the destination makes this fail, so the write never follows one.
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .with_context(|| {
                format!(
                    "cannot create {} exclusively, the destination may already exist as a link",
                    target.display()
                )
            })?;
        output
            .write_all(&body)
            .with_context(|| format!("cannot write {}", target.display()))?;
        written.push(target);
    }

    Ok(written)
}

/// Reads a whole archive file under a hard size limit.
///
/// The declared file length is checked first and the read is bounded, so a file
/// that grows between the check and the read still cannot pass the limit.
fn read_archive_file(path: &Path) -> Result<Vec<u8>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let length = file
        .metadata()
        .with_context(|| format!("cannot inspect {}", path.display()))?
        .len();
    if length > MAX_ARCHIVE_BYTES {
        bail!(
            "{} is {length} bytes, more than the {MAX_ARCHIVE_BYTES} allowed for an archive",
            path.display()
        );
    }
    let mut bytes = Vec::with_capacity(length.min(1024 * 1024) as usize);
    file.take(MAX_ARCHIVE_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("cannot read {}", path.display()))?;
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        bail!(
            "{} grew past the {MAX_ARCHIVE_BYTES} byte limit while it was read",
            path.display()
        );
    }
    Ok(bytes)
}

/// Refuses an entry whose metadata says it is a symbolic link.
///
/// A zip can carry a link as its mode even when the entry looks like a file. The
/// extractor never creates such an entry, and the in-memory reader never follows
/// one, so it is refused wherever it appears.
fn reject_symlink<R: Read>(file: &zip::read::ZipFile<'_, R>, name: &str) -> Result<()> {
    if file
        .unix_mode()
        .is_some_and(|mode| mode & 0o170000 == 0o120000)
    {
        bail!("archive entry {name:?} is a symbolic link, which an import never follows");
    }
    Ok(())
}

/// Resolves the ancestors of a target under the root before anything is created.
///
/// Every existing ancestor must resolve inside the root, whether it is a real
/// folder or a link that points inside it. A missing ancestor is created one
/// component at a time, so the caller never calls `create_dir_all` on a path
/// whose shape has not been checked. The returned path is the target with its
/// ancestors resolved, and its leaf is not created here.
fn prepare_target(root: &Path, name: &str) -> Result<PathBuf> {
    let mut current = root.to_path_buf();
    let components = Path::new(name)
        .components()
        .map(|component| component.as_os_str().to_owned())
        .collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let last = index + 1 == components.len();
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let resolved = current
                    .canonicalize()
                    .with_context(|| format!("cannot resolve {}", current.display()))?;
                if !resolved.starts_with(root) {
                    bail!(
                        "archive entry {name:?} resolves outside the extraction root at {}",
                        resolved.display()
                    );
                }
                current = resolved;
            }
            Ok(metadata) if metadata.is_dir() => {
                let resolved = current
                    .canonicalize()
                    .with_context(|| format!("cannot resolve {}", current.display()))?;
                if !resolved.starts_with(root) {
                    bail!(
                        "archive entry {name:?} resolves outside the extraction root at {}",
                        resolved.display()
                    );
                }
                current = resolved;
            }
            Ok(_) if !last => {
                bail!("archive entry {name:?} has an ancestor that is not a folder");
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !last => {
                std::fs::create_dir(&current)
                    .with_context(|| format!("cannot create {}", current.display()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("cannot inspect {}", current.display()));
            }
        }
    }
    Ok(current)
}

/// Opens an archive from memory with a clear message when the bytes are not a zip.
fn open(bytes: &[u8]) -> Result<zip::ZipArchive<Cursor<&[u8]>>> {
    zip::ZipArchive::new(Cursor::new(bytes)).context("the archive header cannot be read")
}

/// Rejects an entry whose name would leave the extraction root.
///
/// The zip crate's `enclosed_name` already refuses `..` and absolute paths; the
/// name is then re-checked here component by component so the guarantee does not
/// rest on one library's behaviour.
fn safe_name(enclosed: Option<PathBuf>, raw: &str) -> Result<String> {
    if raw.starts_with('/') || raw.starts_with('\\') || raw.contains('\\') {
        bail!("archive entry {raw:?} has an absolute or non-portable path");
    }
    if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
        bail!("archive entry {raw:?} has an absolute or non-portable path");
    }

    let enclosed =
        enclosed.with_context(|| format!("archive entry {raw:?} escapes the extraction root"))?;

    for component in enclosed.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => bail!("archive entry {raw:?} escapes the extraction root"),
        }
    }

    let name = enclosed.to_string_lossy().replace('\\', "/");
    if name.is_empty() {
        bail!("archive entry {raw:?} has an empty path");
    }
    Ok(name)
}

/// Rejects an entry whose expanded size dwarfs the space it takes in the archive.
fn check_expansion(name: &str, declared: u64, stored: u64) -> Result<()> {
    if declared > MAX_ENTRY_BYTES {
        bail!(
            "archive entry {name:?} expands to {declared} bytes, more than the {MAX_ENTRY_BYTES} allowed"
        );
    }
    // Small entries are never a bomb, and their ratio is meaningless.
    if declared > 1024 * 1024 {
        let ratio = declared / stored.max(1);
        if ratio > MAX_EXPANSION_RATIO {
            bail!(
                "archive entry {name:?} expands {ratio} times its stored size, which is a decompression bomb"
            );
        }
    }
    Ok(())
}

/// Reads an entry, stopping one byte past the size the archive declared.
///
/// A header can lie about a size, so the read is bounded rather than trusted.
fn read_limited(reader: &mut impl Read, name: &str, declared: u64) -> Result<Vec<u8>> {
    let mut body = Vec::with_capacity(declared.min(1024 * 1024) as usize);
    let limit = declared
        .saturating_add(1)
        .min(MAX_ENTRY_BYTES.saturating_add(1));
    reader
        .take(limit)
        .read_to_end(&mut body)
        .with_context(|| format!("archive entry {name:?} cannot be decompressed"))?;
    if body.len() as u64 > declared {
        bail!(
            "archive entry {name:?} holds more bytes than the {} it declares",
            declared
        );
    }
    Ok(body)
}
