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

const UNIQUE_ICON_EXTENSIONS = new Set(['exe', 'ico', 'cur', 'ani', 'lnk', 'url', 'cpl', 'msi', 'msix', 'appx']);

const getCacheKey = (path: string, name: string, isDir: boolean, size: number) => {
    const sizeStr = size <= 16 ? '32' : '96';
    if (isDir) return `dir:${path}:${sizeStr}`;

    const dotIndex = name.lastIndexOf('.');
    const ext = dotIndex !== -1 ? name.slice(dotIndex + 1).toLowerCase() : 'noext';

    if (UNIQUE_ICON_EXTENSIONS.has(ext)) return `path:${path}:${sizeStr}`;
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
        let timeout: ReturnType<typeof setTimeout>;

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
                    // The backend now returns Vec<u8> (binary)
                    const bytes = await invoke<number[]>('get_file_icon', { path, size: sizeStr });
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

        timeout = setTimeout(fetchIcon, 30);

        return () => {
            isMounted = false;
            clearTimeout(timeout);
        };
    }, [cacheKey, iconUrl, error, path, size]);

    const Fallback = isDir ?
        <Folder size={`${size / 16}rem`} className={className || "text-blue-400"} /> :
        <File size={`${size / 16}rem`} className={className || "text-slate-400"} />;

    const remSize = `${size / 16}rem`;

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

