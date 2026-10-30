//! Scoped fault injection for production archive mutation integration tests.
use std::{cell::RefCell, io, path::Path};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    BeforeRebuild,
    BeforeOriginalCheck,
    AfterBackup,
    BeforeRollback,
    BeforeBackupRemoval,
    CacheWritten,
    BrokerBeforeSourceRead,
}
type Hook = Box<dyn FnMut(Stage, &Path, &Path) -> io::Result<()>>;
thread_local! { static HOOK: RefCell<Option<Hook>> = const { RefCell::new(None) }; }
#[doc(hidden)]
pub fn with_hook_for_tests<T>(
    hook: impl FnMut(Stage, &Path, &Path) -> io::Result<()> + 'static,
    run: impl FnOnce() -> T,
) -> T {
    struct Reset(Option<Hook>);
    impl Drop for Reset {
        fn drop(&mut self) {
            HOOK.with(|hook| *hook.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(HOOK.with(|current| current.replace(Some(Box::new(hook)))));
    run()
}
#[cfg(not(test))]
pub(crate) fn checkpoint(stage: Stage, first: &Path, second: &Path) -> io::Result<()> {
    HOOK.with(|hook| match hook.borrow_mut().as_mut() {
        Some(hook) => hook(stage, first, second),
        None => Ok(()),
    })
}
