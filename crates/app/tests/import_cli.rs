// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The import verb tests. They run the real command line binary against a
//! scratch data folder and assert on its output, its exit code, and the files
//! it leaves behind.
//!
//! Every source here is synthetic. The USFM, OSIS, Zefania, and SWORD fixtures
//! carry generated words only, so no scripture is committed, and the data
//! folder is a temporary one, so the reader's own library is never touched.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bible_core::books;

/// The binary under test, built by cargo before the tests run.
const BINARY: &str = env!("CARGO_BIN_EXE_bible");

/// A distinctive, generated phrase the import tests look for after installing.
const WITNESS: &str = "synthetic witness words";

/// A scratch data folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-import-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data")).expect("scratch folder");
    dir
}

/// Runs the CLI against a data folder.
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BINARY)
        .args(args)
        .env("BIBLE_DATA_DIR", dir.join("data"))
        .output()
        .expect("the binary runs")
}

/// The standard output of a finished run, as text.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The standard error of a finished run, as text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The translations folder of a scratch data folder.
fn translations(dir: &Path) -> PathBuf {
    dir.join("data").join("translations")
}

/// Every file in the translations folder, sorted by name.
fn installed_files(dir: &Path) -> Vec<String> {
    let mut names = match std::fs::read_dir(translations(dir)) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

/// A USFM document holding Genesis 1:1 and John 3:16, in generated words.
fn usfm_document() -> String {
    "\\id GEN\n\\c 1\n\\v 1 alpha beta gamma generated words.\n\\c 2\n\\v 1 delta epsilon.\n"
        .to_owned()
}

/// The second half of the USFM fixture, as its own document.
fn usfm_john() -> String {
    format!("\\id JHN\n\\c 3\n\\v 16 {WITNESS} in a generated verse.\n")
}

/// An OSIS XML document holding Genesis 1:1 and John 3:16.
fn osis_document() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <osis xmlns=\"http://www.bibletechnologies.net/2003/OSIS/namespace\">\n\
         <osisText osisIDWork=\"demo\">\n\
         <header><work osisWork=\"demo\"><title>Generated Demo</title></work></header>\n\
         <div type=\"book\" osisID=\"Gen\"><chapter osisID=\"Gen.1\">\
         <verse osisID=\"Gen.1.1\">alpha beta gamma generated words.</verse>\
         </chapter></div>\n\
         <div type=\"book\" osisID=\"John\"><chapter osisID=\"John.3\">\
         <verse osisID=\"John.3.16\">{WITNESS} in a generated verse.</verse>\
         </chapter></div>\n\
         </osisText>\n\
         </osis>\n"
    )
}

/// A Zefania XML document holding Genesis 1:1 and John 3:16.
fn zefania_document() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <XMLBIBLE biblename=\"Generated Demo\">\n\
         <BIBLEBOOK bnumber=\"1\" bname=\"Genesis\"><CHAPTER cnumber=\"1\">\
         <VERS vnumber=\"1\">alpha beta gamma generated words.</VERS>\
         </CHAPTER></BIBLEBOOK>\n\
         <BIBLEBOOK bnumber=\"43\" bname=\"John\"><CHAPTER cnumber=\"3\">\
         <VERS vnumber=\"16\">{WITNESS} in a generated verse.</VERS>\
         </CHAPTER></BIBLEBOOK>\n\
         </XMLBIBLE>\n"
    )
}

/// Builds the verse index and text body of one RawText SWORD testament.
///
/// The module's record layout is two testament records, then one record per
/// book, one per chapter, and one per verse, so every generated verse sits at a
/// predictable offset. Only the verses the closure names carry text.
fn rawtext_testament(
    range: std::ops::Range<u8>,
    text_for: impl Fn(u8, u8, u8) -> Option<String>,
) -> (Vec<u8>, Vec<u8>) {
    let mut entries = vec![String::new(), String::new()];
    for book_index in range {
        let book = books::book(book_index).expect("a canonical book");
        entries.push(String::new());
        for chapter in 1..=book.chapter_count {
            entries.push(String::new());
            let count = books::verses_in_chapter(book_index, chapter).expect("a real chapter");
            for verse in 1..=count as u8 {
                entries.push(text_for(book_index, chapter, verse).unwrap_or_default());
            }
        }
    }

    let mut body = Vec::new();
    let mut index = Vec::new();
    for entry in entries {
        let start = u32::try_from(body.len()).expect("the body fits a u32");
        let length = u16::try_from(entry.len()).expect("the verse fits a u16");
        index.extend_from_slice(&start.to_le_bytes());
        index.extend_from_slice(&length.to_le_bytes());
        body.extend_from_slice(entry.as_bytes());
    }
    (index, body)
}

/// Writes a RawText SWORD module into `root` and returns the manifest path.
///
/// `cipher_key` is written verbatim, so an empty value produces a locked module.
fn write_sword_module(root: &Path, cipher_key: &str) {
    std::fs::create_dir_all(root.join("mods.d")).expect("mods.d");
    let data = root.join("modules/texts/rawtext/synth");
    std::fs::create_dir_all(&data).expect("the module folder");
    std::fs::write(
        root.join("mods.d/synth.conf"),
        format!(
            "[Synth]\nModDrv=RawText\nDataPath=modules/texts/rawtext/synth\n\
             Versification=KJV\nCipherKey={cipher_key}\n\
             Description=Synthetic Witness Module\nLang=en\n"
        ),
    )
    .expect("the manifest");

    let (ot_index, ot_body) = rawtext_testament(0..39, |book, chapter, verse| {
        (book == 0 && chapter == 1 && verse == 1)
            .then(|| "alpha beta gamma generated words.".to_owned())
    });
    let (nt_index, nt_body) = rawtext_testament(39..66, |book, chapter, verse| {
        (book == 42 && chapter == 3 && verse == 16).then(|| format!("{WITNESS}."))
    });
    std::fs::write(data.join("ot"), ot_body).expect("the OT body");
    std::fs::write(data.join("ot.vss"), ot_index).expect("the OT index");
    std::fs::write(data.join("nt"), nt_body).expect("the NT body");
    std::fs::write(data.join("nt.vss"), nt_index).expect("the NT index");
}

/// Computes the CRC-32 checksum a zip entry header needs.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// Writes a zip archive whose entries are stored without compression.
///
/// The archive is hand built so the tests need no extra dependency. It writes
/// the entry names exactly as given, which is how a crafted name reaches the
/// reader's validation.
fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut local = Vec::new();
    let mut central = Vec::new();
    for (name, bytes) in entries {
        let offset = u32::try_from(local.len()).expect("the archive fits a u32");
        let crc = crc32(bytes);
        let size = u32::try_from(bytes.len()).expect("the entry fits a u32");
        let name_len = u16::try_from(name.len()).expect("the name fits a u16");

        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&20u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&name_len.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes());
        local.extend_from_slice(name.as_bytes());
        local.extend_from_slice(bytes);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }

    let central_offset = u32::try_from(local.len()).expect("the archive fits a u32");
    let central_size = u32::try_from(central.len()).expect("the table fits a u32");
    let count = u16::try_from(entries.len()).expect("the entry count fits a u16");

    let mut archive = local;
    archive.extend_from_slice(&central);
    archive.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&central_size.to_le_bytes());
    archive.extend_from_slice(&central_offset.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes());
    std::fs::write(path, archive).expect("the archive");
}

/// Installs a source through the import verb and fails the test with the
/// binary's own message when the command did not succeed.
fn import(dir: &Path, path: &Path, flags: &[&str]) -> String {
    let mut args = vec!["import", path.to_str().expect("a path")];
    args.extend_from_slice(flags);
    let output = run(dir, &args);
    assert!(
        output.status.success(),
        "import failed: {}",
        stderr(&output)
    );
    stdout(&output)
}

/// Asserts that a source failed to import, that nothing was installed, and that
/// the failure carried a message.
fn assert_refused(dir: &Path, path: &Path, flags: &[&str]) -> String {
    let mut args = vec!["import", path.to_str().expect("a path")];
    args.extend_from_slice(flags);
    let output = run(dir, &args);
    assert!(
        !output.status.success(),
        "the import should have failed: {}",
        stdout(&output)
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = stderr(&output);
    assert!(!message.trim().is_empty(), "the failure carried no message");
    assert!(
        installed_files(dir).is_empty(),
        "a refused import installed {}",
        installed_files(dir).join(", ")
    );
    message
}

#[test]
fn a_usfm_file_imports_and_reads_back() {
    let dir = scratch("usfm-file");
    let source = dir.join("gen.usfm");
    std::fs::write(&source, format!("{}{}", usfm_document(), usfm_john())).expect("the source");

    let reported = import(
        &dir,
        &source,
        &[
            "--id",
            "demo",
            "--abbreviation",
            "DM",
            "--name",
            "Generated Demo",
            "--language",
            "en",
            "--licence",
            "MIT for generated words",
            "--copyright",
            "Copyright 2026 the test",
            "--provenance",
            "a synthetic USFM fixture",
        ],
    );
    assert!(reported.contains("installed demo"), "{reported}");
    assert!(reported.contains("from USFM"), "{reported}");
    assert_eq!(installed_files(&dir), vec!["demo.bbl".to_owned()]);

    let read = run(&dir, &["read", "John 3:16", "--translation", "demo"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS), "{}", stdout(&read));
    let genesis = run(&dir, &["read", "Genesis 1:1", "--translation", "demo"]);
    assert!(stdout(&genesis).contains("alpha beta gamma"));
}

#[test]
fn the_supplied_metadata_reaches_the_listing() {
    let dir = scratch("metadata");
    let source = dir.join("gen.usfm");
    std::fs::write(&source, usfm_document()).expect("the source");
    import(
        &dir,
        &source,
        &[
            "--id",
            "demo",
            "--abbreviation",
            "DM",
            "--name",
            "Generated Demo",
            "--language",
            "en",
            "--licence",
            "MIT for generated words",
            "--copyright",
            "Copyright 2026 the test",
        ],
    );

    let listed = run(&dir, &["translations"]);
    assert!(listed.status.success(), "{}", stderr(&listed));
    let text = stdout(&listed);
    assert!(
        text.contains("demo\tDM\tGenerated Demo\tinstalled"),
        "{text}"
    );
}

#[test]
fn a_usfm_folder_imports_every_document() {
    let dir = scratch("usfm-folder");
    let folder = dir.join("usfm");
    std::fs::create_dir_all(&folder).expect("the folder");
    std::fs::write(folder.join("01-gen.usfm"), usfm_document()).expect("the first document");
    std::fs::write(folder.join("02-jhn.usfm"), usfm_john()).expect("the second document");

    let reported = import(&dir, &folder, &["--id", "demo"]);
    assert!(reported.contains("from USFM"), "{reported}");
    let read = run(&dir, &["read", "John 3:16", "--translation", "demo"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
}

#[test]
fn a_zip_of_usfm_documents_imports() {
    let dir = scratch("usfm-zip");
    let archive = dir.join("bundle.zip");
    let genesis = usfm_document().into_bytes();
    let john = usfm_john().into_bytes();
    write_zip(&archive, &[("gen.usfm", &genesis), ("jhn.usfm", &john)]);

    let reported = import(&dir, &archive, &["--id", "demo"]);
    assert!(reported.contains("from USFM"), "{reported}");
    let read = run(&dir, &["read", "John 3:16", "--translation", "demo"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
}

#[test]
fn an_osis_file_imports() {
    let dir = scratch("osis");
    let source = dir.join("demo.osis");
    std::fs::write(&source, osis_document()).expect("the source");

    let reported = import(&dir, &source, &["--id", "demo"]);
    assert!(reported.contains("from OSIS"), "{reported}");
    let read = run(&dir, &["read", "John 3:16", "--translation", "demo"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
    let genesis = run(&dir, &["read", "Genesis 1:1", "--translation", "demo"]);
    assert!(stdout(&genesis).contains("alpha beta gamma"));
}

#[test]
fn a_zefania_file_imports() {
    let dir = scratch("zefania");
    let source = dir.join("demo.xml");
    std::fs::write(&source, zefania_document()).expect("the source");

    let reported = import(&dir, &source, &["--id", "demo"]);
    assert!(reported.contains("from Zefania"), "{reported}");
    let read = run(&dir, &["read", "John 3:16", "--translation", "demo"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
    let genesis = run(&dir, &["read", "Genesis 1:1", "--translation", "demo"]);
    assert!(stdout(&genesis).contains("alpha beta gamma"));
}

#[test]
fn a_sword_module_folder_imports() {
    let dir = scratch("sword-folder");
    let module = dir.join("module");
    write_sword_module(&module, "the-users-own-key");

    let reported = import(&dir, &module, &["--id", "demo"]);
    assert!(reported.contains("from SWORD"), "{reported}");
    let read = run(&dir, &["read", "John 3:16", "--translation", "demo"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
}

#[test]
fn a_sword_manifest_file_imports() {
    let dir = scratch("sword-conf");
    let module = dir.join("module");
    write_sword_module(&module, "the-users-own-key");
    let manifest = module.join("mods.d/synth.conf");

    let reported = import(&dir, &manifest, &["--id", "demo"]);
    assert!(reported.contains("from SWORD"), "{reported}");
    let read = run(&dir, &["read", "John 3:16", "--translation", "demo"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
}

#[test]
fn a_locked_sword_module_is_refused_with_its_reason() {
    let dir = scratch("sword-locked");
    let module = dir.join("module");
    write_sword_module(&module, "");

    let message = assert_refused(&dir, &module, &["--id", "demo"]);
    assert!(message.contains("locked"), "{message}");

    let with_key = assert_refused(
        &dir,
        &module,
        &["--id", "demo", "--unlock-key", "a-key-the-user-owns"],
    );
    assert!(with_key.contains("does not decipher"), "{with_key}");
}

#[test]
fn a_malformed_usfm_file_is_refused() {
    let dir = scratch("bad-usfm");
    let source = dir.join("bad.usfm");
    std::fs::write(&source, "\\id GEN\n\\c 1\n").expect("the source");
    let message = assert_refused(&dir, &source, &["--id", "demo"]);
    assert!(message.contains("holds no verse"), "{message}");
}

#[test]
fn a_malformed_osis_file_is_refused() {
    let dir = scratch("bad-osis");
    let source = dir.join("bad.osis");
    std::fs::write(&source, "<osis><osisText><verse osisID=\"Gen.1.1\">").expect("the source");
    assert_refused(&dir, &source, &["--id", "demo"]);
}

#[test]
fn a_malformed_zefania_file_is_refused() {
    let dir = scratch("bad-zefania");
    let source = dir.join("bad.xml");
    std::fs::write(&source, "<XMLBIBLE><BIBLEBOOK bname=\"Genesis\">").expect("the source");
    assert_refused(&dir, &source, &["--id", "demo"]);
}

#[test]
fn a_malformed_sword_manifest_is_refused() {
    let dir = scratch("bad-sword");
    let module = dir.join("module");
    std::fs::create_dir_all(module.join("mods.d")).expect("mods.d");
    let manifest = module.join("mods.d/bad.conf");
    std::fs::write(&manifest, "[Bad]\nDriver=RawText\n").expect("the manifest");
    assert_refused(&dir, &manifest, &["--id", "demo"]);
}

#[test]
fn a_crafted_archive_cannot_escape_the_data_folder() {
    let dir = scratch("crafted-archive");
    let archive = dir.join("hostile.zip");
    let usfm = usfm_document().into_bytes();
    write_zip(
        &archive,
        &[("../../escape.usfm", &usfm), ("gen.usfm", &usfm)],
    );

    let message = assert_refused(&dir, &archive, &["--id", "demo"]);
    assert!(message.contains("escape"), "{message}");
    assert!(
        !dir.join("escape.usfm").exists(),
        "the archive wrote outside the translations folder"
    );
    assert!(
        !dir.join("data/escape.usfm").exists(),
        "the archive wrote outside the translations folder"
    );
}

#[test]
fn an_identifier_cannot_leave_the_translations_folder() {
    let dir = scratch("hostile-id");
    let source = dir.join("gen.usfm");
    std::fs::write(&source, usfm_document()).expect("the source");

    let message = assert_refused(&dir, &source, &["--id", "../../escape"]);
    assert!(message.contains("plain file name"), "{message}");
    assert!(!dir.join("escape.bbl").exists());
}

#[test]
fn a_plain_text_file_still_imports() {
    let dir = scratch("text");
    let source = dir.join("witness.txt");
    std::fs::write(
        &source,
        format!("# generated test words, no scripture\nJohn 3:16\t{WITNESS} from a file\nGenesis 1:1\talpha beta gamma from a file\n"),
    )
    .expect("the source");

    let reported = import(&dir, &source, &["--id", "mine"]);
    assert!(reported.contains("from text"), "{reported}");
    let read = run(&dir, &["read", "John 3:16", "--translation", "mine"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
}

#[test]
fn the_apps_own_binary_format_still_imports() {
    let dir = scratch("binary");
    let source = dir.join("witness.txt");
    std::fs::write(
        &source,
        format!("John 3:16\t{WITNESS} from a file\nGenesis 1:1\talpha beta gamma from a file\n"),
    )
    .expect("the source");
    import(&dir, &source, &["--id", "first"]);

    let binary = dir.join("first-copy.bbl");
    std::fs::copy(translations(&dir).join("first.bbl"), &binary).expect("the copy");
    // A binary translation carries its own identity, so the verb installs it
    // under the identifier in its header rather than the one on the command line.
    let reported = import(&dir, &binary, &["--id", "second"]);
    assert!(reported.contains("from binary"), "{reported}");
    assert!(reported.contains("installed first"), "{reported}");

    let read = run(&dir, &["read", "John 3:16", "--translation", "first"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains(WITNESS));
}

#[test]
fn an_import_that_names_no_format_fails_clearly() {
    let dir = scratch("unknown");
    let source = dir.join("notes.dat");
    std::fs::write(&source, [0xffu8, 0xfe, 0x00, 0x01]).expect("the source");
    let message = assert_refused(&dir, &source, &["--id", "demo"]);
    assert!(message.contains("not a USFM"), "{message}");
}
