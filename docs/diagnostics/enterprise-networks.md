# Enterprise networks

Situations met on company networks that Oxyde may handle differently from Explorer. Most are hypotheses, not verified failures. Track each one here: status, how to simulate it, and the result.

Statuses: **fixed**, **to test**, **not simulable** (needs infrastructure we lack), **out of scope**.

## Reference case: issue #13

A read-only share whose parent folders the user may traverse but not list. The SMB server refuses any open of those parents, even attribute-only or with no access, while Explorer never opens them. Source selection held every ancestor and walked from the share root, so it failed with os error 5. A first fix (`7df48879`) skips refused ancestors and resumes the no-reparse walk below them. The reporter could however list every folder: the actual cause is N16, opens asking for more than reading. The limit is recorded in [security status](../security/status.md) (R4).

## Local test share

Windows 11 Pro, elevated PowerShell. This reproduces #13 and serves as the base for the other cases.

```powershell
$user = 'oxyde_test'; $pw = Read-Host 'Password'
New-LocalUser -Name $user -Password (ConvertTo-SecureString $pw -AsPlainText -Force) -PasswordNeverExpires
$root = 'C:\OxydeShareTest'; New-Item -ItemType Directory -Force "$root\Engineering\Dokumentation" | Out-Null
Set-Content "$root\Engineering\Dokumentation\test.xlsx" 'test'
icacls $root /inheritance:r /grant:r '*S-1-5-32-544:(OI)(CI)F' '*S-1-5-18:(OI)(CI)F' "$env:USERDOMAIN\${env:USERNAME}:(OI)(CI)F" "${user}:(X,RA)"
icacls "$root\Engineering" /grant "${user}:(X,RA)"
icacls "$root\Engineering\Dokumentation" /grant "${user}:(OI)(CI)RX"
New-SmbShare -Name OxydeRO -Path $root -ReadAccess $user, "$env:USERDOMAIN\$env:USERNAME"
```

Then, **not elevated**: `net use T: \\127.0.0.1\OxydeRO <password> /user:<computer>\oxyde_test /persistent:no`. Use `127.0.0.1`, not the computer name, which already has a session under another identity (error 1219). Cleanup: `net use T: /delete`, `Remove-SmbShare OxydeRO`, `Remove-LocalUser oxyde_test`, delete the folder.

## Cases

| # | Situation | Possible failure in Oxyde | Simulation | Status |
| --- | --- | --- | --- | --- |
| N1 | Parents that can be traversed but not listed | Source selection refused (os error 5) | Local test share | **fixed** (`7df48879`), on an ACL without Synchronize, which the Windows dialog always grants: the reporter's real case is N16 |
| N2 | DFS namespace (`\\domain\dfs\…` → `\\srv\share$\…`) | Canonical path differs from the typed one: "destination inside source", copy onto itself and overlapping sources go undetected or are wrongly refused. DFS links are server reparse points: no-reparse lookup may refuse them (see issue #8) | Needs Windows Server; a server-side `mklink /D` inside the share covers part of it | partly tested: a file behind a server junction copies; the junction itself is refused, as every Windows reparse point is ([links](../architecture/links.md)). Real DFS untested |
| N3 | Access-based enumeration (ABE) | Hidden but existing entries: conflict naming picks a name that exists, a folder looks empty | `Set-SmbShare OxydeRO -FolderEnumerationMode AccessBased` | tested on the drop folder (N5): hidden entries stay hidden, a folder holding only hidden entries looks empty, copying onto a hidden name fails with "file exists" (os error 80) and leaves it intact. A copy into its own folder picks `name - Copie` from the visible names: when that name is hidden it fails the same way, without trying the next one. Safe, left as a known limit |
| N4 | File open elsewhere (Excel lock) | Source opened without `FILE_SHARE_WRITE`: sharing violation (os error 32) where Explorer copies | Open the file in Excel, or keep a `[IO.File]::Open(..., 'Open', 'ReadWrite', 'Read')` handle | **fixed**: the copy source and the native copy guard now share writing, like Explorer; a writer can still change the file during the copy |
| N5 | Drop folder: create allowed, modify/delete refused | Temporary file then rename, replace on conflict, Undo and rollback fail midway; check partial effects are reported and nothing is lost | ACL `(AD,WD)` without `(M)` / `(D)` on the destination | **fixed** (retested): any copy into the drop folder failed with access denied, even a new file, where PowerShell creates it: Oxyde staged into a file it then had to rename and delete (DELETE access). A new file is now written directly under its name and reread with read access only, as Explorer does. A cancelled copy leaves an empty file the notice names. A source with an alternate stream (Mark of the Web) still failed: the drop folder refuses reopening the new file to add the stream, so the file is now kept without it, with its dates and readonly attribute, as Explorer does. Replacing, Undo and rollback still need delete access |
| N6 | Samba / NAS (Synology, NetApp), WSL share | Null or unstable `FileId` refused by identity checks; case sensitivity; exposed Unix symlinks; no alternate streams (already handled) | `\\wsl$` share | to test |
| N7 | Azure Files, high-latency SMB | Timeouts, cancellation that waits on the redirector | — | not simulable |
| N8 | Offline files (CSC), redirected folders | Same item seen through a local and a UNC path, unstable identities | Offline files can be enabled locally, partial | not simulable |
| N9 | OneDrive / SharePoint sync, files online only | Cloud reparse points: refused as reparse objects, or hydration triggered by a preview or a scan | OneDrive on this machine, file set to "online only" (`attrib +U -P`) | **fixed**: hovering a file downloaded it for the tooltip preview, and grid thumbnails read images. A file whose content is not on disk (offline, recall on open or on data access) now gets neither: its icon shows until it is opened. Browsing, folder size, copy, rename, move and Undo, and delete leave such files online only or work on them as Explorer does; opening a preview downloads the file, as expected |
| N10 | SharePoint over WebDAV (`\\site@SSL\DavWWWRoot\…`) | No reliable identity, non-atomic renames, server locks | Any WebDAV server | to test |
| N11 | Third-party drives: `subst`, VeraCrypt, rclone/WinFsp, Google Drive | Missing stable identity or relative opens | `subst X: C:\…`; rclone with WinFsp | `subst` tested: the drive and its target are recognized as the same place, so "destination inside source" is refused through either path, and copying a file onto itself across the two paths creates `name - Copie` instead of replacing it. Move and Undo, rename, new folder, zip and refresh across tabs work. Delete has no Recycle Bin on the drive, as in Explorer. rclone/WinFsp untested (not installed) |
| N12 | Long paths on the server behind a short mapped drive | Canonical path over 260 characters | Share on a folder 105 characters deep, mapped to a drive letter | tested: 310 characters on the server, up to 263 through the drive. Browsing, preview, copy in both directions, move and Undo, rename, new folder, delete, zip compression and extraction, and copy path all work. No change needed |
| N13 | Same server under two identities | Error 1219 at mapping time: not an Oxyde bug, a source of confusing reports | Reproduced while preparing N1 | out of scope |
| N14 | Antivirus, DLP, Defender ASR filters | Refusal of backup-intent or relative opens | — | not simulable |
| N15 | Novell / OES through the NCP client | Third-party redirector, ABE-like "File Scan" rights | — | out of scope |
| N16 | Folders the user may list and read, without "Read permissions" or "Read extended attributes" | `GENERIC_READ` also asks for `READ_CONTROL` and `FILE_READ_EA`: holding parents and opening the source failed with os error 5 (issue #13: `T:\`, `T:\Engineering` and `Dokumentation` all listable) | `icacls <folder> /grant user:(OI)(CI)(RD,X,RA,S)` on the test share, the file itself `(RX)` | **fixed**: same message as the report before, copy succeeds after; those opens ask only `FILE_READ_DATA \| FILE_READ_ATTRIBUTES \| SYNCHRONIZE` |

## Policy

Oxyde is not recommended for company environments: their combinations of servers, permissions and filters cannot all be tested. Fixes stay welcome when a report gives enough detail to reproduce.
