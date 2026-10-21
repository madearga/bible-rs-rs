// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Archive tests. The archives here are built by hand so a hostile entry can be
//! written exactly as the attack would write it.

use std::path::{Path, PathBuf};

use bible_core::import::zip::{
    MAX_ARCHIVE_BYTES, MAX_ENTRIES, MAX_ENTRY_BYTES, entry_names, extract_archive, read_archive,
    read_archive_bytes,
};
use bible_core::{ImportFormat, TranslationMeta, import_path};

/// One entry of a synthetic archive.
struct Entry<'a> {
    /// The path the entry claims inside the archive.
    name: &'a str,
    /// The bytes the entry stores.
    data: &'a [u8],
    /// The size the entry claims to expand to, which a bomb misstates.
    declared: u32,
    /// The Unix mode the entry carries, which is how a zip spells a symlink.
    mode: u32,
}

/// Builds a stored zip archive in memory.
fn archive(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();
    let mut count = 0u16;

    for entry in entries {
        let offset = out.len() as u32;
        let name = entry.name.as_bytes();
        let crc = crc32(entry.data);

        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0x21u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&entry.declared.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(entry.data);

        directory.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        directory.extend_from_slice(&0x0314u16.to_le_bytes());
        directory.extend_from_slice(&20u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0x21u16.to_le_bytes());
        directory.extend_from_slice(&crc.to_le_bytes());
        directory.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
        directory.extend_from_slice(&entry.declared.to_le_bytes());
        directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes());
        directory.extend_from_slice(&entry.mode.wrapping_shl(16).to_le_bytes());
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name);
        count += 1;
    }

    let directory_offset = out.len() as u32;
    let directory_size = directory.len() as u32;
    out.extend_from_slice(&directory);

    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&directory_size.to_le_bytes());
    out.extend_from_slice(&directory_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// The CRC-32 a zip entry carries so a reader can check it.
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 == 1 {
                (value >> 1) ^ 0xedb8_8320
            } else {
                value >> 1
            };
        }
        *slot = value;
    }
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc = table[((crc ^ u32::from(*byte)) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffff_ffff
}

/// A folder under the system temp folder, removed again when the test ends.
struct Scratch {
    /// The folder itself.
    path: PathBuf,
}

impl Scratch {
    /// Creates a fresh folder for one test.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("bible-import-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the scratch folder is created");
        Self { path }
    }

    /// Writes an archive into the scratch folder.
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, bytes).expect("the archive is written");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Every file under a folder, as paths relative to it, in name order.
fn files_under(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_path_that_escapes_the_root_is_refused() {
    let scratch = Scratch::new("traversal");
    let path = scratch.write(
        "evil.zip",
        &archive(&[
            Entry {
                name: "good.txt",
                data: b"expected",
                declared: 8,
                mode: 0o100644,
            },
            Entry {
                name: "../escape.txt",
                data: b"escaped",
                declared: 7,
                mode: 0o100644,
            },
        ]),
    );

    let result = read_archive(&path);
    assert!(
        result.is_err(),
        "an entry escaping the root must be refused"
    );

    let root = scratch.path.join("out");
    let extracted = extract_archive(&path, &root);
    assert!(extracted.is_err(), "extraction must refuse the same entry");
    assert_eq!(
        files_under(&root),
        vec!["good.txt".to_string()],
        "the extraction root holds only the entry that was safe"
    );
}

#[test]
fn an_absolute_path_is_refused() {
    let scratch = Scratch::new("absolute");
    let path = scratch.write(
        "absolute.zip",
        &archive(&[Entry {
            name: "/etc/passwd",
            data: b"nope",
            declared: 4,
            mode: 0o100644,
        }]),
    );
    assert!(read_archive(&path).is_err());
    assert!(extract_archive(&path, &scratch.path.join("out")).is_err());
}

#[test]
fn an_entry_that_expands_to_a_huge_size_is_refused() {
    let scratch = Scratch::new("bomb");
    let path = scratch.write(
        "bomb.zip",
        &archive(&[Entry {
            name: "bomb.txt",
            data: b"tiny",
            declared: u32::try_from(MAX_ENTRY_BYTES + 1).expect("the size fits"),
            mode: 0o100644,
        }]),
    );
    let error = read_archive(&path).expect_err("a decompression bomb must be refused");
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("expands")),
        "the error says why: {error:?}"
    );
}

#[test]
fn an_entry_that_expands_far_beyond_its_stored_size_is_refused() {
    let scratch = Scratch::new("ratio");
    let path = scratch.write(
        "ratio.zip",
        &archive(&[Entry {
            name: "ratio.txt",
            data: b"tiny",
            declared: 4 * 1024 * 1024,
            mode: 0o100644,
        }]),
    );
    let error = read_archive(&path).expect_err("a decompression bomb must be refused");
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("decompression bomb")),
        "the error says why: {error:?}"
    );
}

#[test]
fn a_truncated_archive_is_an_error_and_not_a_panic() {
    let scratch = Scratch::new("truncated");
    let bytes = archive(&[Entry {
        name: "good.txt",
        data: b"expected",
        declared: 8,
        mode: 0o100644,
    }]);
    let truncated = &bytes[..bytes.len() / 2];
    let path = scratch.write("truncated.zip", truncated);
    assert!(read_archive(&path).is_err());
    assert!(read_archive_bytes(truncated).is_err());
    assert!(read_archive_bytes(b"not a zip at all").is_err());
}

#[test]
fn a_plain_archive_reads_back_its_files() {
    let scratch = Scratch::new("plain");
    let path = scratch.write(
        "plain.zip",
        &archive(&[Entry {
            name: "folder/good.txt",
            data: b"expected",
            declared: 8,
            mode: 0o100644,
        }]),
    );
    let entries = read_archive(&path).expect("the archive reads");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "folder/good.txt");
    assert_eq!(entries[0].bytes, b"expected");

    let root = scratch.path.join("out");
    extract_archive(&path, &root).expect("the archive extracts");
    assert_eq!(files_under(&root), vec!["folder/good.txt".to_string()]);
}

#[test]
fn a_zip_of_usfm_documents_is_recognised_and_imported() {
    let scratch = Scratch::new("usfm");
    let path = scratch.write(
        "documents.zip",
        &archive(&[
            Entry {
                name: "gen.usfm",
                data: b"\\id GEN\n\\c 1\n\\v 1 Alpha one.\n",
                declared: 29,
                mode: 0o100644,
            },
            Entry {
                name: "texts/matt.sfm",
                data: b"\\id MAT\n\\c 1\n\\v 1 Delta four.\n",
                declared: 31,
                mode: 0o100644,
            },
        ]),
    );

    let names = entry_names(&path).expect("the names are listed");
    assert_eq!(names, vec!["gen.usfm", "texts/matt.sfm"]);
    assert_eq!(ImportFormat::detect(&path), Some(ImportFormat::Usfm));

    let translation = import_path(&path, TranslationMeta::new("synthetic", "S"), None)
        .expect("the archive imports");
    let genesis = translation.verse_at(0, 1, 1).expect("the slot exists");
    assert_eq!(translation.verse_text(genesis), "Alpha one.");
    let matthew = translation.verse_at(39, 1, 1).expect("the slot exists");
    assert_eq!(translation.verse_text(matthew), "Delta four.");
}

#[test]
fn an_entry_that_is_a_symlink_is_refused() {
    let scratch = Scratch::new("symlink");
    let path = scratch.write(
        "symlink.zip",
        &archive(&[Entry {
            name: "link.txt",
            data: b"../../outside.txt",
            declared: 17,
            mode: 0o120777,
        }]),
    );

    let error = read_archive(&path).expect_err("a link entry is refused");
    assert!(
        format!("{error:#}").contains("symbolic link"),
        "the error says why: {error:#}"
    );
    let extracted = extract_archive(&path, &scratch.path.join("out"));
    assert!(extracted.is_err(), "extraction refuses the link too");
    assert!(
        !scratch.path.join("outside.txt").exists(),
        "nothing was written through the link"
    );
}

#[test]
fn an_existing_link_at_the_destination_is_not_followed() {
    let scratch = Scratch::new("destination-link");
    let path = scratch.write(
        "plain.zip",
        &archive(&[Entry {
            name: "escaped.txt",
            data: b"expected",
            declared: 8,
            mode: 0o100644,
        }]),
    );
    let outside = scratch.path.join("outside.txt");
    std::fs::write(&outside, "untouched sentinel").expect("write the sentinel");
    let root = scratch.path.join("out");
    std::fs::create_dir_all(&root).expect("create the root");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("escaped.txt")).expect("plant the link");

    let error = extract_archive(&path, &root).expect_err("the planted link is refused");
    assert!(
        format!("{error:#}").contains("outside the extraction root"),
        "the error says why: {error:#}"
    );
    assert_eq!(
        std::fs::read_to_string(&outside).expect("the sentinel remains"),
        "untouched sentinel",
        "the link target was not written through"
    );
}

#[test]
fn the_entry_limit_counts_directory_entries() {
    let scratch = Scratch::new("directories");
    let names = (0..=MAX_ENTRIES)
        .map(|index| format!("folder{index}/"))
        .collect::<Vec<_>>();
    let entries = names
        .iter()
        .map(|name| Entry {
            name,
            data: b"",
            declared: 0,
            mode: 0o040755,
        })
        .collect::<Vec<_>>();
    let path = scratch.write("directories.zip", &archive(&entries));

    let error = read_archive(&path).expect_err("the entry limit includes folders");
    let message = format!("{error:#}");
    assert!(
        message.contains(&format!("{} entries", MAX_ENTRIES + 1)),
        "{message}"
    );
    assert!(message.contains("allowed"), "{message}");
    assert!(
        extract_archive(&path, &scratch.path.join("out")).is_err(),
        "extraction applies the same limit"
    );
}

#[test]
fn an_archive_file_past_the_input_limit_is_refused() {
    let scratch = Scratch::new("oversized");
    let path = scratch.path.join("oversized.zip");
    // A sparse file of the declared length costs no disk space, and the limit is
    // checked before anything is read.
    let file = std::fs::File::create(&path).expect("create the file");
    file.set_len(MAX_ARCHIVE_BYTES + 1).expect("set the length");

    let error = entry_names(&path).expect_err("the input limit is applied when listing");
    let message = format!("{error:#}");
    assert!(message.contains("more than"), "{message}");
    assert!(message.contains("allowed for an archive"), "{message}");
    assert!(
        read_archive(&path).is_err(),
        "reading applies the same limit"
    );
}
