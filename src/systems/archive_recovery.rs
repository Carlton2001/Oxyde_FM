//! Parent-selected recovery locations, propagated across copy workers and operation wrappers.
use crate::models::error::ArchiveRecoveryFailure;
use std::{
    cell::RefCell,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(crate) struct Reports {
    retained: Locations,
    published: Locations,
}
#[derive(Default)]
struct Locations {
    paths: Vec<String>,
    bytes: usize,
    omitted: u64,
}
impl Locations {
    fn add(&mut self, path: &Path) {
        let path = path.to_string_lossy().into_owned();
        if self
            .paths
            .iter()
            .any(|existing| Path::new(&path).starts_with(existing))
        {
            return;
        }
        self.paths
            .retain(|existing| !Path::new(existing).starts_with(&path));
        self.bytes = self.paths.iter().map(String::len).sum();
        if self.paths.len() >= 8 || self.bytes.saturating_add(path.len()) > 256 * 1024 {
            self.omitted = self.omitted.saturating_add(1);
            log::warn!("Additional archive recovery or publication location: {path:?}");
            return;
        }
        self.bytes += path.len();
        self.paths.push(path);
    }
}
type Context = Arc<Mutex<Reports>>;
thread_local! { static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) }; }
pub(crate) fn context() -> Option<Context> {
    CONTEXT.with(|context| context.borrow().clone())
}
pub(crate) fn with_context<T>(context: Option<Context>, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<Context>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CONTEXT.with(|context| *context.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(CONTEXT.with(|current| current.replace(context)));
    run()
}
pub(crate) fn retained(path: &Path) {
    if let Some(context) = context() {
        if let Ok(mut report) = context.lock() {
            report.retained.add(path);
        }
    }
}
pub(crate) fn published(path: &Path) {
    if let Some(context) = context() {
        if let Ok(mut report) = context.lock() {
            report.published.add(path);
        }
    }
}
pub(crate) fn scope<T, E: std::fmt::Display + From<ArchiveRecoveryFailure>>(
    run: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    if context().is_some() {
        return run();
    }
    let context = Arc::new(Mutex::new(Reports::default()));
    let result = with_context(Some(context.clone()), run);
    let mut report = context.lock().unwrap_or_else(|error| error.into_inner());
    let completed = result.is_ok();
    if !completed {
        let published = std::mem::take(&mut report.published);
        for path in published.paths {
            report.retained.add(Path::new(&path));
        }
        report.retained.omitted = report.retained.omitted.saturating_add(published.omitted);
    }
    if report.retained.paths.is_empty() {
        return result;
    }
    let mut cause = result
        .as_ref()
        .err()
        .map(ToString::to_string)
        .unwrap_or_default();
    if cause.len() > 16 * 1024 {
        let mut end = 16 * 1024;
        while !cause.is_char_boundary(end) {
            end -= 1;
        }
        cause.truncate(end);
    }
    Err(ArchiveRecoveryFailure {
        completed,
        locations: std::mem::take(&mut report.retained.paths),
        omitted: report.retained.omitted,
        cause,
    }
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_messages_reject_unknown_fields_and_excessive_allocations() {
        let valid = ArchiveRecoveryFailure {
            completed: false,
            locations: vec!["C:/école/{$literal}".into()],
            omitted: 0,
            cause: String::new(),
        };
        assert_eq!(
            ArchiveRecoveryFailure::from_message(&format!("Archive Error: {valid}")),
            Some(valid.clone())
        );
        let mut value = serde_json::to_value(&valid).unwrap();
        value["worker_path"] = "C:/foreign".into();
        assert!(ArchiveRecoveryFailure::from_message(&format!(
            "{}{}",
            ArchiveRecoveryFailure::MARKER,
            value
        ))
        .is_none());
        for error in [
            ArchiveRecoveryFailure {
                locations: vec![],
                ..valid.clone()
            },
            ArchiveRecoveryFailure {
                locations: vec!["C:/w".into(); 9],
                ..valid.clone()
            },
            ArchiveRecoveryFailure {
                locations: vec!["x".repeat(256 * 1024 + 1)],
                ..valid.clone()
            },
            ArchiveRecoveryFailure {
                cause: "x".repeat(16 * 1024 + 1),
                ..valid
            },
        ] {
            assert!(ArchiveRecoveryFailure::from_message(&error.to_string()).is_none());
        }
        assert!(ArchiveRecoveryFailure::from_message(&format!(
            "{}{}",
            ArchiveRecoveryFailure::MARKER,
            "x".repeat(2 * 1024 * 1024 + 1)
        ))
        .is_none());
    }
    #[test]
    fn scopes_report_owned_locations_after_cleanup_and_keep_thread_context_bounded() {
        let result = scope::<(), String>(|| {
            let context = context();
            std::thread::spawn(move || {
                with_context(context, || {
                    for index in 0..12 {
                        retained(Path::new(&format!("C:/école/{index}")));
                    }
                })
            })
            .join()
            .unwrap();
            Ok(())
        })
        .unwrap_err();
        let report = ArchiveRecoveryFailure::from_message(&result).unwrap();
        assert!(report.completed);
        assert_eq!(report.locations.len(), 8);
        assert_eq!(report.omitted, 4);
        assert!(context().is_none());
        assert!(scope::<(), String>(|| {
            published(Path::new("C:/complete.zip"));
            Ok(())
        })
        .is_ok());
        let result = scope::<(), String>(|| {
            scope::<(), String>(|| {
                published(Path::new("C:/destination.zip"));
                Err("cancelled".into())
            })
        })
        .unwrap_err();
        let report = ArchiveRecoveryFailure::from_message(&result).unwrap();
        assert!(!report.completed);
        assert_eq!(report.cause, "cancelled");
        assert_eq!(report.locations, ["C:/destination.zip"]);
    }
}
