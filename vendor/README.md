# Vendored Rust dependency

`proc-macro-error2` is copied from the crates.io 2.0.1 release under its
MIT or Apache-2.0 license. Oxyde patches one line in `src/lib.rs`:
`extern crate proc_macro` becomes `pub extern crate proc_macro`.

GPUI 0.2.2 pulls this crate through `stacksafe-macro 0.1.4`. The patch removes
Rust's E0365 future-incompatibility warning. Replace this copy when GPUI's
dependency chain migrates to a maintained alternative.
