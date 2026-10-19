use super::*;
use crate::ui::{components::icons::{get_icon_type, get_preview_type, IconType, PreviewType}, date_groups::FileFamily};
use oxyde_lib::models::FileEntry;

#[test]
fn languages_and_variants_share_code_group_icon_and_text_preview() {
    for name in ["main.hs", "main.lhs", "build.gradle", "main.d", "main.pas", "main.tcl",
        "main.as", "main.applescript", "main.mm", "main.cc", "main.cxx", "main.hh",
        "main.pyi", "main.pyx", "main.pm", "build.sbt", "main.scala", "main.erl", "main.lisp",
        "View.XAML", "schema.XML", "Gemfile", "GNUmakefile", "Cargo.lock", "page.html.erb"] {
        let file = FileEntry { name: name.into(), ..Default::default() };
        assert!(is_code_file(name), "{name}");
        assert_eq!(FileFamily::for_file(&file), FileFamily::Code, "{name}");
        assert!(matches!(get_icon_type(name, false, None, false, false), IconType::FileCode), "{name}");
        assert_eq!(get_preview_type(name), PreviewType::Text, "{name}");
    }
}

#[test]
fn syntect_inventory_is_covered_without_changing_plain_or_unknown_files() {
    for syntax in syntect::parsing::SyntaxSet::load_defaults_newlines().syntaxes() {
        if syntax.name == "Plain Text" { continue; }
        for extension in &syntax.file_extensions {
            assert!(is_code_file(&format!("probe.{extension}")), "{}: {extension}", syntax.name);
        }
    }
    for name in ["notes.txt", "data.bin", "report.pdf", "README", "file.unknown"] {
        assert!(!is_code_file(name), "{name}");
    }
}

#[test]
fn document_and_image_formats_keep_their_primary_family() {
    for (name, expected) in [("image.svg", FileFamily::Images), ("README.md", FileFamily::Documents)] {
        assert_eq!(FileFamily::for_file(&FileEntry { name: name.into(), ..Default::default() }), expected);
    }
}

#[test]
fn powershell_modules_and_manifests_are_previewable_scripts() {
    for name in ["profile.ps1", "module.PSM1", "module.psd1", "run.sh", "run.bat"] {
        let file = FileEntry { name: name.into(), ..Default::default() };
        assert_eq!(FileFamily::for_file(&file), FileFamily::Scripts, "{name}");
        assert_eq!(get_preview_type(name), PreviewType::Text, "{name}");
    }
    assert_eq!(FileFamily::for_file(&FileEntry { name: "style.scss".into(), ..Default::default() }), FileFamily::Code);
}
