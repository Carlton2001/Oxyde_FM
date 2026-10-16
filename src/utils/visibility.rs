/// Listing and search use the same visibility contract: System takes precedence
/// over Hidden when both attributes are set. Names alone never imply System.
pub(crate) fn is_visible(hidden: bool, system: bool, show_hidden: bool, show_system: bool) -> bool {
    if system {
        show_system
    } else {
        show_hidden || !hidden
    }
}

#[cfg(test)]
#[path = "visibility/tests/attributes.rs"]
mod tests;
