import React, { useState, useEffect } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { IMAGE_EXTENSIONS } from '../../utils/fileIcons';

interface ThumbnailProps {
    path: string;
    name: string;
    isDir: boolean;
    fallback: React.ReactNode;
}

// Memory cache to avoid repeated invokes for the same session
const THUMB_CACHE = new Map<string, string>();

/**
 * A component that displays a file thumbnail for images,
 * falling back to a provided node (usually an icon) if loading fails
 * or if the file type is not supported for thumbnails.
 */
export const Thumbnail: React.FC<ThumbnailProps> = React.memo(({ path, name, isDir, fallback }) => {
    const [src, setSrc] = useState<string | null>(null);
    const [error, setError] = useState(false);

    useEffect(() => {
        if (isDir) {
            setSrc(null);
            setError(false);
            return;
        }

        const ext = name.split('.').pop()?.toLowerCase() || '';
        if (IMAGE_EXTENSIONS.includes(ext)) {
            if (THUMB_CACHE.has(path)) {
                setSrc(THUMB_CACHE.get(path)!);
                setError(false);
                return;
            }

            let isMounted = true;
            const loadThumb = async () => {
                try {
                    // Call the Rust backend to get a resized cached version
                    const cachedPath = await invoke<string>('get_image_thumbnail', { path });
                    if (isMounted) {
                        const finalSrc = convertFileSrc(cachedPath);
                        THUMB_CACHE.set(path, finalSrc);
                        setSrc(finalSrc);
                    }
                } catch (err) {
                    // Fallback to original if thumbnail generation fails
                    if (isMounted) {
                        setSrc(convertFileSrc(path));
                    }
                }
            };

            loadThumb();
            return () => { isMounted = false; };
        } else {
            setSrc(null);
            setError(false);
        }
    }, [path, name, isDir]);

    if (!src || error) return <>{fallback}</>;

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
            onError={() => setError(true)}
        />
    );
});

