//! Pure validation for full-atlas initial data, shared by renderer and tests.
pub(crate) fn can_initialize_full_texture(width: i32, height: i32) -> bool {
    (1024..=16384).contains(&width) && (1024..=16384).contains(&height)
}

pub(crate) fn has_full_texture_bytes(
    width: i32,
    height: i32,
    bytes_per_pixel: u32,
    length: usize,
) -> bool {
    let Ok(width) = usize::try_from(width) else {
        return false;
    };
    let Ok(height) = usize::try_from(height) else {
        return false;
    };
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel as usize))
        .is_some_and(|required| length >= required)
}

#[cfg(test)]
#[path = "tests/directx_atlas_memory.rs"]
mod tests;
