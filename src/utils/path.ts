/**
 * Path utilities for the file manager
 */

/**
 * Returns the parent directory path of the given path.
 * Returns null for root drives (e.g., "C:\\").
 */
export const getParent = (path: string): string | null => {
    if (!path) return null;
    if (path.endsWith(":\\")) return null;
    const parts = path.split("\\");
    if (parts.length <= 1) return null;
    parts.pop();
    let parent = parts.join("\\");
    if (parent.endsWith(":")) parent += "\\";
    return parent || null;
};

/**
 * Extracts the drive letter from a Windows path (e.g., "C:")
 */
export const getDrive = (path: string): string | null => {
    if (!path) return null;
    const match = path.match(/^([a-zA-Z]:)/);
    return match ? match[1].toUpperCase() : null;
};

/**
 * Checks if two paths are on the same volume (Windows drive)
 */
export const isSameVolume = (path1: string, path2: string): boolean => {
    const drive1 = getDrive(path1);
    const drive2 = getDrive(path2);
    return drive1 !== null && drive1 === drive2;
};

/**
 * Normalizes a Windows path for consistent comparison.
 * Replaces forward slashes with backslashes and ensures drive roots end with a backslash.
 */
export const normalizePath = (path: string): string => {
    if (!path) return "";
    // Standardize slashes
    let normalized = path.replace(/\//g, "\\");

    // Ensure drive letter is uppercase (C:\ instead of c:\)
    if (normalized.match(/^[a-z]:/i)) {
        normalized = normalized.charAt(0).toUpperCase() + normalized.slice(1);
    }

    // Ensure drive root (C:) becomes (C:\)
    if (normalized.match(/^[a-zA-Z]:$/)) {
        normalized += "\\";
    }

    // Remove trailing backslash unless it's a drive root (e.g., C:\)
    if (normalized.length > 3 && normalized.endsWith("\\")) {
        normalized = normalized.slice(0, -1);
    }
    return normalized;
};
