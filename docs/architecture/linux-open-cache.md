# Linux cache of files opened from an archive

Opening a file inside an archive extracts it into a cache entry, then hands it to the associated application. The cache is bounded (1 GiB, 512 entries, 256 MiB per file). A closed, intact entry leaves after 7 days, or after 60 seconds when room is short, oldest first. A modified copy, one still open, or one without ownership proof stays. Windows keeps its v2 cache and authenticated recovery.

## Rules

- **Location**: `$XDG_CACHE_HOME/Oxyde` (`~/.cache/Oxyde`), private to the user; `data/cache` in portable mode. On Windows `%TEMP%` is already per user. A shared `/tmp` folder would let another account block the cache.
- **Proof of closure**: a write lease (`F_SETLEASE`, `F_WRLCK`) on each file of the entry, marker included. The kernel grants it only if no other file description or memory mapping exists, in any process, and while held a new opener waits. It is the equivalent of Windows exclusive sharing; `flock` gives no such proof.
  - File open elsewhere: the whole entry stays and nothing is deleted.
  - Just before deleting, each lease is rechecked: an opener arriving meanwhile keeps the entry.
  - Breaking a lease signals its holder by `SIGIO`, which would terminate Oxyde: so a signal ignored by default (`SIGURG`) and no recipient.
  - System without leases (FUSE, NFS): nothing is evicted.
- **Deletion**: by the held object; a replacement published at the same path is kept.
- The rest of the contract is shared: links refused, BLAKE3 hash of content, single-file entries, preparations in progress protected by their open marker. A preparation interrupted by a crash is taken up 60 seconds later, once its marker is no longer open.

`Platform::confirm_cache_cleanup` has no effect on Windows.

## Remaining gaps

An application that reads then closes the file (most editors) is not visible: its copy can leave under pressure after 60 seconds, as with Notepad on Windows. An already modified copy stays protected by its hash, and an editor placing a lock file beside it (LibreOffice) protects the entry.

## Pitfalls

- A process being launched shares all descriptors until its `exec`: a lease or `flock` can be refused for a few milliseconds. Eviction retries twice; admission already waits on the lock.
- Opening a leased file with `O_NONBLOCK` returns `EWOULDBLOCK` and still starts breaking the lease.
- A memory mapping prevents the lease even after its descriptor closes; an `O_PATH` descriptor does not.
