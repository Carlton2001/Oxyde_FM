//! GFM extended autolinks: `www.` and `http(s)://` addresses and email
//! addresses written as plain text. Only their ranges are found; nothing
//! is ever fetched.
use std::ops::Range;

/// Byte ranges of the extended autolinks in `text`, in order.
pub(super) fn find(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut links = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let found = url_at(text, index).or_else(|| email_at(text, index));
        match found {
            Some(end) => {
                links.push(index..end);
                index = end;
            }
            None => index += text[index..].chars().next().map_or(1, char::len_utf8),
        }
    }
    links
}

/// A URL may start at the beginning, after whitespace or after `*`, `_`, `~`, `(`.
fn boundary(text: &str, index: usize) -> bool {
    text[..index]
        .chars()
        .next_back()
        .is_none_or(|c| c.is_whitespace() || matches!(c, '*' | '_' | '~' | '('))
}

fn url_at(text: &str, start: usize) -> Option<usize> {
    if !boundary(text, start) {
        return None;
    }
    let rest = &text[start..];
    let prefix = ["https://", "http://", "www."]
        .into_iter()
        .find(|prefix| rest.as_bytes().get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes())))?;
    let domain_start = if prefix == "www." { start } else { start + prefix.len() };
    let domain_end = domain(text, domain_start)?;
    let end = domain_end + text[domain_end..].find(|c: char| c.is_whitespace() || c == '<').unwrap_or(text.len() - domain_end);
    Some(trim(text, domain_end, end))
}

/// End of a valid domain: alphanumeric, `-` and `_` segments separated by
/// periods, with at least one period and no `_` in its last two segments.
fn domain(text: &str, start: usize) -> Option<usize> {
    let len = text[start..]
        .find(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .unwrap_or(text.len() - start);
    let domain = text[start..start + len].trim_end_matches('.');
    if !domain.contains('.') || domain.contains("..") || domain.rsplit('.').take(2).any(|segment| segment.contains('_')) {
        return None;
    }
    Some(start + domain.len())
}

/// Removes trailing punctuation, unbalanced closing parentheses and a final entity reference.
fn trim(text: &str, minimum: usize, mut end: usize) -> usize {
    loop {
        let link = &text[..end];
        let Some(last) = link.chars().next_back() else { return end };
        if end <= minimum {
            return end;
        }
        if matches!(last, '?' | '!' | '.' | ',' | ':' | '*' | '_' | '~' | '\'' | '"') {
            end -= 1;
        } else if last == ')' && link[minimum..].matches('(').count() < link[minimum..].matches(')').count() {
            end -= 1;
        } else if last == ';' {
            let entity = link[..end - 1].rfind('&').filter(|amp| {
                *amp >= minimum && link[amp + 1..end - 1].chars().all(|c| c.is_ascii_alphanumeric()) && end - 1 > amp + 1
            });
            match entity {
                Some(amp) => end = amp,
                None => return end,
            }
        } else {
            return end;
        }
    }
}

fn email_at(text: &str, start: usize) -> Option<usize> {
    let local = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+');
    // The local part must begin here, not inside a longer word or address.
    if text[..start].chars().next_back().is_some_and(|c| local(c) || c == '@' || c == '/') {
        return None;
    }
    let rest = &text[start..];
    let at = rest.find(|c: char| !local(c))?;
    if at == 0 || !rest[at..].starts_with('@') {
        return None;
    }
    let host_start = start + at + 1;
    let len = text[host_start..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .unwrap_or(text.len() - host_start);
    let host = text[host_start..host_start + len].trim_end_matches('.');
    if !host.contains('.') || host.contains("..") || host.starts_with('.') || host.ends_with(['-', '_']) {
        return None;
    }
    Some(host_start + host.len())
}

#[cfg(test)]
#[path = "tests/autolink.rs"]
mod tests;
