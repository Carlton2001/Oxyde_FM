use super::*;

fn content_matches(path: &Path, file: &mut File, content: &Content) -> io::Result<bool> {
    let size = file.metadata()?.len();
    if size < content.prefix_size
        || size
            > content
                .prefix_size
                .saturating_add(content.next.len() as u64)
    {
        return Ok(false);
    }
    file.seek(SeekFrom::Start(0))?;
    let mut hash = blake3::Hasher::new();
    let mut prefix = content.prefix_size;
    let mut buffer = [0u8; 64 * 1024];
    while prefix != 0 {
        let count = prefix.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..count])?;
        hash.update(&buffer[..count]);
        prefix -= count as u64;
    }
    if hash.finalize().to_hex().as_str() != content.prefix_hash {
        return Ok(false);
    }
    let tail = (size - content.prefix_size) as usize;
    let mut tail_bytes = vec![0; tail];
    file.read_exact(&mut tail_bytes)?;
    if tail_bytes != content.next[..tail] {
        return Ok(false);
    }
    // Root and every directory/file in this path have already been opened
    // exclusively for cleanup. Reopening HeldParents would conflict with the
    // DELETE access of those guards. Keep this path inside that retained tree.
    Ok(platform().read_file_provenance(path)? == content.provenance)
}

pub(in crate::systems::archive_open_cache) fn evict(
    path: &Path,
    policy: Policy,
    now: u64,
    pressure: bool,
    remaining: &mut usize,
) -> io::Result<bool> {
    let key = platform().cache_proof_key(
        &path
            .parent()
            .ok_or_else(|| io::Error::other("Missing cache proof parent"))?
            .join(".cache-proof-key"),
    )?;
    let root = platform().open_cache_cleanup(path, true)?;
    let bound_root = platform().retained_file_path(&root, path)?;
    let marker_path = path.join(OWNER);
    let marker = platform().open_cache_cleanup(&marker_path, false)?;
    let mut nodes = Vec::new();
    // Discovery is used only to refuse strangers. Every deletion below needs
    // a persisted, authenticated creation record and matching live handle.
    collect_cleanup(path, &marker_path, &mut nodes, remaining, 0)?;
    let mut valid = Vec::new();
    for slot in &mut nodes {
        if !SLOTS.iter().any(|name| slot.path == path.join(name)) {
            continue;
        }
        let mut bytes = Vec::new();
        (&mut slot.file)
            .take(MAX_PROOF as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_PROOF {
            continue;
        }
        let Ok(data) = platform().seal_cache_proof(&bytes, &key, false) else {
            continue;
        };
        let Ok(state) = serde_json::from_slice::<State>(&data) else {
            continue;
        };
        if state.version == 4
            && state.root == bound_root
            && state.records.len() <= MAX_RECORDS
            && state.stamp.matches(&root)?
        {
            valid.push(state);
        }
    }
    let Some(state) = valid.into_iter().max_by_key(|state| state.sequence) else {
        return Ok(false);
    };
    let age = if pressure || !state.finished {
        policy.launch_grace
    } else {
        policy.retention
    };
    if now.saturating_sub(state.created) < age.as_secs() {
        return Ok(false);
    }
    let mut records = HashMap::new();
    for record in &state.records {
        let name =
            crate::utils::archive_security::relative_archive_path(&record.path.to_string_lossy())?;
        if name != record.path || records.insert(name, record).is_some() {
            return Ok(false);
        }
    }
    let Some(owner) = records.get(Path::new(OWNER)) else {
        return Ok(false);
    };
    if !matches!(owner.kind, Kind::Marker) || !owner.stamp.matches(&marker)? {
        return Ok(false);
    }
    for node in &mut nodes {
        let relative = node.path.strip_prefix(path).map_err(io::Error::other)?;
        let Some(record) = records.get(relative) else {
            return Ok(false);
        };
        if !record.stamp.matches(&node.file)? {
            return Ok(false);
        }
        match &record.kind {
            Kind::Directory if node.directory => {}
            Kind::Slot
                if !node.directory && SLOTS.iter().any(|name| relative == Path::new(name)) => {}
            Kind::Payload(content) if !node.directory && relative.starts_with("data") => {
                if content.next.len() > crate::utils::archive_io_protocol::MAX_BLOCK
                    || !content_matches(&node.path, &mut node.file, content)?
                {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    // Validate the entire surviving tree before the first destructive call.
    let mut slots = Vec::new();
    for node in nodes {
        if SLOTS.iter().any(|name| node.path == path.join(name)) {
            slots.push(node);
            continue;
        }
        platform().remove_open_file(&node.file, &node.path)?;
        drop(node.file);
    }
    platform().remove_open_file(&marker, &marker_path)?;
    drop(marker);
    for slot in slots {
        platform().remove_open_file(&slot.file, &slot.path)?;
        drop(slot.file);
    }
    platform().remove_open_file(&root, path)?;
    drop(root);
    Ok(true)
}
