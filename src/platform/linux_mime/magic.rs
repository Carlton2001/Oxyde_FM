//! Content rules of the shared MIME database (`mime/magic`), as xdgmime reads them
//! for GIO: sections by decreasing priority, each a tree of byte comparisons.

pub(super) struct Section {
    pub(super) priority: u32,
    pub(super) mime: String,
    rules: Vec<Rule>,
}

struct Rule {
    indent: usize,
    offset: usize,
    value: Vec<u8>,
    mask: Option<Vec<u8>>,
    /// Bytes searched from `offset`, at least one.
    range: usize,
}

/// Sections of a `magic` file, or nothing if it is not one.
pub(super) fn parse(bytes: &[u8]) -> Vec<Section> {
    let Some(mut rest) = bytes.strip_prefix(b"MIME-Magic\0\n") else { return Vec::new() };
    let mut sections: Vec<Section> = Vec::new();
    while !rest.is_empty() {
        if let Some(header) = rest.strip_prefix(b"[") {
            let Some(end) = header.iter().position(|byte| *byte == b'\n') else { break };
            let line = String::from_utf8_lossy(&header[..end]);
            rest = &header[end + 1..];
            let Some((priority, mime)) = line.trim_end_matches(']').split_once(':') else { continue };
            sections.push(Section { priority: priority.parse().unwrap_or(50), mime: mime.to_owned(), rules: Vec::new() });
            continue;
        }
        match rule(rest) {
            Some((parsed, next)) => {
                if let (Some(rule), Some(section)) = (parsed, sections.last_mut()) {
                    section.rules.push(rule);
                }
                rest = next;
            }
            None => break,
        }
    }
    sections
}

fn number(bytes: &[u8]) -> (usize, &[u8]) {
    let digits = bytes.iter().take_while(|byte| byte.is_ascii_digit()).count();
    let value = std::str::from_utf8(&bytes[..digits]).ok().and_then(|text| text.parse().ok()).unwrap_or(0);
    (value, &bytes[digits..])
}

/// `[indent]>offset=<u16 length><value>[&mask][~word][+range]\n`. A rule with an
/// unknown extension is skipped up to its line end, as xdgmime does.
fn rule(bytes: &[u8]) -> Option<(Option<Rule>, &[u8])> {
    let (indent, rest) = number(bytes);
    let rest = rest.strip_prefix(b">")?;
    let (offset, rest) = number(rest);
    let rest = rest.strip_prefix(b"=")?;
    let length = u16::from_be_bytes([*rest.first()?, *rest.get(1)?]) as usize;
    let rest = &rest[2..];
    let value = rest.get(..length)?.to_vec();
    let mut rest = &rest[length..];
    let mut mask = None;
    if let Some(after) = rest.strip_prefix(b"&") {
        mask = Some(after.get(..length)?.to_vec());
        rest = &after[length..];
    }
    let mut word = 1;
    if let Some(after) = rest.strip_prefix(b"~") {
        (word, rest) = number(after);
    }
    let mut range = 1;
    if let Some(after) = rest.strip_prefix(b"+") {
        (range, rest) = number(after);
    }
    let end = rest.iter().position(|byte| *byte == b'\n')?;
    if end != 0 {
        return Some((None, &rest[end + 1..]));
    }
    let (value, mask) = swapped(value, mask, word);
    Some((Some(Rule { indent, offset, value, mask, range: range.max(1) }), &rest[1..]))
}

/// Values of host-order words are stored big-endian: swap them on this host.
fn swapped(mut value: Vec<u8>, mut mask: Option<Vec<u8>>, word: usize) -> (Vec<u8>, Option<Vec<u8>>) {
    if cfg!(target_endian = "little") && matches!(word, 2 | 4) && value.len() % word == 0 {
        value.chunks_mut(word).for_each(<[u8]>::reverse);
        if let Some(mask) = &mut mask {
            mask.chunks_mut(word).for_each(<[u8]>::reverse);
        }
    }
    (value, mask)
}

impl Rule {
    fn matches(&self, data: &[u8]) -> bool {
        (self.offset..self.offset + self.range).any(|start| {
            let Some(window) = data.get(start..start + self.value.len()) else { return false };
            match &self.mask {
                None => window == self.value.as_slice(),
                Some(mask) => window
                    .iter()
                    .zip(&self.value)
                    .zip(mask)
                    .all(|((byte, value), mask)| byte & mask == value & mask),
            }
        })
    }

    fn extent(&self) -> usize {
        self.offset + self.range - 1 + self.value.len()
    }
}

/// Rule `index` and, if it has children, one of them.
fn tree_matches(rules: &[Rule], index: usize, data: &[u8]) -> bool {
    if !rules[index].matches(data) {
        return false;
    }
    let indent = rules[index].indent;
    let mut children = rules[index + 1..]
        .iter()
        .enumerate()
        .take_while(|(_, rule)| rule.indent > indent)
        .filter(|(_, rule)| rule.indent == indent + 1)
        .map(|(offset, _)| index + 1 + offset)
        .peekable();
    children.peek().is_none() || children.any(|child| tree_matches(rules, child, data))
}

impl Section {
    pub(super) fn matches(&self, data: &[u8]) -> bool {
        self.rules
            .iter()
            .enumerate()
            .any(|(index, rule)| rule.indent == 0 && tree_matches(&self.rules, index, data))
    }
}

/// How many leading bytes the rules may read.
pub(super) fn extent(sections: &[Section]) -> usize {
    sections.iter().flat_map(|section| &section.rules).map(Rule::extent).max().unwrap_or(0)
}

/// The first type that matches, by decreasing priority, with that priority.
pub(super) fn lookup<'a>(sections: &'a [Section], data: &[u8]) -> Option<(&'a str, u32)> {
    sections
        .iter()
        .find(|section| section.matches(data))
        .map(|section| (section.mime.as_str(), section.priority))
}
