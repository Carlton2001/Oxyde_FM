//! Brief transitions for elements that appear beside the user's action.
//!
//! Springs are critically damped (no overshoot) and retarget without restarting,
//! so reversing a fade mid-way never jumps. GPUI's `reduce_motion` flag, driven
//! by the animations preference, resolves every spring to its target instantly.

use gpui::{SpringAnimation, SpringConfig};

/// Settles in about 110 ms.
const ENTER: SpringConfig = SpringConfig::new(3600.0, 120.0, 1.0);
/// Settles in about 85 ms: leaving faster than arriving keeps the interface crisp.
const EXIT: SpringConfig = SpringConfig::new(5625.0, 150.0, 1.0);
/// Settles in about 220 ms, for an end-of-activity signal worth noticing.
const LINGER: SpringConfig = SpringConfig::new(900.0, 60.0, 1.0);
/// Visually indistinguishable from the target for opacities and fractions.
const EPSILON: f32 = 0.01;

/// Opacity-like progress toward 1 when `visible`, toward 0 otherwise.
pub(crate) fn presence(visible: bool) -> SpringAnimation<f32> {
    toward(if visible { 1.0 } else { 0.0 }, visible)
}

/// Spring toward an arbitrary scalar, using the entering pace when `entering`.
pub(crate) fn toward(target: f32, entering: bool) -> SpringAnimation<f32> {
    SpringAnimation::new(if entering { ENTER } else { EXIT })
        .to(target)
        .with_epsilon(EPSILON)
}

/// Like [`presence`], but fading out slowly enough to read as "finished".
pub(crate) fn linger(visible: bool) -> SpringAnimation<f32> {
    SpringAnimation::new(if visible { ENTER } else { LINGER })
        .to(if visible { 1.0 } else { 0.0 })
        .with_epsilon(EPSILON)
}

/// Spring that moves a fraction of its container, precise to 0.2 %.
pub(crate) fn slide(target: f32) -> SpringAnimation<f32> {
    SpringAnimation::new(EXIT).to(target).with_epsilon(0.002)
}

#[cfg(test)]
#[path = "motion/tests/settle_tests.rs"]
mod settle_tests;
