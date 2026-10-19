use super::*;

#[test]
fn rust_multiline_comments_preserve_context_and_utf8_boundaries() {
    let text = "/* début\nfin */\nlet valeur = \"été\";\n";
    let spans = highlight("example.rs", text);
    let second_line = text.find("fin").unwrap();
    assert!(spans
        .iter()
        .any(|span| span.kind == 1 && span.range.contains(&second_line)));
    let string = text.find("été").unwrap();
    assert!(spans
        .iter()
        .any(|span| span.kind == 2 && span.range.contains(&string)));
    for span in &spans {
        assert!(text.is_char_boundary(span.range.start));
        assert!(text.is_char_boundary(span.range.end));
    }
    assert!(spans
        .windows(2)
        .all(|pair| pair[0].range.end <= pair[1].range.start));
}

#[test]
fn unknown_and_oversized_files_keep_plain_text() {
    assert!(highlight("file.oxydeunknown", "ordinary words").is_empty());
    assert!(highlight("file.rs", &"x".repeat(MAX_BYTES + 1)).is_empty());
    assert!(highlight("file.rs", &"x".repeat(MAX_LINE_BYTES + 1)).is_empty());
}

#[test]
fn plain_text_is_not_guessed_from_code_but_unknown_files_can_use_a_shebang() {
    let rust = "fn main() { let x = \"hello\"; }\n";
    assert!(highlight("example.txt", rust).is_empty());
    assert!(highlight("example.unknown", rust).is_empty());
    assert!(!highlight("example.rs", rust).is_empty());
    assert!(!highlight("script", "#!/usr/bin/env python\nprint(\"hello\")\n").is_empty());
}

#[test]
fn xaml_uses_xml_syntax_for_tags_attributes_and_quoted_values() {
    let source = "<Window xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\">\n<TextBlock Text=\"Hello\" />\n</Window>\n";
    let xml = highlight("view.xml", source);
    let xaml = highlight("view.XAML", source);
    assert!(!xaml.is_empty());
    assert_eq!(xaml.iter().map(|span| (&span.range, span.kind)).collect::<Vec<_>>(),
        xml.iter().map(|span| (&span.range, span.kind)).collect::<Vec<_>>());
}

#[test]
fn bundled_grammars_load_and_resolve_their_dependencies() {
    let base = SyntaxSet::load_defaults_newlines().find_unlinked_contexts();
    let added = SYNTAXES.find_unlinked_contexts();
    assert!(added.difference(&base).next().is_none(), "new unlinked contexts: {:?}", added.difference(&base).collect::<Vec<_>>());
    for (extension, name) in [("scss", "SCSS"), ("sass", "Sass"), ("ps1", "PowerShell"), ("psm1", "PowerShell"), ("psd1", "PowerShell")] {
        assert_eq!(SYNTAXES.find_syntax_by_extension(extension).unwrap().name, name);
    }
}

fn assert_kind_at(path: &str, source: &str, token: &str, kind: u8) {
    let at = source.find(token).unwrap();
    let spans = highlight(path, source);
    assert!(spans.iter().any(|span| span.kind == kind && span.range.contains(&at)), "{path}: missing kind {kind} at {token}");
}

#[test]
fn powershell_comments_variables_keywords_and_interpolated_strings_are_colored() {
    let source = "# comment\n$name = \"World\"\nif ($true) { Write-Host \"Hello $name\" }\n";
    for extension in ["ps1", "psm1", "psd1"] {
        let path = format!("example.{extension}");
        for (token, kind) in [("comment", 1), ("$name", 7), ("if", 4), ("World", 2)] {
            assert_kind_at(&path, source, token, kind);
        }
    }
}

#[test]
fn scss_variables_mixins_and_nested_rules_use_the_sass_contexts() {
    let source = "// comment\n$color: #ff0000;\n@mixin theme($tone) {\n  .nested { color: $tone; content: \"hello\"; }\n}\n";
    for (token, kind) in [("comment", 1), ("$color", 7), ("@mixin", 4), ("hello", 2)] {
        assert_kind_at("example.scss", source, token, kind);
    }
}

#[test]
fn powershell_unicode_escapes_keep_their_semantics_with_the_rust_regex_backend() {
    for escape in ["`u{41}", "`u{1F600}", "`u{10FFFF}", "`u{0FFFF}"] {
        let source = format!("Write-Host \"{escape}\"\n");
        assert_kind_at("escape.ps1", &source, escape, 3);
    }
}
