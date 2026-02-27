import React, { useState, useEffect, useRef, useCallback } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { IMAGE_EXTENSIONS } from '../../utils/fileIcons';

interface ThumbnailProps {
    path: string;
    name: string;
    isDir: boolean;
    fallback: React.ReactNode;
}

// Memory cache: path → convertFileSrc URL of cached thumbnail
const THUMB_CACHE = new Map<string, string>();

// ─── Concurrency limiter for backend thumbnail generation ───────────
const MAX_CONCURRENT = 4;
let running = 0;
const queue: Array<() => void> = [];

function enqueue(): Promise<void> {
    if (running < MAX_CONCURRENT) {
        running++;
        return Promise.resolve();
    }
    return new Promise(resolve => queue.push(resolve));
}

function dequeue() {
    running--;
    if (queue.length > 0) {
        running++;
        const next = queue.shift()!;
        next();
    }
}
// ────────────────────────────────────────────────────────────────────

/**
 * Thumbnail strategy:
 * 1. If cached thumbnail exists in memory → use it instantly
 * 2. Otherwise → show original via convertFileSrc (instant, browser-resized)
 * 3. In background → generate a Rust-cached thumbnail
 * 4. If original image fails to load → wait for the cached thumbnail to replace it
 */
export const Thumbnail: React.FC<ThumbnailProps> = React.memo(({ path, name, isDir, fallback }) => {
    const [src, setSrc] = useState<string | null>(null);
    const [showFallback, setShowFallback] = useState(false);
    const mountedRef = useRef(true);
    const cachedThumbRef = useRef<string | null>(null);

    useEffect(() => {
        mountedRef.current = true;
        setSrc(null);
        setShowFallback(false);
        cachedThumbRef.current = null;

        if (isDir) return;

        const ext = name.split('.').pop()?.toLowerCase() || '';
        if (!IMAGE_EXTENSIONS.includes(ext)) return;

        // 1) If we already have a cached thumbnail from a previous load, use it instantly
        if (THUMB_CACHE.has(path)) {
            setSrc(THUMB_CACHE.get(path)!);
            return;
        }

        // 2) Show the original image immediately (browser handles resize via CSS)
        setSrc(convertFileSrc(path));

        // 3) In background, generate a lightweight cached thumbnail
        let released = false;

        const generateThumb = async () => {
            await enqueue();
            if (!mountedRef.current) {
                if (!released) { released = true; dequeue(); }
                return;
            }
            try {
                const cachedPath = await invoke<string>('get_image_thumbnail', { path });
                const finalSrc = convertFileSrc(cachedPath);
                THUMB_CACHE.set(path, finalSrc);
                cachedThumbRef.current = finalSrc;
                if (mountedRef.current) {
                    // Swap to lighter cached version
                    setSrc(finalSrc);
                    setShowFallback(false);
                }
            } catch {
                // If Rust generation fails too, keep showing original or fallback
            } finally {
                if (!released) { released = true; dequeue(); }
            }
        };

        generateThumb();

        return () => {
            mountedRef.current = false;
        };
    }, [path, name, isDir]);

    // 4) If the original image fails to load, try the cached thumbnail
    const handleError = useCallback(() => {
        if (cachedThumbRef.current) {
            // Cached thumbnail is already ready, use it
            setSrc(cachedThumbRef.current);
        } else {
            // Cached thumbnail not ready yet — show fallback temporarily.
            // The background task will call setSrc when it's done.
            setShowFallback(true);
        }
    }, []);

    if (!src || showFallback) return <>{fallback}</>;

    return (
        <img
            src={src}
            alt=""
            className="thumbnail-img"
            style={{
                width: '100%',
                height: '100%',
                objectFit: 'contain',
                display: 'block'
            }}
            draggable={false}
            onError={handleError}
        />
    );
});
