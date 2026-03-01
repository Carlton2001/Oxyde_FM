/**
 * Formatting utilities for the file manager
 */
import { FileEntry, DateFormat } from '../types';

/**
 * Formats a byte size into a human-readable string.
 * @param bytes - Size in bytes
 * @param decimals - Number of decimal places (default: 1)
 * @returns Formatted string (e.g., "1.5 MB")
 */
const SIZE_BASE = 1024;

export const formatSize = (bytes: number, decimals: number = 1, t?: any): string => {
    if (bytes === 0) return `0 ${t ? t('unit_bytes') : 'B'}`;
    const i = Math.floor(Math.log(bytes) / Math.log(SIZE_BASE));
    const size = parseFloat((bytes / Math.pow(SIZE_BASE, i)).toFixed(decimals));

    const units = t
        ? [t('unit_bytes'), t('unit_kb'), t('unit_mb'), t('unit_gb'), t('unit_tb')]
        : ['B', 'KB', 'MB', 'GB', 'TB'];

    // Handle extremely large numbers by capping the unit index
    const unitIndex = Math.min(i, units.length - 1);

    return `${size} ${units[unitIndex]}`;
};

/**
 * Formats a timestamp (milliseconds) into a localized date string.
 * @param ms - Timestamp in milliseconds
 * @param format - Date format setting (default: US)
 * @param fallback - Value to return if ms is 0 or falsy
 * @returns Formatted date string or fallback
 */
export const formatDate = (ms: number, format: DateFormat = 'European', fallback?: string): string => {
    if (!ms && fallback !== undefined) return fallback;
    if (!ms) return '';

    const date = new Date(ms);

    // Helper to pad numbers
    const pad = (n: number) => n.toString().padStart(2, '0');

    const day = pad(date.getDate());
    const month = pad(date.getMonth() + 1);
    const year = date.getFullYear();
    const hours = pad(date.getHours());
    const minutes = pad(date.getMinutes());
    const seconds = pad(date.getSeconds());

    const timeStr = `${hours}:${minutes}:${seconds}`;

    switch (format) {
        case 'US':
            return `${month}/${day}/${year} ${timeStr}`;
        case 'ISO':
            return `${year}-${month}-${day} ${timeStr}`;
        case 'European':
        default:
            return `${day}/${month}/${year} ${timeStr}`;
    }
};

/**
 * Gets a localized string representation of the file type.
 * @param entry - The file entry
 * @param t - Translation function
 * @returns Localized type string (e.g. "PNG File", "Folder")
 */
export const getFileTypeString = (entry: FileEntry, t: any): string => {
    // 1. Network & Virtual paths (High Priority)
    if (entry.path === '__network_vincinity__') {
        return t('network');
    }

    // Windows Shell items (Media devices, UPnP, etc.) or top-level UNC paths
    const isNetworkItem = entry.path.startsWith('::{') || entry.path.startsWith('?') ||
        (entry.path.startsWith('\\\\') && entry.path.split('\\').filter(Boolean).length <= 1) ||
        entry.is_media_device || entry.has_web_page;

    if (isNetworkItem) {
        return t('network');
    }

    if (entry.is_dir) {
        return t('folder');
    }

    const lastDotIndex = entry.name.lastIndexOf('.');
    if (lastDotIndex > 0 && lastDotIndex < entry.name.length - 1) {
        const ext = entry.name.slice(lastDotIndex + 1);
        // If extension contains spaces, it's likely part of the filename (e.g. "file.svg - Shortcut")
        if (!ext.includes(' ')) {
            return ext.toUpperCase();
        }
    }

    return '';
};
