import React, { useState, useEffect } from 'react';
import { Folder, File } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface AsyncFileIconProps {
    path: string;
    isDir: boolean;
    name: string;
    size?: number;
    className?: string;
}

// Memory cache for Blob URLs to avoid re-fetching and re-creating
const blobUrlCache = new Map<string, string>();
const inFlightRequests = new Map<string, Promise<string>>();

const UNIQUE_ICON_EXTENSIONS = new Set([
    'exe', 'ico', 'cur', 'ani', 'lnk', 'url', 'cpl', 'msi', 'msix', 'appx',
    'bat', 'cmd', 'msc', 'ps1', 'scr', 'theme', 'themepack'
]);

/**
 * Identifies folders that likely have custom icons (drives, cloud, system folders)
 */
const isSpecialPath = (path: string): boolean => {
    if (!path) return false;
    // Root drives (C:\) or Shell namespaces
    if (/^[a-zA-Z]:\\?$/.test(path) || path.startsWith('::{') || path.startsWith('?') || path.startsWith('trash://')) return true;

    // Network servers (\\SERVER)
    if (path.startsWith('\\\\')) {
        const parts = path.split('\\').filter(Boolean);
        if (parts.length <= 1) return true;
    }

    const lower = path.toLowerCase();

    // Special subfolders in user profile usually have custom icons
    if (lower.includes('\\users\\')) {
        const parts = lower.split('\\');
        const userIdx = parts.indexOf('users');
        // Check if it's a folder like C:\Users\Name\Documents (3 levels after root)
        if (userIdx !== -1 && parts.length === userIdx + 3) {
            const leaf = parts[parts.length - 1];
            const specialNames = [
                'documents', 'downloads', 'desktop', 'pictures', 'videos', 'music',
                'favorites', 'links', 'onedrive', 'searches', 'contacts', '3d objects',
                'mes documents', 'mes images', 'mes vidéos', 'téléchargements', 'bureau', 'ma musique'
            ];
            if (specialNames.includes(leaf)) return true;
        }
    }

    return false;
};

const getCacheKey = (path: string, name: string, isDir: boolean, size: number) => {
    const sizeStr = size <= 16 ? '32' : '96';
    if (isDir) {
        if (isSpecialPath(path)) return `dir:${path}:${sizeStr}`;
        // Standard folders share the same cache entry
        return `dir:generic:${sizeStr}`;
    }

    const dotIndex = name.lastIndexOf('.');
    const ext = dotIndex !== -1 ? name.slice(dotIndex + 1).toLowerCase() : 'noext';

    // Files with embedded icons (exe, ico, lnk) MUST use their full path
    if (UNIQUE_ICON_EXTENSIONS.has(ext)) return `path:${path}:${sizeStr}`;

    // Standard extensions can share a cache key
    return `ext:${ext}:${sizeStr}`;
};

const getActualRootFontSize = () => {
    if (typeof window === 'undefined') return 16;
    return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
};

export const AsyncFileIcon: React.FC<AsyncFileIconProps> = React.memo(({ path, isDir, name, size = 16, className }) => {
    const cacheKey = getCacheKey(path, name, isDir, size);
    const [iconUrl, setIconUrl] = useState<string | null>(blobUrlCache.get(cacheKey) || null);
    const [error, setError] = useState(false);

    // Sync state if cacheKey changes (component reuse in virtualized lists)
    const [prevCacheKey, setPrevCacheKey] = useState(cacheKey);
    if (cacheKey !== prevCacheKey) {
        setPrevCacheKey(cacheKey);
        setIconUrl(blobUrlCache.get(cacheKey) || null);
        setError(false);
    }

    useEffect(() => {
        if (iconUrl || error) return;

        let isMounted = true;

        const fetchIcon = async () => {
            // Check if already in flight
            if (inFlightRequests.has(cacheKey)) {
                const url = await inFlightRequests.get(cacheKey);
                if (isMounted && url) setIconUrl(url);
                return;
            }

            const promise = (async () => {
                try {
                    const rootFontSize = getActualRootFontSize();
                    const targetPx = (size / 16) * rootFontSize;
                    const sizeStr = targetPx <= 24 ? 'small' : 'large';

                    // NEW SIMPLE LOGIC: Mirror of the folder system
                    let fetchPath = path;
                    const dotIndex = name.lastIndexOf('.');
                    const ext = dotIndex !== -1 ? name.slice(dotIndex + 1).toLowerCase() : 'noext';

                    if (isDir) {
                        if (!isSpecialPath(path)) fetchPath = "oxyde_dir_generic";
                    } else {
                        if (!UNIQUE_ICON_EXTENSIONS.has(ext)) fetchPath = `oxyde_ext_${ext}`;
                    }

                    // The backend now returns Vec<u8> (binary)
                    const bytes = await invoke<number[]>('get_file_icon', { path: fetchPath, size: sizeStr });
                    if (!bytes || bytes.length === 0) throw new Error('Empty icon');

                    const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' });
                    const url = URL.createObjectURL(blob);
                    blobUrlCache.set(cacheKey, url);
                    return url;
                } catch (e) {
                    console.error('Icon fetch failed:', e);
                    throw e;
                }
            })();

            inFlightRequests.set(cacheKey, promise);

            try {
                const url = await promise;
                if (isMounted) setIconUrl(url);
            } catch {
                if (isMounted) setError(true);
            } finally {
                inFlightRequests.delete(cacheKey);
            }
        };

        fetchIcon();

        return () => {
            isMounted = false;
        };
    }, [cacheKey, iconUrl, error, size]);

    const remSize = `${size / 16}rem`;
    const genericDirCacheKey = `dir:generic:${size <= 24 ? '32' : '96'}`;
    const genericFileCacheKey = `ext:generic:${size <= 24 ? '32' : '96'}`;

    const genericDirUrl = isDir ? blobUrlCache.get(genericDirCacheKey) : null;
    const genericFileUrl = !isDir ? blobUrlCache.get(genericFileCacheKey) : null;

    const Fallback = (isDir && genericDirUrl) ? (
        <img
            src={genericDirUrl}
            style={{ width: remSize, height: remSize, objectFit: 'contain', opacity: 0.6 }}
            className={className}
            alt=""
        />
    ) : (!isDir && genericFileUrl) ? (
        <img
            src={genericFileUrl}
            style={{ width: remSize, height: remSize, objectFit: 'contain', opacity: 0.4 }}
            className={className}
            alt=""
        />
    ) : isDir ? (
        <Folder size={remSize} className={className || "text-blue-400"} />
    ) : (
        <File size={remSize} className={className || "text-slate-400"} />
    );

    return (
        <div className="relative flex items-center justify-center shrink-0" style={{ width: remSize, height: remSize }}>
            {(!iconUrl || error) ? (
                Fallback
            ) : (
                <img
                    src={iconUrl}
                    className={`${className || ''}`}
                    style={{
                        width: remSize,
                        height: remSize,
                        objectFit: 'contain'
                    }}
                    alt=""
                    draggable={false}
                    onError={() => setError(true)}
                />
            )}
        </div>
    );
});

export const purgeIconCache = () => {
    blobUrlCache.forEach(url => URL.revokeObjectURL(url));
    blobUrlCache.clear();
    inFlightRequests.clear();
    invoke('purge_icon_cache').catch(() => { });
};

// --- PRE-WARMING ---
// Fetch the generic and common special folder icons immediately so they are ready before any folder is rendered.
const preWarmIcons = async () => {
    try {
        const rootFontSize = getActualRootFontSize();

        // Define common sizes to pre-load
        const sizes = [16, 48]; // 32px and 96px logic

        // 1. Pre-warm Generic Folder (High Priority)
        for (const size of sizes) {
            const targetPx = (size / 16) * rootFontSize;
            const sizeStr = targetPx <= 24 ? 'small' : 'large';
            const cacheKey = `dir:generic:${targetPx <= 24 ? '32' : '96'}`;

            if (!blobUrlCache.has(cacheKey)) {
                invoke<number[]>('get_file_icon', { path: "C:\\Windows", size: sizeStr }).then(bytes => {
                    if (bytes && bytes.length > 0) {
                        const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' });
                        blobUrlCache.set(cacheKey, URL.createObjectURL(blob));
                    }
                }).catch(() => { });
            }
        }

        // 2. Pre-warm Generic File (High Priority)
        for (const size of sizes) {
            const targetPx = (size / 16) * rootFontSize;
            const sizeStr = targetPx <= 24 ? 'small' : 'large';
            const cacheKey = `ext:generic:${targetPx <= 24 ? '32' : '96'}`;

            if (!blobUrlCache.has(cacheKey)) {
                invoke<number[]>('get_file_icon', { path: "oxyde_ext_unknown", size: sizeStr }).then(bytes => {
                    if (bytes && bytes.length > 0) {
                        const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' });
                        blobUrlCache.set(cacheKey, URL.createObjectURL(blob));
                    }
                }).catch(() => { });
            }
        }

        // 3. Pre-warm Common Extensions (Background)
        const commonExts = ['zip', 'pdf', 'txt', 'png', 'jpg', 'mp3', 'wav', 'mp4'];
        for (const ext of commonExts) {
            for (const size of sizes) {
                const targetPx = (size / 16) * rootFontSize;
                const sizeStr = targetPx <= 24 ? 'small' : 'large';
                const cacheKey = `ext:${ext}:${targetPx <= 24 ? '32' : '96'}`;

                if (!blobUrlCache.has(cacheKey)) {
                    invoke<number[]>('get_file_icon', { path: `oxyde_ext_${ext}`, size: sizeStr }).then(bytes => {
                        if (bytes && bytes.length > 0) {
                            const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' });
                            blobUrlCache.set(cacheKey, URL.createObjectURL(blob));
                        }
                    }).catch(() => { });
                }
            }
        }
    } catch (e) {
        console.warn('Pre-warming icons failed:', e);
    }
};

// Initialize pre-warming
if (typeof window !== 'undefined') {
    preWarmIcons();
}

