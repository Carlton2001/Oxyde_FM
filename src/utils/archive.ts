export const ARCHIVE_EXTENSIONS = [
    'zip', '7z', 'rar', 'tar', 'gz', 'tgz', 'xz', 'txz', 'zst', 'tzst', 'bz2', 'tbz2'
];

export const isArchivePath = (path: string): boolean => {
    if (!path) return false;
    const parts = path.split('.');
    if (parts.length <= 1) return false;
    const ext = parts.pop()?.toLowerCase();
    if (!ext) return false;

    // Check for double extensions like .tar.gz
    if (ext === 'gz' || ext === 'xz' || ext === 'zst' || ext === 'bz2') {
        const prevExt = parts.pop()?.toLowerCase();
        if (prevExt === 'tar') return true;
        // If not tar, it's still an archive (.gz, etc)
        return true;
    }

    return ARCHIVE_EXTENSIONS.includes(ext);
};

export const isSupportedArchiveForAdding = (path: string): boolean => {
    if (!path) return false;
    const ext = path.split('.').pop()?.toLowerCase();
    return ext === 'zip';
};
