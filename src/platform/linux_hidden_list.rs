//! The `.hidden` file a directory may hold: one name per line, honored by GIO and Nautilus.

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

/// Larger lists are ignored, as GIO bounds the file it loads.
const MAX_LIST_BYTES: u64 = 1024 * 1024;

pub(super) fn read(directory: &Path) -> HashSet<String> {
    let Ok(file) = std::fs::File::open(directory.join(".hidden")) else {
        return HashSet::new();
    };
    if !file.metadata().is_ok_and(|metadata| metadata.is_file() && metadata.len() <= MAX_LIST_BYTES) {
        return HashSet::new();
    }
    let mut bytes = Vec::new();
    if file.take(MAX_LIST_BYTES).read_to_end(&mut bytes).is_err() {
        return HashSet::new();
    }
    parse(&String::from_utf8_lossy(&bytes))
}

fn parse(text: &str) -> HashSet<String> {
    text.split('\n').filter(|name| !name.is_empty()).map(str::to_owned).collect()
}

#[cfg(test)]
#[path = "linux_hidden_list/tests/list.rs"]
mod tests;
