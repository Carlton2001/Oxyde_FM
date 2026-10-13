//! Persistent creation proofs, never ownership inferred from a directory scan.
use super::*;
use crate::platform::FileIdentity;
use std::{
    collections::HashMap,
    io::{Seek, SeekFrom},
    sync::{Arc, Mutex},
};
pub(super) const SLOTS: [&str; 2] = [".owner-0.proof", ".owner-1.proof"];
const MAX_PROOF: usize = 2 * 1024 * 1024;
const MAX_RECORDS: usize = 128;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Stamp {
    identity: FileIdentity,
    created: Option<SystemTime>,
}
impl Stamp {
    fn new(file: &File) -> io::Result<Self> {
        Ok(Self {
            identity: platform().file_identity(file)?,
            created: file.metadata()?.created().ok(),
        })
    }
    fn matches(&self, file: &File) -> io::Result<bool> {
        Ok(self.identity == platform().file_identity(file)?
            && self.created == file.metadata()?.created().ok())
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Content {
    prefix_size: u64,
    prefix_hash: String,
    next: Vec<u8>,
    provenance: Option<Vec<u8>>,
}
impl Default for Content {
    fn default() -> Self {
        Self {
            prefix_size: 0,
            prefix_hash: blake3::hash(&[]).to_hex().to_string(),
            next: Vec::new(),
            provenance: None,
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
enum Kind {
    Directory,
    Marker,
    Slot,
    Payload(Content),
}
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Record {
    path: PathBuf,
    stamp: Stamp,
    kind: Kind,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    sequence: u64,
    root: PathBuf,
    stamp: Stamp,
    created: u64,
    finished: bool,
    records: Vec<Record>,
}

pub(crate) struct Journal {
    state: State,
    slots: [File; 2],
    written: HashMap<PathBuf, (u64, blake3::Hasher)>,
    key: [u8; 32],
}
impl Journal {
    pub(super) fn new(
        owner: &mut super::super::archive_owned_tree::OwnedTree,
        created: u64,
    ) -> io::Result<Arc<Mutex<Self>>> {
        let key = platform().cache_proof_key(&owner.path().parent().ok_or_else(|| io::Error::other("Missing cache proof parent"))?.join(".cache-proof-key"))?;
        let first = owner.create_file(&owner.path().join(SLOTS[0]))?;
        let second = owner.create_file(&owner.path().join(SLOTS[1]))?;
        let mut records = Vec::new();
        for (path, file, directory) in owner.recovery_records() {
            let relative = path
                .strip_prefix(owner.path())
                .map_err(io::Error::other)?
                .to_path_buf();
            let kind = if directory {
                Kind::Directory
            } else if relative == Path::new(OWNER) {
                Kind::Marker
            } else if SLOTS.iter().any(|name| relative == Path::new(name)) {
                Kind::Slot
            } else {
                return Err(io::Error::other(
                    "Cache proof initialized after payload creation",
                ));
            };
            records.push(Record {
                path: relative,
                stamp: Stamp::new(file)?,
                kind,
            });
        }
        let mut journal = Self {
            state: State {
                version: 4,
                sequence: 0,
                root: owner.path().into(),
                stamp: Stamp::new(owner.recovery_root())?,
                created,
                finished: false,
                records,
            },
            slots: [first, second],
            written: HashMap::new(),
            key,
        };
        journal.persist()?;
        journal.persist()?;
        Ok(Arc::new(Mutex::new(journal)))
    }
    fn persist(&mut self) -> io::Result<()> {
        if self.state.records.len() > MAX_RECORDS {
            return Err(io::Error::other("Cache creation proof count exceeded"));
        }
        self.state.sequence = self
            .state
            .sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Cache proof sequence exhausted"))?;
        let data = serde_json::to_vec(&self.state)?;
        if data.len() > MAX_PROOF {
            return Err(io::Error::other("Cache creation proof size exceeded"));
        }
        let sealed = platform().seal_cache_proof(&data, &self.key, true)?;
        let slot = &mut self.slots[self.state.sequence as usize % 2];
        slot.seek(SeekFrom::Start(0))?;
        slot.write_all(&sealed)?;
        slot.set_len(sealed.len() as u64)?;
        slot.sync_all()
    }
    pub(crate) fn created(&mut self, path: &Path, file: &File, directory: bool) -> io::Result<()> {
        let path = path
            .strip_prefix(&self.state.root)
            .map_err(io::Error::other)?
            .to_path_buf();
        if !path.starts_with("data") || self.state.records.iter().any(|record| record.path == path)
        {
            return Err(io::Error::other("Invalid cache creation proof path"));
        }
        self.state.records.push(Record {
            path: path.clone(),
            stamp: Stamp::new(file)?,
            kind: if directory {
                Kind::Directory
            } else {
                Kind::Payload(Content::default())
            },
        });
        if !directory {
            self.written.insert(path, (0, blake3::Hasher::new()));
        }
        self.persist()
    }
    pub(crate) fn before_write(&mut self, path: &Path, file: &File, data: &[u8]) -> io::Result<()> {
        if data.len() > crate::utils::archive_io_protocol::MAX_BLOCK {
            return Err(io::Error::other("Cache write proof block exceeded"));
        }
        let path = path
            .strip_prefix(&self.state.root)
            .map_err(io::Error::other)?;
        let (size, hash) = self
            .written
            .get(path)
            .ok_or_else(|| io::Error::other("Missing cache write proof"))?;
        let record = self
            .state
            .records
            .iter_mut()
            .find(|record| record.path == path)
            .ok_or_else(|| io::Error::other("Missing cache creation proof"))?;
        if !record.stamp.matches(file)? {
            return Err(io::Error::other("Cache output replaced"));
        }
        record.kind = Kind::Payload(Content {
            prefix_size: *size,
            prefix_hash: hash.finalize().to_hex().to_string(),
            next: data.to_vec(),
            provenance: None,
        });
        // Durable expected bytes precede the payload write. A crash halfway
        // through that write can still be verified as an exact prefix.
        self.persist()?;
        let (size, hash) = self.written.get_mut(path).unwrap();
        *size = size
            .checked_add(data.len() as u64)
            .ok_or_else(|| io::Error::other("Cache write size overflow"))?;
        hash.update(data);
        Ok(())
    }
    pub(crate) fn complete(&mut self, path: &Path, file: &File) -> io::Result<()> {
        let relative = path
            .strip_prefix(&self.state.root)
            .map_err(io::Error::other)?;
        let (size, hash) = self
            .written
            .get(relative)
            .ok_or_else(|| io::Error::other("Missing cache write proof"))?;
        let parents = super::super::path_parents::HeldParents::new(path)?;
        let provenance =
            platform().read_archive_output_provenance(parents.directory(), file, path)?;
        let record = self
            .state
            .records
            .iter_mut()
            .find(|record| record.path == relative)
            .unwrap();
        record.kind = Kind::Payload(Content {
            prefix_size: *size,
            prefix_hash: hash.finalize().to_hex().to_string(),
            next: Vec::new(),
            provenance,
        });
        self.persist()
    }
    pub(super) fn finish(&mut self) -> io::Result<()> {
        self.state.finished = true;
        self.persist()
    }
}

#[path = "recovery/reclaim.rs"]
mod reclaim;
pub(super) use reclaim::evict;

#[cfg(test)]
#[path = "tests/recovery_tests.rs"]
mod tests;
