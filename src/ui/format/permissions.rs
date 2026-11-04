//! Unix permissions in the details column: octal there, symbolic in its tooltip.

#[cfg(test)]
#[path = "permissions/tests/notation.rs"]
mod notation_tests;

/// Three digits, or four when setuid, setgid or sticky is set (`1777`, `4755`).
pub fn octal(mode: u32) -> String {
    let mode = mode & 0o7777;
    if mode > 0o777 { format!("{mode:04o}") } else { format!("{mode:03o}") }
}

/// Octal backed by the symbolic access bits, as `755  rwxr-xr-x`: filter and tooltip.
pub fn paired(mode: u32) -> String {
    format!("{}  {}", octal(mode), &symbolic(mode, false, false)[1..])
}

/// The `ls -l` form: type, then owner, group and others, with `s`/`t` for special bits.
pub fn symbolic(mode: u32, is_dir: bool, is_symlink: bool) -> String {
    let mut text = String::with_capacity(10);
    text.push(if is_symlink { 'l' } else if is_dir { 'd' } else { '-' });
    for (shift, special, mark) in [(6, 0o4000, 's'), (3, 0o2000, 's'), (0, 0o1000, 't')] {
        let bits = (mode >> shift) & 0o7;
        text.push(if bits & 4 != 0 { 'r' } else { '-' });
        text.push(if bits & 2 != 0 { 'w' } else { '-' });
        let execute = bits & 1 != 0;
        text.push(match (mode & special != 0, execute) {
            (true, true) => mark,
            (true, false) => mark.to_ascii_uppercase(),
            (false, true) => 'x',
            (false, false) => '-',
        });
    }
    text
}
