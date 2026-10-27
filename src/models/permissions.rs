//! Unix permissions as Nautilus presents them: an access level for the owner, the
//! group and others, plus the execution bits of a file. Only the access bits of the
//! chosen class change; the other bits, special ones included, are kept.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: u32,
    pub name: String,
}

/// What the Permissions tab reads, and what an applied change must still find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnixPermissions {
    pub device: u64,
    pub inode: u64,
    /// Permission and special bits, without the file type.
    pub mode: u32,
    pub is_dir: bool,
    pub owner: Account,
    pub group: Account,
    /// Groups the user may give the item, its current group included: all of them
    /// for root, as Nautilus offers an administrator.
    pub groups: Vec<Account>,
    /// Accounts root may give the item, labelled « login – Real Name » as Nautilus
    /// does; empty for any other user, who cannot change the owner.
    pub owners: Vec<Account>,
    /// The current user owns the item or is root.
    pub editable: bool,
}

/// A change requested from the Permissions tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionChange {
    pub expected: UnixPermissions,
    pub mode: u32,
    pub owner: u32,
    pub group: u32,
    /// For a folder: levels for everything inside it, applied before its own change.
    pub enclosed: Levels,
}

impl PermissionChange {
    /// The item as it is: nothing to apply yet.
    pub fn of(expected: UnixPermissions) -> Self {
        Self {
            mode: expected.mode,
            owner: expected.owner.id,
            group: expected.group.id,
            enclosed: Levels::default(),
            expected,
        }
    }

    pub fn is_empty(&self) -> bool {
        (self.mode, self.owner, self.group) == (self.expected.mode, self.expected.owner.id, self.expected.group.id)
            && self.enclosed.is_empty()
    }
}

/// Levels per class for files and for folders: those of the items inside a folder, as
/// Nautilus's « Change Permissions for Enclosed Files », or those of a selection.
/// None keeps a class as each item has it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Levels {
    pub files: [Option<Access>; 3],
    pub folders: [Option<Access>; 3],
}

impl Levels {
    pub fn is_empty(&self) -> bool {
        self.files.iter().chain(&self.folders).all(Option::is_none)
    }

    /// What Nautilus proposes for enclosed items: the levels of a new file (`rw-` for
    /// each class) and of a new folder (`rwx`) under this umask. A class whose bits
    /// match no level stays unchanged.
    pub fn from_umask(umask: u32) -> Self {
        let levels = |is_dir: bool| {
            let mode = if is_dir { 0o777 } else { 0o666 } & !umask;
            PermissionClass::ALL.map(|class| Access::of(mode, is_dir, class))
        };
        Self { files: levels(false), folders: levels(true) }
    }

    /// The mode an item receives.
    pub fn mode(&self, mode: u32, is_dir: bool) -> u32 {
        let levels = if is_dir { &self.folders } else { &self.files };
        PermissionClass::ALL
            .into_iter()
            .zip(levels)
            .fold(mode, |mode, (class, level)| level.map_or(mode, |level| level.apply(mode, is_dir, class)))
    }
}

/// Why a change was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionError {
    /// The item is another object, or its permissions or group changed since loading.
    Changed,
    /// The file system accepted the change without keeping it, as ntfs-3g and FAT do.
    NotKept,
    /// The system refused or failed the change; nothing else was attempted after it.
    Failed(String),
    /// Some enclosed items kept their permissions; the others and the folder changed.
    Incomplete { failed: u64, first: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionClass {
    Owner,
    Group,
    Others,
}

impl PermissionClass {
    pub const ALL: [Self; 3] = [Self::Owner, Self::Group, Self::Others];

    pub fn index(self) -> usize {
        match self {
            Self::Owner => 0,
            Self::Group => 1,
            Self::Others => 2,
        }
    }

    fn shift(self) -> u32 {
        match self {
            Self::Owner => 6,
            Self::Group => 3,
            Self::Others => 0,
        }
    }
}

/// An access level; files and folders offer different ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    None,
    /// Files only.
    ReadOnly,
    /// Files only.
    ReadWrite,
    /// Folders only: `r--`.
    ListFiles,
    /// Folders only: `r-x`.
    AccessFiles,
    /// Folders only: `rwx`.
    CreateDelete,
}

const READ: u32 = 4;
const WRITE: u32 = 2;
const EXECUTE: u32 = 1;

impl Access {
    /// The levels offered for a class. As in Nautilus, the owner is never offered None.
    pub fn choices(is_dir: bool, class: PermissionClass) -> &'static [Access] {
        match (is_dir, class) {
            (true, PermissionClass::Owner) => &[Self::ListFiles, Self::AccessFiles, Self::CreateDelete],
            (true, _) => &[Self::None, Self::ListFiles, Self::AccessFiles, Self::CreateDelete],
            (false, PermissionClass::Owner) => &[Self::ReadOnly, Self::ReadWrite],
            (false, _) => &[Self::None, Self::ReadOnly, Self::ReadWrite],
        }
    }

    /// The level shown for these bits, or None for a combination outside the levels.
    /// A file's execution bit is not part of its level.
    pub fn of(mode: u32, is_dir: bool, class: PermissionClass) -> Option<Access> {
        let bits = (mode >> class.shift()) & 0o7;
        let level = if is_dir {
            match bits {
                0 => Self::None,
                READ => Self::ListFiles,
                b if b == READ | EXECUTE => Self::AccessFiles,
                0o7 => Self::CreateDelete,
                _ => return None,
            }
        } else {
            match bits & !EXECUTE {
                0 => Self::None,
                READ => Self::ReadOnly,
                b if b == READ | WRITE => Self::ReadWrite,
                _ => return None,
            }
        };
        Self::choices(is_dir, class).contains(&level).then_some(level)
    }

    /// The mode with this level for the class. A file keeps its execution bit.
    pub fn apply(self, mode: u32, is_dir: bool, class: PermissionClass) -> u32 {
        let shift = class.shift();
        let kept = if is_dir { 0 } else { (mode >> shift) & EXECUTE };
        let bits = match self {
            Self::None => 0,
            Self::ReadOnly | Self::ListFiles => READ,
            Self::ReadWrite => READ | WRITE,
            Self::AccessFiles => READ | EXECUTE,
            Self::CreateDelete => READ | WRITE | EXECUTE,
        };
        (mode & !(0o7 << shift)) | ((bits | kept) << shift)
    }
}

/// Whether a file runs as a program: its owner may execute it.
pub fn executable(mode: u32) -> bool {
    mode & 0o100 != 0
}

/// Nautilus sets or clears the three execution bits together.
pub fn with_executable(mode: u32, executable: bool) -> u32 {
    if executable { mode | 0o111 } else { mode & !0o111 }
}

/// The class bits all these modes share, as one of them; None when they differ.
/// A file's execution bit is not part of its level, so it is not compared.
pub fn shared(modes: impl IntoIterator<Item = u32>, is_dir: bool, class: PermissionClass) -> Option<u32> {
    let mask = if is_dir { 0o7 } else { READ | WRITE } << class.shift();
    let mut modes = modes.into_iter();
    let first = modes.next()?;
    modes.all(|mode| mode & mask == first & mask).then_some(first)
}

/// `rwx` text of a class, for a combination outside the levels.
pub fn symbolic(mode: u32, class: PermissionClass) -> String {
    let bits = (mode >> class.shift()) & 0o7;
    [(READ, 'r'), (WRITE, 'w'), (EXECUTE, 'x')]
        .into_iter()
        .map(|(bit, letter)| if bits & bit != 0 { letter } else { '-' })
        .collect()
}

#[cfg(test)]
#[path = "permissions/tests/access.rs"]
mod tests;
