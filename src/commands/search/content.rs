//! Bounded line search. Small lines retain the optimized regex engine; large
//! lines carry the same compiled Thompson states across UTF-8 input blocks.
//! A byte that is not part of a UTF-8 character reads as Windows-1252, so legacy
//! text stays searchable past its first accented letter.
use super::ContentReader;
use crate::utils::legacy_text::cp1252;
use regex::{Regex, RegexBuilder};
use regex_automata::{
    nfa::thompson::{WhichCaptures, NFA},
    util::syntax,
};
use std::{
    io::{self, BufRead, BufReader, Read},
    sync::atomic::{AtomicBool, Ordering},
};

mod normalize;
mod vm;

const LINE_BUFFER_LIMIT: usize = 64 * 1024;

pub(super) struct ContentPattern {
    regex: Regex,
    nfa: NFA,
}

impl ContentPattern {
    pub(super) fn new(pattern: &str, case_sensitive: bool) -> Result<Self, String> {
        let regex = RegexBuilder::new(pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|error| error.to_string())?;
        let nfa = NFA::compiler()
            .configure(
                NFA::config()
                    .which_captures(WhichCaptures::None)
                    .nfa_size_limit(Some(10 * 1024 * 1024)),
            )
            .syntax(syntax::Config::new().case_insensitive(!case_sensitive))
            .build(pattern)
            .map_err(|error| error.to_string())?;
        Ok(Self { regex, nfa })
    }

    pub(super) fn is_match(&self, text: &str) -> bool {
        self.regex.is_match(text)
    }
}

pub(super) fn text_reader_contains_content(
    reader: impl Read,
    pattern: &ContentPattern,
    ignore_accents: bool,
    cancellation: &AtomicBool,
) -> bool {
    match contains(
        reader,
        pattern,
        ignore_accents,
        cancellation,
        LINE_BUFFER_LIMIT,
    ) {
        Ok(matched) => matched && !cancellation.load(Ordering::Relaxed),
        Err(error) => {
            if !cancellation.load(Ordering::Relaxed) {
                log::debug!("Content search read failed: {error}");
            }
            false
        }
    }
}

fn contains(
    reader: impl Read,
    pattern: &ContentPattern,
    ignore_accents: bool,
    cancel: &AtomicBool,
    limit: usize,
) -> io::Result<bool> {
    let mut reader = BufReader::new(ContentReader {
        inner: reader,
        cancellation: cancel,
    });
    let mut line = Line::new(pattern, ignore_accents, limit)?;
    loop {
        let bytes = match reader.fill_buf() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("Search cancelled"));
        }
        if bytes.is_empty() {
            return if line.started() {
                line.finish(false, cancel)
            } else {
                Ok(false)
            };
        }
        let end = bytes.iter().position(|&byte| byte == b'\n');
        let count = end.unwrap_or(bytes.len());
        line.push(&bytes[..count], cancel)?;
        reader.consume(count + usize::from(end.is_some()));
        if end.is_some() && line.finish(true, cancel)? {
            return Ok(true);
        }
    }
}

struct Line<'a> {
    pattern: &'a ContentPattern,
    ignore_accents: bool,
    limit: usize,
    short: Vec<u8>,
    long: Option<LongLine<'a>>,
}

impl<'a> Line<'a> {
    fn new(pattern: &'a ContentPattern, ignore_accents: bool, limit: usize) -> io::Result<Self> {
        let mut short = Vec::new();
        short.try_reserve_exact(limit).map_err(io::Error::other)?;
        Ok(Self {
            pattern,
            ignore_accents,
            limit,
            short,
            long: None,
        })
    }

    fn started(&self) -> bool {
        !self.short.is_empty() || self.long.is_some()
    }

    fn push(&mut self, bytes: &[u8], cancel: &AtomicBool) -> io::Result<()> {
        if self.long.is_none() && bytes.len() <= self.limit - self.short.len() {
            self.short.extend_from_slice(bytes);
            return Ok(());
        }
        if self.long.is_none() {
            let mut long = LongLine::new(&self.pattern.nfa, self.ignore_accents)?;
            long.push(&self.short, cancel)?;
            self.short.clear();
            self.long = Some(long);
        }
        self.long.as_mut().unwrap().push(bytes, cancel)
    }

    fn finish(&mut self, newline: bool, cancel: &AtomicBool) -> io::Result<bool> {
        if let Some(mut long) = self.long.take() {
            return long.finish(newline, cancel);
        }
        let bytes = if newline && self.short.last() == Some(&b'\r') {
            &self.short[..self.short.len() - 1]
        } else {
            &self.short
        };
        let text = decode_line(bytes);
        let matched = if self.ignore_accents {
            self.pattern.is_match(&crate::utils::remove_accents(&text))
        } else {
            self.pattern.is_match(&text)
        };
        self.short.clear();
        Ok(matched)
    }
}

/// The text of a line: UTF-8 characters, and each byte outside one as Windows-1252.
fn decode_line(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.into(),
        Err(_) => bytes
            .utf8_chunks()
            .flat_map(|chunk| chunk.valid().chars().chain(chunk.invalid().iter().map(|&byte| cp1252(byte))))
            .collect::<String>()
            .into(),
    }
}

struct LongLine<'a> {
    vm: vm::Vm<'a>,
    normalizer: Option<normalize::Normalizer>,
    utf8: [u8; 4],
    utf8_len: usize,
    held: Option<char>,
}

impl<'a> LongLine<'a> {
    fn new(nfa: &'a NFA, ignore_accents: bool) -> io::Result<Self> {
        Ok(Self {
            vm: vm::Vm::new(nfa)?,
            normalizer: ignore_accents
                .then(normalize::Normalizer::new)
                .transpose()?,
            utf8: [0; 4],
            utf8_len: 0,
            held: None,
        })
    }

    fn push(&mut self, mut bytes: &[u8], cancel: &AtomicBool) -> io::Result<()> {
        // Complete the character cut at the end of the previous block.
        while self.utf8_len > 0 && !bytes.is_empty() {
            self.utf8[self.utf8_len] = bytes[0];
            self.utf8_len += 1;
            bytes = &bytes[1..];
            match std::str::from_utf8(&self.utf8[..self.utf8_len]) {
                Ok(text) => {
                    let c = text.chars().next().unwrap();
                    self.utf8_len = 0;
                    self.character(c, cancel)?;
                }
                Err(error) if error.error_len().is_some() => {
                    let (pending, length) = (self.utf8, self.utf8_len);
                    self.utf8_len = 0;
                    self.decode(&pending[..length], cancel)?;
                }
                Err(_) => {}
            }
        }
        self.decode(bytes, cancel)
    }

    /// Feeds UTF-8 characters, and each byte outside one as Windows-1252; keeps
    /// a character cut at the end of `bytes` for the next block.
    fn decode(&mut self, mut bytes: &[u8], cancel: &AtomicBool) -> io::Result<()> {
        loop {
            let error = match std::str::from_utf8(bytes) {
                Ok(text) => {
                    for c in text.chars() {
                        self.character(c, cancel)?;
                    }
                    return Ok(());
                }
                Err(error) => error,
            };
            let (valid, rest) = bytes.split_at(error.valid_up_to());
            for c in std::str::from_utf8(valid).unwrap().chars() {
                self.character(c, cancel)?;
            }
            let Some(invalid) = error.error_len() else {
                self.utf8[..rest.len()].copy_from_slice(rest);
                self.utf8_len = rest.len();
                return Ok(());
            };
            for &byte in &rest[..invalid] {
                self.character(cp1252(byte), cancel)?;
            }
            bytes = &rest[invalid..];
        }
    }

    fn character(&mut self, c: char, cancel: &AtomicBool) -> io::Result<()> {
        // Delay the final scalar so CR is stripped only immediately before LF,
        // matching BufRead::lines (a bare CR at EOF belongs to the line).
        if let Some(previous) = self.held.replace(c) {
            self.emit(previous, cancel)?;
        }
        Ok(())
    }

    fn emit(&mut self, c: char, cancel: &AtomicBool) -> io::Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("Search cancelled"));
        }
        if self.vm.matched {
            return Ok(());
        }
        if let Some(normalizer) = &mut self.normalizer {
            normalizer.push(c, &mut self.vm, cancel)
        } else {
            self.vm.push(c, cancel)
        }
    }

    fn finish(&mut self, newline: bool, cancel: &AtomicBool) -> io::Result<bool> {
        let (pending, length) = (self.utf8, self.utf8_len);
        self.utf8_len = 0;
        for &byte in &pending[..length] {
            self.character(cp1252(byte), cancel)?;
        }
        if let Some(c) = self.held.take().filter(|&c| !newline || c != '\r') {
            self.emit(c, cancel)?;
        }
        if !self.vm.matched {
            if let Some(normalizer) = &mut self.normalizer {
                normalizer.finish(&mut self.vm, cancel)?;
            }
        }
        self.vm.finish(cancel)
    }
}

#[cfg(test)]
#[path = "content/tests/streaming.rs"]
mod tests;
