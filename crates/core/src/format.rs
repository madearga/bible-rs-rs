// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The binary on-disk format for a translation.
//!
//! The layout is fixed. A four-byte magic and a version lead, then an
//! uncompressed JSON header holding the metadata, then the compressed body. The
//! body is column-oriented: all verse numbers together, all text lengths
//! together, and so on, which compresses far better than interleaved records and
//! decodes in a few linear passes.
//!
//! ```text
//! "BIBL" u16 version u32 meta_len meta_json
//! u32 body_len zstd(body)
//! ```

use anyhow::{Result, bail};

use crate::books::{CHAPTER_COUNT, VERSES_PER_CHAPTER};
use crate::model::{Mark, MarkKind, Span, SpanStyle, Translation, TranslationMeta, Verse};

/// The four bytes every translation file starts with.
pub const MAGIC: [u8; 4] = *b"BIBL";

/// The format version this crate reads and writes.
pub const FORMAT_VERSION: u16 = 1;

/// The zstd level used when writing; a middle setting keeps decode fast and files small.
const COMPRESSION_LEVEL: i32 = 3;

/// Encodes a translation into the binary format.
///
/// The metadata header stays uncompressed so a translation list can be read
/// without decompressing the body.
pub fn encode(translation: &Translation) -> Result<Vec<u8>> {
    let meta_json = serde_json::to_vec(translation.meta())?;
    let body = encode_body(translation)?;
    let compressed = zstd::bulk::compress(&body, COMPRESSION_LEVEL)?;

    let mut out = Vec::with_capacity(10 + meta_json.len() + 4 + compressed.len());

    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    put_u32(&mut out, meta_len(meta_json.len())?);
    out.extend_from_slice(&meta_json);
    put_u32(&mut out, body_len(body.len())?);
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// Decodes a translation from the binary format, checking every length and range.
pub fn decode(bytes: &[u8]) -> Result<Translation> {
    let mut cursor = Cursor::new(bytes);
    check_header(&mut cursor)?;
    let meta_len = cursor.u32()? as usize;
    let meta: TranslationMeta = serde_json::from_slice(cursor.take(meta_len)?)?;

    let expected_body = cursor.u32()? as usize;
    let compressed = cursor.rest();
    let body = zstd::stream::decode_all(compressed)?;
    if body.len() != expected_body {
        bail!(
            "body length is {} but the header says {expected_body}",
            body.len()
        );
    }

    decode_body(meta, &body)
}

/// Reads only the metadata header, for listing installed translations cheaply.
pub fn read_meta(bytes: &[u8]) -> Result<TranslationMeta> {
    let mut cursor = Cursor::new(bytes);
    check_header(&mut cursor)?;
    let meta_len = cursor.u32()? as usize;
    let meta = serde_json::from_slice(cursor.take(meta_len)?)?;
    Ok(meta)
}

/// Reads and checks the magic, version, and metadata length.
fn check_header(cursor: &mut Cursor<'_>) -> Result<()> {
    if cursor.take(4)? != MAGIC {
        bail!("not a translation file, the magic is missing");
    }
    let version = cursor.u16()?;
    if version != FORMAT_VERSION {
        bail!("unsupported translation format version {version}");
    }
    Ok(())
}

/// Serialises the body columns in the fixed order.
fn encode_body(translation: &Translation) -> Result<Vec<u8>> {
    let text = translation.text().as_bytes();
    let extra = translation.extra().as_bytes();
    let verses = translation.verses();

    let mut body = Vec::with_capacity(text.len() + extra.len() + verses.len() * 9 + 64);
    put_u32(&mut body, body_len(text.len())?);
    body.extend_from_slice(text);
    put_u32(&mut body, body_len(extra.len())?);
    body.extend_from_slice(extra);

    for count in VERSES_PER_CHAPTER {
        body.extend_from_slice(&count.to_le_bytes());
    }

    put_u32(&mut body, body_len(verses.len())?);
    body.extend(verses.iter().map(|verse| verse.number));
    body.extend(verses.iter().map(|verse| verse.number_end));
    for verse in verses {
        body.extend_from_slice(&verse.text_len.to_le_bytes());
    }
    for verse in verses {
        body.extend_from_slice(&verse.marks_len.to_le_bytes());
    }
    for verse in verses {
        body.extend_from_slice(&verse.spans_len.to_le_bytes());
    }

    put_u32(&mut body, body_len(translation.marks().len())?);
    for mark in translation.marks() {
        body.extend_from_slice(&mark.offset.to_le_bytes());
        body.push(mark.kind.as_u8());
        body.push(mark.level);
        put_u32(&mut body, mark.payload_start);
        put_u32(&mut body, mark.payload_len);
    }

    put_u32(&mut body, body_len(translation.spans().len())?);
    for span in translation.spans() {
        body.extend_from_slice(&span.start.to_le_bytes());
        body.extend_from_slice(&span.end.to_le_bytes());
        body.push(span.style.as_u8());
    }

    Ok(body)
}

/// Rebuilds the model from the body columns, computing text starts as it goes.
fn decode_body(meta: TranslationMeta, body: &[u8]) -> Result<Translation> {
    let mut cursor = Cursor::new(body);

    let text_len = cursor.u32()? as usize;
    let text = std::str::from_utf8(cursor.take(text_len)?)?.to_owned();
    let extra_len = cursor.u32()? as usize;
    let extra = std::str::from_utf8(cursor.take(extra_len)?)?.to_owned();

    let mut counts = [0u16; CHAPTER_COUNT];
    for count in &mut counts {
        *count = cursor.u16()?;
    }
    if counts != VERSES_PER_CHAPTER {
        bail!("verses-per-chapter table does not match the canonical grid");
    }

    let n = cursor.u32()? as usize;
    let numbers = cursor.take(n)?;
    let number_ends = cursor.take(n)?;
    let text_lens = read_u16s(&mut cursor, n)?;
    let marks_lens = read_u16s(&mut cursor, n)?;
    let spans_lens = read_u16s(&mut cursor, n)?;

    let text_total: usize = text_lens.iter().map(|len| *len as usize).sum();
    if text_total != text.len() {
        bail!(
            "verse text lengths total {text_total} but the text is {} bytes",
            text.len()
        );
    }

    let mut verses = Vec::with_capacity(n);
    let mut text_start = 0u32;
    let mut marks_start = 0u32;
    let mut spans_start = 0u32;
    for index in 0..n {
        verses.push(Verse {
            number: numbers[index],
            number_end: number_ends[index],
            text_start,
            text_len: text_lens[index],
            marks_start,
            marks_len: marks_lens[index],
            spans_start,
            spans_len: spans_lens[index],
        });
        text_start += u32::from(text_lens[index]);
        marks_start += u32::from(marks_lens[index]);
        spans_start += u32::from(spans_lens[index]);
    }

    let m = cursor.u32()? as usize;
    let mut marks = Vec::with_capacity(m);
    for _ in 0..m {
        let offset = cursor.u16()?;
        let kind = MarkKind::from_u8(cursor.u8()?)
            .ok_or_else(|| anyhow::anyhow!("unknown mark kind in file"))?;
        let level = cursor.u8()?;
        let payload_start = cursor.u32()?;
        let payload_len = cursor.u32()?;
        marks.push(Mark {
            offset,
            kind,
            level,
            payload_start,
            payload_len,
        });
    }

    if marks_start as usize != m {
        bail!("verse mark counts total {marks_start} but the file holds {m} marks");
    }

    let k = cursor.u32()? as usize;
    let mut spans = Vec::with_capacity(k);
    for _ in 0..k {
        let start = cursor.u16()?;
        let end = cursor.u16()?;
        let style = SpanStyle::from_u8(cursor.u8()?)
            .ok_or_else(|| anyhow::anyhow!("unknown span style in file"))?;
        spans.push(Span { start, end, style });
    }

    if spans_start as usize != k {
        bail!("verse span counts total {spans_start} but the file holds {k} spans");
    }
    if !cursor.is_empty() {
        bail!(
            "{} trailing bytes after the last column",
            cursor.rest().len()
        );
    }

    Translation::from_parts(meta, text, extra, verses, marks, spans)
}

/// Reads a column of little-endian `u16`s.
fn read_u16s(cursor: &mut Cursor<'_>, count: usize) -> Result<Vec<u16>> {
    let bytes = cursor.take(count * 2)?;
    let (pairs, _) = bytes.as_chunks::<2>();
    Ok(pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect())
}

/// Converts a length to the `u32` the format stores.
fn meta_len(len: usize) -> Result<u32> {
    u32::try_from(len).map_err(|_| anyhow::anyhow!("metadata is longer than 4 GiB"))
}

/// Converts a length to the `u32` the format stores.
fn body_len(len: usize) -> Result<u32> {
    u32::try_from(len).map_err(|_| anyhow::anyhow!("body is longer than 4 GiB"))
}

/// Appends a little-endian `u32`.
fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// A bounds-checked reader over the encoded bytes.
struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    /// Starts at the first byte.
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    /// Takes the next `len` bytes, or reports that the file ended early.
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("length overflows"))?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| anyhow::anyhow!("file ended after {} bytes", self.position))?;
        self.position = end;
        Ok(slice)
    }

    /// Reads one byte.
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// Reads a little-endian `u16`.
    fn u16(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// Reads a little-endian `u32`.
    fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// The bytes not read yet.
    fn rest(&self) -> &'a [u8] {
        &self.bytes[self.position..]
    }

    /// Whether every byte has been read.
    fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }
}
