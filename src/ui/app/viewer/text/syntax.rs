//! Bounded syntax analysis, performed in the background, never during render.
use std::{
    ops::Range,
    sync::LazyLock,
    time::{Duration, Instant},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Color, StyleModifier, Theme, ThemeItem},
    parsing::{SyntaxDefinition, SyntaxSet},
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 16 * 1024;
const MAX_SPANS: usize = 1_000_000;
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(load_syntaxes);

fn load_syntaxes() -> SyntaxSet {
    let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
    // Keep upstream names: SCSS links to named contexts in Sass.
    for (name, source) in [
        ("Sass", include_str!("../../../../../vendor/syntaxes/Sass.sublime-syntax")),
        ("SCSS", include_str!("../../../../../vendor/syntaxes/SCSS.sublime-syntax")),
        ("PowerShell", include_str!("../../../../../vendor/syntaxes/PowerShell.sublime-syntax")),
    ] {
        match SyntaxDefinition::load_from_str(source, true, Some(name)) {
            Ok(syntax) => builder.add(syntax),
            Err(error) => log::error!("Cannot load embedded syntax {name}: {error}"),
        }
    }
    builder.build()
}

#[derive(Clone)]
pub(super) struct Span {
    pub(super) range: Range<usize>,
    pub(super) kind: u8,
}

/// Pays the grammar compilation before the first file needs it.
pub(super) fn warm() {
    LazyLock::force(&SYNTAXES);
}

/// Whole-file analysis.
#[cfg(test)]
pub(super) fn highlight(path: &str, text: &str) -> Vec<Span> {
    highlight_within(path, text, None)
}

/// Analysis stopping at the first whole line past `budget`, for callers that
/// wait on the result.
pub(super) fn highlight_within(path: &str, text: &str, budget: Option<Duration>) -> Vec<Span> {
    let mut spans = Vec::new();
    analyse(path, text, budget, |chunk| {
        spans.extend(chunk);
        true
    });
    spans
}

/// Whole-file analysis published in chunks, the start of the file first, so the
/// visible rows are coloured without waiting for the rest. `publish` returns
/// false once nobody listens, which stops the analysis.
pub(super) fn highlight_progressively(path: &str, text: &str, publish: impl FnMut(Vec<Span>) -> bool) {
    analyse(path, text, None, publish);
}

fn analyse(path: &str, text: &str, budget: Option<Duration>, mut publish: impl FnMut(Vec<Span>) -> bool) {
    // A first chunk small enough to cover the opening screen almost at once.
    const FIRST_CHUNK: Duration = Duration::from_millis(10);
    const CHUNK: Duration = Duration::from_millis(80);
    if text.len() > MAX_BYTES {
        return;
    }
    let syntaxes = &*SYNTAXES;
    let name = std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    let syntax = syntaxes
        .find_syntax_by_extension(name)
        // XAML uses XML lexical syntax; markup extensions remain plain strings.
        .or_else(|| extension.eq_ignore_ascii_case("xaml").then(|| syntaxes.find_syntax_by_extension("xml")).flatten())
        .or_else(|| syntaxes.find_syntax_by_extension(extension))
        .or_else(|| syntaxes.find_syntax_by_token(extension))
        .or_else(|| {
            text.lines()
                .next()
                .filter(|line| line.len() <= MAX_LINE_BYTES)
                .and_then(|line| syntaxes.find_syntax_by_first_line(line))
        });
    let Some(syntax) = syntax else {
        return;
    };
    // RGB values encode categories only; render resolves the current Oxyde
    // theme on every frame, so switching themes never requires another parse.
    let mut theme = Theme::default();
    for (scope, kind) in [
        ("comment", 1),
        ("string", 2),
        ("constant", 3),
        ("keyword, storage", 4),
        ("entity.name, support", 5),
        ("variable", 7),
        ("invalid", 6),
    ] {
        theme.scopes.push(ThemeItem {
            scope: scope.parse().expect("static syntax selector"),
            style: StyleModifier {
                foreground: Some(Color {
                    r: kind,
                    g: 0,
                    b: 0,
                    a: 255,
                }),
                ..Default::default()
            },
        });
    }
    let mut highlighter = HighlightLines::new(syntax, &theme);
    let started = Instant::now();
    let mut published = started;
    let mut interval = FIRST_CHUNK;
    let mut count = 0;
    let mut spans = Vec::new();
    let mut offset = 0;
    for line in syntect::util::LinesWithEndings::from(text) {
        // Stop at a whole logical line: a partial parse cannot preserve context.
        if line.len() > MAX_LINE_BYTES || budget.is_some_and(|budget| started.elapsed() > budget) {
            break;
        }
        let Ok(parts) = highlighter.highlight_line(line, syntaxes) else {
            break;
        };
        for (style, part) in parts {
            let end = offset + part.len();
            if style.foreground.g == 0
                && style.foreground.b == 0
                && (1..=7).contains(&style.foreground.r)
            {
                if count == MAX_SPANS {
                    publish(spans);
                    return;
                }
                count += 1;
                spans.push(Span {
                    range: offset..end,
                    kind: style.foreground.r,
                });
            }
            offset = end;
        }
        if published.elapsed() > interval {
            if !publish(std::mem::take(&mut spans)) {
                return;
            }
            published = Instant::now();
            interval = CHUNK;
        }
    }
    publish(spans);
}

#[cfg(test)]
#[path = "../tests/syntax.rs"]
mod tests;
