/// The standard field is the default for every new form or window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum InputAppearance {
    #[default]
    Standard,
    /// Borderless editor inside a shared field frame with icons or controls.
    Embedded,
    /// Borderless, unpadded editor inside a navigation or numeric control.
    Bare,
    /// Bordered editor whose height and typography follow its surrounding row.
    Inline,
}

/// Presentation and explicit opt-in behavior; callbacks remain with the caller.
#[derive(Default)]
pub(crate) struct TextInputOptions {
    pub appearance: InputAppearance,
    pub clearable: bool,
    pub submit_on_outside: bool,
    pub initial_selection: Option<std::ops::Range<usize>>,
}
