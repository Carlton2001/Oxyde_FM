//! Parent-side extraction admission. Check before interpreting any filesystem effect.
use super::archive_io_protocol::{invalid, Entry, Frame, Limits, Message};
use super::archive_security::relative_archive_path;
use std::collections::{HashMap, HashSet};
use std::io;

pub const MAX_MANIFEST_NAMES: usize = 64 * 1024 * 1024;

pub struct ExtractionState {
    limits: Limits,
    selected: Option<std::path::PathBuf>,
    preflight: bool,
    sealed: bool,
    finished: bool,
    entries: HashMap<u64, Entry>,
    names: HashSet<String>,
    completed: HashSet<u64>,
    active: Option<(u64, u64)>,
    name_bytes: usize,
    declared: u64,
    written: u64,
}

impl ExtractionState {
    pub fn new(limits: Limits, selected: Option<&str>, preflight: bool) -> io::Result<Self> {
        let selected = selected.map(relative_archive_path).transpose()?;
        if selected
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(invalid());
        }
        Ok(Self {
            limits,
            selected,
            preflight,
            sealed: false,
            finished: false,
            entries: HashMap::new(),
            names: HashSet::new(),
            completed: HashSet::new(),
            active: None,
            name_bytes: 0,
            declared: 0,
            written: 0,
        })
    }

    pub fn entry(&self, id: u64) -> io::Result<&Entry> {
        self.entries.get(&id).ok_or_else(invalid)
    }

    /// This gate has no IO. A rejected transition can never create or publish a file.
    pub fn accept(&mut self, frame: &Frame) -> io::Result<()> {
        frame.validate()?;
        if self.finished {
            return Err(invalid());
        }
        match &frame.message {
            Message::Entry { entry } => {
                if self.sealed
                    || self.active.is_some()
                    || entry.id != self.entries.len() as u64
                    || self.entries.len() as u64 >= self.limits.entries
                {
                    return Err(invalid());
                }
                entry.validate()?;
                let path = relative_archive_path(&entry.name)?;
                if path.components().count() as u64 > self.limits.depth {
                    return Err(invalid());
                }
                let key = path.to_string_lossy().replace('\\', "/").to_lowercase();
                if self.names.contains(&key) {
                    return Err(invalid());
                }
                let name_bytes = self
                    .name_bytes
                    .checked_add(entry.name.len())
                    .filter(|n| *n <= MAX_MANIFEST_NAMES)
                    .ok_or_else(invalid)?;
                let selected = self
                    .selected
                    .as_ref()
                    .is_none_or(|selected| *selected == path);
                if selected && self.selected.is_some() && entry.directory {
                    return Err(invalid());
                }
                let declared = if selected {
                    self.declared
                        .checked_add(entry.size)
                        .filter(|n| *n <= self.limits.bytes)
                        .ok_or_else(invalid)?
                } else {
                    self.declared
                };
                self.names.insert(key);
                self.entries.insert(entry.id, entry.clone());
                self.name_bytes = name_bytes;
                self.declared = declared;
            }
            Message::ManifestEnd => {
                if self.sealed || self.active.is_some() {
                    return Err(invalid());
                }
                if let Some(selected) = &self.selected {
                    if !self.entries.values().any(|entry| {
                        relative_archive_path(&entry.name).is_ok_and(|path| path == *selected)
                    }) {
                        return Err(invalid());
                    }
                }
                self.sealed = true;
            }
            Message::Begin { id } => {
                if (self.preflight && !self.sealed)
                    || self.active.is_some()
                    || self.completed.contains(id)
                {
                    return Err(invalid());
                }
                let entry = self.entry(*id)?;
                if let Some(selected) = &self.selected {
                    if relative_archive_path(&entry.name)? != *selected {
                        return Err(invalid());
                    }
                }
                self.active = Some((*id, 0));
            }
            Message::Chunk { id, offset } => {
                let Some((active, count)) = self.active else {
                    return Err(invalid());
                };
                let entry = self.entry(*id)?;
                if active != *id || count != *offset || entry.directory || frame.data.is_empty() {
                    return Err(invalid());
                }
                let count = count
                    .checked_add(frame.data.len() as u64)
                    .filter(|n| *n <= entry.size)
                    .ok_or_else(invalid)?;
                let written = self
                    .written
                    .checked_add(frame.data.len() as u64)
                    .filter(|n| *n <= self.limits.bytes)
                    .ok_or_else(invalid)?;
                self.active = Some((*id, count));
                self.written = written;
            }
            Message::End { id } => {
                let Some((active, count)) = self.active else {
                    return Err(invalid());
                };
                if active != *id || count != self.entry(*id)?.size {
                    return Err(invalid());
                }
                self.completed.insert(*id);
                self.active = None;
            }
            Message::Finished => {
                if self.active.is_some()
                    || (self.preflight && !self.sealed)
                    || self.completed.len()
                        != if self.selected.is_some() {
                            1
                        } else {
                            self.entries.len()
                        }
                {
                    return Err(invalid());
                }
                self.finished = true;
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }

    pub fn finished(&self) -> bool {
        self.finished
    }
    pub fn written(&self) -> u64 {
        self.written
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state(selected: Option<&str>, preflight: bool) -> ExtractionState {
        ExtractionState::new(
            Limits {
                entries: 3,
                bytes: 4,
                depth: 2,
                seconds: 1,
            },
            selected,
            preflight,
        )
        .unwrap()
    }
    fn entry(id: u64, name: &str, size: u64) -> Frame {
        Frame::new(Message::Entry {
            entry: Entry {
                id,
                name: name.into(),
                directory: false,
                size,
                modified: None,
                created: None,
                accessed: None,
                mode: 0o644,
            },
        })
    }
    fn accept(state: &mut ExtractionState, message: Message) {
        state.accept(&Frame::new(message)).unwrap();
    }

    #[test]
    fn preflight_gates_all_content_and_requires_a_complete_terminal_result() {
        let mut state = state(None, true);
        state.accept(&entry(0, "safe", 4)).unwrap();
        assert!(state.accept(&Frame::new(Message::Begin { id: 0 })).is_err());
        assert!(state.accept(&entry(1, "../late-danger", 0)).is_err());
        accept(&mut state, Message::ManifestEnd);
        assert!(state.accept(&Frame::new(Message::Finished)).is_err());
        accept(&mut state, Message::Begin { id: 0 });
        assert!(state.accept(&Frame::new(Message::End { id: 0 })).is_err());
        state
            .accept(&Frame::data(
                Message::Chunk { id: 0, offset: 0 },
                b"safe".to_vec(),
            ))
            .unwrap();
        accept(&mut state, Message::End { id: 0 });
        accept(&mut state, Message::Finished);
        assert_eq!(state.written(), 4);
        assert!(state.finished());
        assert!(state.accept(&Frame::new(Message::Finished)).is_err());
    }

    #[test]
    fn selection_admits_large_neighbors_but_never_their_content() {
        let mut state = state(Some("docs/selected"), true);
        state.accept(&entry(0, "neighbor", u64::MAX)).unwrap();
        state.accept(&entry(1, "docs/selected", 0)).unwrap();
        accept(&mut state, Message::ManifestEnd);
        assert!(state.accept(&Frame::new(Message::Begin { id: 0 })).is_err());
        accept(&mut state, Message::Begin { id: 1 });
        accept(&mut state, Message::End { id: 1 });
        accept(&mut state, Message::Finished);
        assert!(state.finished());
    }

    #[test]
    fn duplicate_names_offsets_ids_and_budget_overflows_are_refused() {
        let mut state = state(None, false);
        state.accept(&entry(0, "docs/file", 4)).unwrap();
        for frame in [
            entry(1, "DOCS\\FILE", 0),
            entry(2, "foreign-id", 0),
            entry(1, "a/b/c", 0),
            entry(1, "extra", 1),
        ] {
            assert!(state.accept(&frame).is_err());
        }
        accept(&mut state, Message::Begin { id: 0 });
        for (id, offset, bytes) in [(1, 0, 1), (0, 1, 1), (0, 0, 5)] {
            assert!(state
                .accept(&Frame::data(Message::Chunk { id, offset }, vec![0; bytes]))
                .is_err());
        }
        state
            .accept(&Frame::data(
                Message::Chunk { id: 0, offset: 0 },
                vec![0; 4],
            ))
            .unwrap();
        accept(&mut state, Message::End { id: 0 });
        assert!(state.accept(&Frame::new(Message::Begin { id: 0 })).is_err());
        assert!(state
            .accept(&Frame::data(Message::Chunk { id: 0, offset: 4 }, vec![0]))
            .is_err());
        accept(&mut state, Message::Finished);
    }

    #[test]
    fn missing_selected_member_cannot_seal_the_manifest() {
        let mut state = state(Some("absent"), true);
        state.accept(&entry(0, "other", 0)).unwrap();
        assert!(state.accept(&Frame::new(Message::ManifestEnd)).is_err());
        assert!(state.accept(&Frame::new(Message::Finished)).is_err());
    }
}
