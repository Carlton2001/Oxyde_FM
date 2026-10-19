# Additional Syntect grammars

These are syntax definitions, not executable plugins. Syntect loads them once
on the background text job. SCSS/Sass are unmodified. PowerShell has the
compatibility adaptation documented below.

## PowerShell

- `PowerShell.sublime-syntax`: bat's Sublime Text 3 conversion.
- Source: https://github.com/sharkdp/bat/blob/f748c1a467ae161682849949a7d036ab8362f9ba/assets/syntaxes/02_Extra/PowerShell.sublime-syntax
- Original: https://github.com/PowerShell/EditorSyntax at revision
  `a08b55bf1146c210f58e844be53c2aa78fd5e610` (referenced by bat).
- MIT notices: `PowerShell-LICENSE.txt`, `PowerShell-NOTICES.txt`,
  `bat-LICENSE-MIT.txt`.
- Extensions: `.ps1`, `.psm1`, `.psd1`.
- Oxyde adaptation: in `unicodeEscape`, replace the Oniguruma subroutine call
  `\g<1>` by `[0-9a-fA-F]`, the exact character class in group 1. This preserves
  matching while avoiding a feature unsupported by the `fancy-regex` backend.

## SCSS and Sass

- `SCSS.sublime-syntax` depends on contexts in `Sass.sublime-syntax`.
- Source: https://github.com/SublimeText/Sass/tree/d3d94046409db6fbbc9d51dea52b589ecc9d3d48/Syntaxes
- Revision `d3d94046409db6fbbc9d51dea52b589ecc9d3d48` is also referenced by bat.
- `SCSS-LICENSE.txt` (MIT) covers both files.
- YAML frontmatter resolves to Syntect's existing YAML grammar.

Updating these definitions requires the linking and semantic tests in
`src/ui/app/viewer/tests/syntax.rs`. Distribution copies this README and the
license/notice files under `licenses/syntaxes/`. These notices cover these
additional grammars only, not every third-party component in Oxyde.
