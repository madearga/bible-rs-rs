// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The binary translation format: an uncompressed JSON header over column
//! arrays compressed with zstd.
//!
//! ```text
//! "BIBL" u16 version  u32 meta_len  meta_json  (uncompressed header)
//! u32 body_len  zstd(body)
//!
//! body = u32 text_len text  u32 extra_len extra
//!        u16 x 1189 verses-per-chapter
//!        u32 n  then column arrays, one array each:
//!                number u8 x n
//!                number_end u8 x n
//!                text_len u16 x n
//!                marks_len u16 x n
//!                spans_len u16 x n
//!        u32 m  marks, each: offset u16, kind u8, level u8, payload_start u32, payload_len u32
//!        u32 k  spans, each: start u16, end u16, style u8
//! ```
//!
//! Every integer is little-endian. The header stays uncompressed so listing the
//! installed translations never decompresses anything.

use anyhow::{Context, Result, anyhow, ensure};

use crate::books::{CHAPTER_COUNT, VERSES_PER_CHAPTER};
use crate::model::{Mark, MarkKind, Span, SpanStyle, Translation, TranslationMeta, Verse};

/// The four ASCII bytes every translation file starts with.
pub const MAGIC: [u8; 4] = *b"BIBL";

/// The format revision this crate writes and reads.
pub const VERSION: u16 = 1;

/// The zstd level [`encode`] uses.
///
/// Encoding is a build-time cost and decoding is not, so the default trades a
/// little compression time for a smaller file.
pub const DEFAULT_LEVEL: i32 = 3;

/// Encodes a translation, header and compressed body, into one buffer.
pub fn encode(translation: &Translation) -> Result<Vec<u8>> {
    encode_with_level(translation, DEFAULT_LEVEL)
}

/// Encodes a translation at an explicit zstd level, for a build that prefers a
/// smaller file over a faster encode.
pub fn encode_with_level(translation: &Translation, level: i32) -> Result<Vec<u8>> {
    let meta =
        serde_json::to_vec(translation.meta()).context("serialising translation metadata")?;
    let meta_len = u32::try_from(meta.len()).context("translation metadata is too large")?;
    let body = build_body(translation)?;
    let body_len = u32::try_from(body.len()).context("translation body is too large")?;
    let compressed =
        zstd::bulk::compress(&body, level).context("compressing the translation body")?;

    let mut out = Vec::with_capacity(4 + 2 + 4 + meta.len() + 4 + compressed.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&meta_len.to_le_bytes());
    out.extend_from_slice(&meta);
    out.extend_from_slice(&body_len.to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// Decodes a translation, verse text and all, from a whole file.
pub fn decode(bytes: &[u8]) -> Result<Translation> {
    let mut reader = Reader::new(bytes);
    let meta = read_header(&mut reader)?;
    let body_len = reader.u32()? as usize;
    let body = zstd::bulk::decompress(&bytes[reader.position..], body_len)
        .context("decompressing the translation body")?;
    ensure!(
        body.len() == body_len,
        "translation body is {} bytes, the header says {body_len}",
        body.len()
    );
    decode_body(meta, &body)
}

/// Reads only the metadata header, leaving the compressed body untouched.
pub fn read_meta(bytes: &[u8]) -> Result<TranslationMeta> {
    let mut reader = Reader::new(bytes);
    read_header(&mut reader)
}

fn read_header(reader: &mut Reader<'_>) -> Result<TranslationMeta> {
    ensure!(
        reader.take(4)? == MAGIC,
        "not a translation file: it does not start with BIBL"
    );
    let version = reader.u16()?;
    ensure!(
        version == VERSION,
        "translation format version {version} is not supported by this build, which reads version {VERSION}"
    );
    let meta_len = reader.u32()? as usize;
    let meta = serde_json::from_slice(reader.take(meta_len)?)
        .context("parsing the translation metadata header")?;
    Ok(meta)
}

fn build_body(translation: &Translation) -> Result<Vec<u8>> {
    let text = translation.text().as_bytes();
    let extra = translation.extra().as_bytes();
    let verses = translation.verses();
    let marks = translation.marks();
    let spans = translation.spans();

    let mark_total: u32 = verses.iter().map(|verse| u32::from(verse.marks_len)).sum();
    let span_total: u32 = verses.iter().map(|verse| u32::from(verse.spans_len)).sum();
    ensure!(
        mark_total as usize == marks.len(),
        "verses point at {mark_total} marks, the mark table holds {}",
        marks.len()
    );
    ensure!(
        span_total as usize == spans.len(),
        "verses point at {span_total} spans, the span table holds {}",
        spans.len()
    );

    let mut body = Vec::with_capacity(
        text.len()
            + extra.len()
            + 8
            + CHAPTER_COUNT * 2
            + 5
            + verses.len() * 8
            + marks.len() * 12
            + spans.len() * 5,
    );
    body.extend_from_slice(&u32::try_from(text.len())?.to_le_bytes());
    body.extend_from_slice(text);
    body.extend_from_slice(&u32::try_from(extra.len())?.to_le_bytes());
    body.extend_from_slice(extra);
    for count in &VERSES_PER_CHAPTER {
        body.extend_from_slice(&count.to_le_bytes());
    }

    body.extend_from_slice(&u32::try_from(verses.len())?.to_le_bytes());
    for verse in verses {
        body.push(verse.number);
    }
    for verse in verses {
        body.push(verse.number_end);
    }
    for verse in verses {
        body.extend_from_slice(&verse.text_len.to_le_bytes());
    }
    for verse in verses {
        body.extend_from_slice(&verse.marks_len.to_le_bytes());
    }
    for verse in verses {
        body.extend_from_slice(&verse.spans_len.to_le_bytes());
    }

    body.extend_from_slice(&u32::try_from(marks.len())?.to_le_bytes());
    for mark in marks {
        body.extend_from_slice(&mark.offset.to_le_bytes());
        body.push(mark.kind.code());
        body.push(mark.level);
        body.extend_from_slice(&mark.payload_start.to_le_bytes());
        body.extend_from_slice(&mark.payload_len.to_le_bytes());
    }

    body.extend_from_slice(&u32::try_from(spans.len())?.to_le_bytes());
    for span in spans {
        body.extend_from_slice(&span.start.to_le_bytes());
        body.extend_from_slice(&span.end.to_le_bytes());
        body.push(span.style.code());
    }
    Ok(body)
}

fn decode_body(meta: TranslationMeta, body: &[u8]) -> Result<Translation> {
    let mut reader = Reader::new(body);

    let text_len = reader.u32()? as usize;
    let text = std::str::from_utf8(reader.take(text_len)?)
        .context("verse text is not valid UTF-8")?
        .to_owned();
    let extra_len = reader.u32()? as usize;
    let extra = std::str::from_utf8(reader.take(extra_len)?)
        .context("heading text is not valid UTF-8")?
        .to_owned();

    let mut grid = [0u16; CHAPTER_COUNT];
    for slot in &mut grid {
        *slot = reader.u16()?;
    }
    ensure!(
        grid == VERSES_PER_CHAPTER,
        "translation was built against a different verse grid than the canon"
    );

    let verse_count = reader.u32()? as usize;
    let numbers = reader.take(verse_count)?;
    let number_ends = reader.take(verse_count)?;
    let text_lens = reader.take(verse_count * 2)?;
    let marks_lens = reader.take(verse_count * 2)?;
    let spans_lens = reader.take(verse_count * 2)?;

    let mut verses = Vec::with_capacity(verse_count);
    let mut text_start = 0u32;
    let mut marks_start = 0u32;
    let mut spans_start = 0u32;
    for index in 0..verse_count {
        let verse = Verse {
            number: numbers[index],
            number_end: number_ends[index],
            text_start,
            text_len: column_u16(text_lens, index),
            marks_start,
            marks_len: column_u16(marks_lens, index),
            spans_start,
            spans_len: column_u16(spans_lens, index),
        };
        text_start += u32::from(verse.text_len);
        marks_start += u32::from(verse.marks_len);
        spans_start += u32::from(verse.spans_len);
        verses.push(verse);
    }
    ensure!(
        text_start as usize == text.len(),
        "verse text ranges cover {text_start} bytes, the text holds {}",
        text.len()
    );

    let mark_count = reader.u32()? as usize;
    ensure!(
        mark_count as u32 == marks_start,
        "verses point at {marks_start} marks, the file holds {mark_count}"
    );
    let mut marks = Vec::with_capacity(mark_count);
    for _ in 0..mark_count {
        let offset = reader.u16()?;
        let kind = MarkKind::from_code(reader.u8()?)
            .ok_or_else(|| anyhow!("translation uses a mark kind this build does not know"))?;
        let level = reader.u8()?;
        let payload_start = reader.u32()?;
        let payload_len = reader.u32()?;
        ensure!(
            payload_start as usize + payload_len as usize <= extra.len(),
            "a mark payload points past the heading text"
        );
        marks.push(Mark {
            offset,
            kind,
            level,
            payload_start,
            payload_len,
        });
    }

    let span_count = reader.u32()? as usize;
    ensure!(
        span_count as u32 == spans_start,
        "verses point at {spans_start} spans, the file holds {span_count}"
    );
    let mut spans = Vec::with_capacity(span_count);
    for _ in 0..span_count {
        spans.push(Span {
            start: reader.u16()?,
            end: reader.u16()?,
            style: SpanStyle::from_code(reader.u8()?)
                .ok_or_else(|| anyhow!("translation uses a span style this build does not know"))?,
        });
    }
    ensure!(
        reader.position == body.len(),
        "translation body has {} bytes this build does not read",
        body.len() - reader.position
    );

    for verse in &verses {
        for mark in marks_of(&marks, verse) {
            ensure!(
                mark.offset <= verse.text_len,
                "a mark sits past the end of a {} byte verse",
                verse.text_len
            );
        }
        for span in spans_of(&spans, verse) {
            ensure!(
                span.start <= span.end && span.end <= verse.text_len,
                "a span runs outside its {} byte verse",
                verse.text_len
            );
        }
    }

    Ok(Translation::from_parts(
        meta, text, extra, verses, marks, spans,
    ))
}

fn marks_of<'a>(marks: &'a [Mark], verse: &Verse) -> &'a [Mark] {
    let start = verse.marks_start as usize;
    &marks[start..start + usize::from(verse.marks_len)]
}

fn spans_of<'a>(spans: &'a [Span], verse: &Verse) -> &'a [Span] {
    let start = verse.spans_start as usize;
    &spans[start..start + usize::from(verse.spans_len)]
}

fn column_u16(column: &[u8], index: usize) -> u16 {
    u16::from_le_bytes([column[index * 2], column[index * 2 + 1]])
}

/// A bounds-checked cursor over a translation file or its decompressed body.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| {
                anyhow!(
                    "translation data ends at byte {} while {len} more bytes were expected",
                    self.position
                )
            })?;
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}
