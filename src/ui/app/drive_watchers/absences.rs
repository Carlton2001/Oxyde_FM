//! A drive missing from a survey is gone only once it has stayed missing for a while:
//! a network drive or a key being enumerated again vanishes for a moment, and a single
//! notification brings several surveys within a second.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long a drive may be missing before its tabs leave it. The surveys that follow a
/// notification come at 0.15, 0.5, 1.25 and 2.75 s: the one at 2.75 s settles it.
pub(super) const GRACE: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(super) struct DriveAbsences {
    since: HashMap<String, Instant>,
}

impl DriveAbsences {
    /// Of the drives `missing` from the survey made at `now`, those still to be shown.
    /// A drive back in a survey starts again from zero.
    pub(super) fn still_shown<T>(&mut self, missing: Vec<T>, path: impl Fn(&T) -> &str, now: Instant) -> Vec<T> {
        let mut since = HashMap::with_capacity(missing.len());
        let mut shown = Vec::new();
        for drive in missing {
            let first = self.since.get(path(&drive)).copied().unwrap_or(now);
            since.insert(path(&drive).to_owned(), first);
            if now.saturating_duration_since(first) < GRACE {
                shown.push(drive);
            }
        }
        self.since = since;
        shown
    }
}

#[cfg(test)]
#[path = "tests/absences.rs"]
mod tests;
