//! UTF-16 text files (.reg exports, PowerShell output, some CSV) read as UTF-8 for the
//! content search, which matches lines of UTF-8 text.
use std::io::{self, Read};

/// The byte order a UTF-16 byte order mark announces, if `start` begins with one.
pub(super) fn byte_order(start: &[u8]) -> Option<bool> {
    match start {
        [0xFF, 0xFE, ..] => Some(false),
        [0xFE, 0xFF, ..] => Some(true),
        _ => None,
    }
}

/// UTF-8 from UTF-16 after its byte order mark; an unpaired surrogate becomes U+FFFD.
pub(super) struct Utf16Reader<R> {
    inner: R,
    big_endian: bool,
    started: bool,
    /// A lone byte, or a high surrogate, waiting for the rest of its character.
    odd_byte: Option<u8>,
    high: Option<u16>,
    output: Vec<u8>,
    position: usize,
}

impl<R: Read> Utf16Reader<R> {
    pub(super) fn new(inner: R, big_endian: bool) -> Self {
        Self { inner, big_endian, started: false, odd_byte: None, high: None, output: Vec::new(), position: 0 }
    }

    fn push(&mut self, character: char) {
        let mut bytes = [0; 4];
        self.output.extend_from_slice(character.encode_utf8(&mut bytes).as_bytes());
    }

    fn unit(&mut self, unit: u16) {
        match (self.high.take(), unit) {
            (Some(high), 0xDC00..=0xDFFF) => {
                let value = 0x10000 + ((u32::from(high) - 0xD800) << 10) + (u32::from(unit) - 0xDC00);
                self.push(char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER));
            }
            (high, _) => {
                if high.is_some() {
                    self.push(char::REPLACEMENT_CHARACTER);
                }
                match unit {
                    0xD800..=0xDBFF => self.high = Some(unit),
                    0xDC00..=0xDFFF => self.push(char::REPLACEMENT_CHARACTER),
                    _ => self.push(char::from_u32(u32::from(unit)).unwrap_or(char::REPLACEMENT_CHARACTER)),
                }
            }
        }
    }

    /// Decodes the next block of the file; false at its end.
    fn fill(&mut self) -> io::Result<bool> {
        self.output.clear();
        self.position = 0;
        let mut raw = [0u8; 16 * 1024];
        let read = self.inner.read(&mut raw)?;
        if read == 0 {
            if self.high.take().is_some() || self.odd_byte.take().is_some() {
                self.push(char::REPLACEMENT_CHARACTER);
            }
            return Ok(!self.output.is_empty());
        }
        let mut bytes = &raw[..read];
        if !self.started {
            // The mark itself, read in the first block of any real file, is not text.
            self.started = true;
            if bytes.len() >= 2 && byte_order(bytes).is_some() {
                bytes = &bytes[2..];
            }
        }
        let joined;
        if let Some(first) = self.odd_byte.take() {
            joined = [&[first][..], bytes].concat();
            bytes = &joined;
        }
        let (pairs, rest) = bytes.as_chunks::<2>();
        if let [last] = rest {
            self.odd_byte = Some(*last);
        }
        for &pair in pairs {
            let unit = if self.big_endian { u16::from_be_bytes(pair) } else { u16::from_le_bytes(pair) };
            self.unit(unit);
        }
        Ok(true)
    }
}

impl<R: Read> Read for Utf16Reader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        while self.position == self.output.len() {
            if !self.fill()? {
                return Ok(0);
            }
        }
        let count = buffer.len().min(self.output.len() - self.position);
        buffer[..count].copy_from_slice(&self.output[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}
