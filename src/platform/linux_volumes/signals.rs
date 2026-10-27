//! UDisks2 announces plugged and removed devices, which the mount table does not show
//! while they stay unmounted, and the mount points it records after the kernel's.
use super::udisks::SERVICE;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type;
use zbus::zvariant::OwnedValue;
use zbus::MatchRule;

/// A blocking signal iterator cannot be woken: dropping the guard ends the thread at the
/// next UDisks2 signal.
pub(super) struct Signals {
    stopped: Arc<AtomicBool>,
}

impl Drop for Signals {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
    }
}

pub(super) fn start(revision: Arc<AtomicU64>) -> zbus::Result<Signals> {
    // A connection of its own, so that waiting never holds the one blocking calls share.
    let connection = Connection::system()?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(SERVICE)?
        .path_namespace("/org/freedesktop/UDisks2")?
        .build();
    let signals = MessageIterator::for_match_rule(rule, &connection, None)?;
    let stopped = Arc::new(AtomicBool::new(false));
    let stop = stopped.clone();
    std::thread::Builder::new()
        .name("oxyde-udisks-signals".into())
        .spawn(move || {
            let _connection = connection;
            for message in signals.flatten() {
                if stop.load(Ordering::Acquire) {
                    return;
                }
                if relevant(&message) {
                    revision.fetch_add(1, Ordering::Release);
                }
            }
        })
        .map_err(|error| zbus::Error::InputOutput(Arc::new(error)))?;
    Ok(Signals { stopped })
}

/// Devices added or removed, and filesystem or block changes such as mount points and
/// labels; job progress and drive health readings are ignored.
fn relevant(message: &zbus::Message) -> bool {
    let header = message.header();
    match header.member().map(|member| member.as_str()) {
        Some("InterfacesAdded" | "InterfacesRemoved") => true,
        Some("PropertiesChanged") => message
            .body()
            .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            .is_ok_and(|(interface, _, _)| {
                matches!(interface.as_str(), "org.freedesktop.UDisks2.Filesystem" | "org.freedesktop.UDisks2.Block")
            }),
        _ => false,
    }
}
