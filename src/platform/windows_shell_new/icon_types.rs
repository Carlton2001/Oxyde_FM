//! Type metadata for native New menu entries. The Shell still supplies the commands.

use super::{HKEY_CLASSES_ROOT, extensions, package_types, registration, registration_label};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
static CACHE: Mutex<Option<(Instant, Arc<IconTypes>)>> = Mutex::new(None);

#[derive(Default)]
pub(in crate::platform) struct IconTypes(HashMap<String, Option<String>>);

fn label_key(label: &str) -> String {
    label
        .trim()
        .chars()
        .filter(|&ch| ch != '&')
        .flat_map(char::to_lowercase)
        .collect()
}

impl IconTypes {
    fn add(&mut self, label: &str, extension: &str) {
        let extension = extension.to_lowercase();
        self.0
            .entry(label_key(label))
            .and_modify(|entry| {
                if entry
                    .as_ref()
                    .is_some_and(|previous| previous != &extension)
                {
                    *entry = None;
                }
            })
            .or_insert(Some(extension));
    }

    pub(in crate::platform) fn extension(&self, label: &str) -> Option<&str> {
        self.0.get(&label_key(label)).and_then(Option::as_deref)
    }
}

pub(in crate::platform) fn icon_types() -> Arc<IconTypes> {
    if let Some((_, types)) = CACHE
        .lock()
        .unwrap()
        .as_ref()
        .filter(|(loaded_at, _)| loaded_at.elapsed() < REFRESH_INTERVAL)
    {
        return types.clone();
    }

    let mut types = IconTypes::default();
    let root = HKEY_CLASSES_ROOT;
    if let Ok(extensions) = extensions(root) {
        for extension in extensions {
            if let Some(registration) = registration(root, &extension) {
                if let Some(label) = registration_label(root, &registration, &extension) {
                    types.add(&label, &extension);
                }
            }
        }
    }
    if let Err(error) =
        package_types::visit_installed(|label, extension| types.add(label, extension))
    {
        log::debug!("Cannot read packaged Shell New types: {error}");
    }
    let types = Arc::new(types);
    *CACHE.lock().unwrap() = Some((Instant::now(), types.clone()));
    types
}

#[cfg(test)]
#[path = "tests/icon_types.rs"]
mod tests;
