import { useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { formatCommandError } from '../utils/error';
import { FileEntry, DriveInfo, SortConfig, DirResponse, FileSummary, DirBatchEvent, PanelId } from '../types';
import { getParent, isVirtualPath } from '../utils/path';
import { listen } from '@tauri-apps/api/event';

export const useDrives = () => {
    const [drives, setDrives] = useState<DriveInfo[]>([]);
    const [mountedImages, setMountedImages] = useState<string[]>([]);

    const refreshDrives = useCallback(async () => {
        try {
            // 1. Fetch BASIC drive info (FAST, no hardware detection)
            const basicDrives = await invoke<DriveInfo[]>('get_drives', { skipHardwareInfo: true });
            setDrives(prev => {
                if (prev.length === basicDrives.length && prev.every((d, i) => {
                    const n = basicDrives[i];
                    return d.path === n.path && d.label === n.label && d.drive_type === n.drive_type;
                })) {
                    return prev;
                }
                return basicDrives;
            });

            // 2. Fetch MOUNTED IMAGES (Medium, runs PowerShell)
            invoke<string[]>('get_mounted_images').then(newMountedImages => {
                setMountedImages(prev => {
                    const normPrev = prev.map(p => p.toLowerCase()).sort();
                    const normNew = newMountedImages.map((p: string) => p.toLowerCase()).sort();
                    if (normPrev.length === normNew.length && normPrev.every((val, index) => val === normNew[index])) {
                        return prev;
                    }
                    return newMountedImages;
                });
            }).catch(console.error);

            // 3. Fetch ENRICHED drive info (SLOWER, hardware detection/spin-up)
            invoke<DriveInfo[]>('get_drives', { skipHardwareInfo: false }).then(enrichedDrives => {
                setDrives(prev => {
                    // Check if anything actually changed (like media_type or physical_id)
                    const changed = enrichedDrives.some((d, i) => {
                        const p = prev[i];
                        return !p || d.media_type !== p.media_type || d.physical_id !== p.physical_id;
                    });
                    if (!changed) return prev;
                    return enrichedDrives;
                });
            }).catch(console.error);

        } catch (err) {
            console.error("Failed to refresh drives:", err);
        }
    }, []);

    useEffect(() => {
        refreshDrives();

        const unlisten = listen('drives-changed', refreshDrives);

        return () => {
            unlisten.then(f => f());
        };
    }, [refreshDrives]);

    return { drives, mountedImages, refreshDrives };
};

export const getSortedFiles = (files: FileEntry[], config: SortConfig) => {
    return [...files].sort((a, b) => {
        // 1. Folders usually first. 
        // Exception: when sorting by size, we mix them ONLY IF the folders involved have been calculated.
        const isSizeSort = config.field === 'size';
        const aUncalc = a.is_dir && !a.is_calculated;
        const bUncalc = b.is_dir && !b.is_calculated;

        if (!isSizeSort || (aUncalc || bUncalc)) {
            if (a.is_dir && !b.is_dir) return -1;
            if (!a.is_dir && b.is_dir) return 1;
        }

        // 2. Prepare aspects
        let aspectA: any;
        let aspectB: any;

        if (config.field === 'type') {
            // All folders have same "type" to group them and let tie-breaker handle name sort
            aspectA = a.is_dir ? '' : (a.name.split('.').pop() || '').toLowerCase();
            aspectB = b.is_dir ? '' : (b.name.split('.').pop() || '').toLowerCase();
        } else if (config.field === 'date') {
            aspectA = a.modified;
            aspectB = b.modified;
        } else if (config.field === 'deletedDate') {
            // Trash-specific: sort by deletion time
            aspectA = a.deleted_time || 0;
            aspectB = b.deleted_time || 0;
        } else if (config.field === 'location') {
            // For trash items, use original_path; for search results, use parent
            aspectA = (a.original_path || getParent(a.path) || '').toLowerCase();
            aspectB = (b.original_path || getParent(b.path) || '').toLowerCase();
        } else {
            aspectA = a[config.field as keyof FileEntry];
            aspectB = b[config.field as keyof FileEntry];
        }

        // 3. Compare aspects if field is NOT name
        if (config.field !== 'name' && aspectA !== aspectB) {
            if (aspectA < aspectB) return config.direction === 'asc' ? -1 : 1;
            if (aspectA > aspectB) return config.direction === 'asc' ? 1 : -1;
        }

        // 4. Tie-breaker or Name Sort (Natural & Case-Insensitive)
        const nameCmp = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });

        return config.direction === 'asc' ? nameCmp : -nameCmp;
    });
};

export const useFiles = (panelId: PanelId, path: string, sortConfig: SortConfig, showHidden: boolean, showSystem: boolean) => {
    const [files, setFiles] = useState<FileEntry[]>([]);
    const [summary, setSummary] = useState<FileSummary | null>(null);
    const [isComplete, setIsComplete] = useState(true);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const prevParamsRef = useRef({ path, sortConfig, showHidden, showSystem });
    // Track current path for real-time event filtering (prevents stale closure issues)
    const currentPathRef = useRef(path);
    currentPathRef.current = path;

    // Internal reconciliation function
    const reconcileFiles = (oldFiles: FileEntry[], newFiles: FileEntry[]): FileEntry[] => {
        if (oldFiles.length === 0) return newFiles;

        const oldMap = new Map(oldFiles.map(f => [f.path, f]));
        let hasChanges = false;
        let orderChanged = false;

        if (oldFiles.length !== newFiles.length) {
            hasChanges = true;
        }

        const merged = newFiles.map((newFile, idx) => {
            const oldFile = oldMap.get(newFile.path);

            if (!orderChanged && oldFiles[idx]?.path !== newFile.path) {
                orderChanged = true;
            }

            if (oldFile) {
                let finalSize = newFile.size;
                let finalCalculated = newFile.is_calculated;

                // Priority: preserve locally calculated sizes for directories
                if (newFile.is_dir && !newFile.is_calculated && oldFile.is_calculated) {
                    finalSize = oldFile.size;
                    finalCalculated = true;
                }

                if (finalSize === oldFile.size &&
                    finalCalculated === oldFile.is_calculated &&
                    oldFile.modified === newFile.modified &&
                    oldFile.is_dir === newFile.is_dir &&
                    oldFile.is_hidden === newFile.is_hidden &&
                    oldFile.is_system === newFile.is_system
                ) {
                    return oldFile;
                }
                hasChanges = true;
                return { ...newFile, size: finalSize, is_calculated: finalCalculated };
            }
            hasChanges = true;
            return newFile;
        });

        if (!hasChanges && !orderChanged) return oldFiles;
        return merged;
    };

    // Listen for streamed results with buffering
    useEffect(() => {
        let buffer: FileEntry[] = [];
        let timeout: any = null;
        let idleCallbackId: number | null = null;
        let isCleanedUp = false;

        // Use requestIdleCallback when available for non-blocking UI updates
        const scheduleUpdate = (callback: () => void) => {
            if (isCleanedUp) return;
            if ('requestIdleCallback' in window) {
                idleCallbackId = (window as any).requestIdleCallback(callback, { timeout: 50 });
            } else {
                callback();
            }
        };

        const flush = () => {
            if (isCleanedUp) return;
            if (buffer.length > 0) {
                const batchToApply = [...buffer];
                buffer = [];
                scheduleUpdate(() => {
                    if (isCleanedUp) return;
                    setFiles(prev => [...prev, ...batchToApply]);
                });
            }
            timeout = null;
        };

        const unlisten = listen<DirBatchEvent>('dir_batch', (event) => {
            if (isCleanedUp) return;
            const batch = event.payload;
            // Use currentPathRef to check against CURRENT path, not stale closure path
            // This prevents memory accumulation from events that arrive after navigation
            if (batch.panel_id === panelId && batch.path === currentPathRef.current) {
                buffer.push(...batch.entries);

                if (batch.is_complete) {
                    if (timeout) clearTimeout(timeout);
                    flush();
                    setIsComplete(true);
                } else if (!timeout) {
                    // Increased from 150ms to 200ms for smoother UI
                    timeout = setTimeout(flush, 200);
                }
            }
        });

        return () => {
            isCleanedUp = true;
            unlisten.then(fn => fn());
            if (timeout) clearTimeout(timeout);
            if (idleCallbackId && 'cancelIdleCallback' in window) {
                (window as any).cancelIdleCallback(idleCallbackId);
            }
            // Clear buffer to free memory
            buffer = [];
        };
    }, [panelId, path, sortConfig, showHidden, showSystem]);

    const refresh = useCallback(async (silent: boolean = false) => {
        if (!path) return;

        const prevParams = prevParamsRef.current;
        const pathOrFiltersChanged = path !== prevParams.path ||
            showHidden !== prevParams.showHidden ||
            showSystem !== prevParams.showSystem;

        const sortChanged = sortConfig.field !== prevParams.sortConfig.field ||
            sortConfig.direction !== prevParams.sortConfig.direction;

        if (pathOrFiltersChanged) {
            setFiles([]);
            setSummary(null);
            prevParamsRef.current = { path, sortConfig, showHidden, showSystem };
        } else if (sortChanged) {
            // Just update ref, don't clear files. Sorting will be handled by useMemo.
            prevParamsRef.current = { path, sortConfig, showHidden, showSystem };
            return; // Skip re-fetch when only sort changed to preserve local state (like folder sizes)
        }

        if (!silent) setLoading(true);
        setError(null);
        try {
            if (path.startsWith('trash://') || path.startsWith('trash:\\\\')) {
                const entries = await invoke<FileEntry[]>('list_trash');
                setFiles(entries);
                setIsComplete(true);
                setSummary(null);
            } else if (path === '__network_vincinity__' || (path.startsWith('\\\\') && path.split('\\').filter(Boolean).length === 1)) {
                const networkPath = path === '__network_vincinity__' ? undefined : path;
                const netResources = await invoke<any[]>('get_network_resources', { path: networkPath });

                const entries: FileEntry[] = netResources.map(r => ({
                    name: r.name,
                    path: r.remote_path,
                    is_dir: !r.is_media_device, // If it's a media device, don't treat it as a folder to navigate into
                    size: 0,
                    modified: 0,
                    readonly: true,
                    is_hidden: false,
                    is_system: false,
                    is_calculating: false,
                    is_calculated: false,
                    drive_type: r.resource_type === 1 ? 'remote' : undefined, // RESOURCETYPE_DISK
                    is_media_device: r.is_media_device,
                    has_web_page: r.has_web_page
                } as FileEntry));

                setFiles(entries);
                setIsComplete(true);
                setSummary(null);
            } else if (isVirtualPath(path)) {
                setFiles([]);
                setIsComplete(true);
            } else {
                const response = await invoke<DirResponse>('list_dir', {
                    panelId,
                    path,
                    sortConfig,
                    showHidden,
                    showSystem,
                    forceRefresh: true // Always force refresh to avoid stale cache issues
                });

                // console.log('[useFiles] list_dir response', response.entries.length, 'entries');

                if (response.is_complete) {
                    setFiles(prev => {
                        const next = reconcileFiles(prev, response.entries);
                        return next;
                    });
                } else {
                    setFiles(prev => reconcileFiles(prev, response.entries));
                }

                setSummary(response.summary);
                setIsComplete(response.is_complete);
            }
        } catch (err) {
            const errStr = formatCommandError(err);
            if (path !== prevParamsRef.current.path) return;

            const isUnmountError = (errStr.includes('os error 3') || errStr.toLowerCase().includes('not found'));
            if (isUnmountError) {
                setFiles([]);
                setError(null);
                return;
            }

            console.error(`Failed to list dir: ${path}`, err);
            setError(errStr);
            setFiles([]);
        } finally {
            if (!silent) setLoading(false);
        }
    }, [panelId, path, sortConfig, showHidden, showSystem]);

    // Independent fs-change listener to ensuring auto-refresh
    useEffect(() => {
        const unlisten = listen('fs-change', (event: any) => {
            const { payload } = event;
            // payload might be { watcher_id, path, kind } OR { paths: [] } depending on backend implementation.

            let paths: string[] = [];
            if (payload.paths && Array.isArray(payload.paths)) {
                paths = payload.paths;
            } else if (payload.path && typeof payload.path === 'string') {
                paths = [payload.path];
            } else if (typeof payload === 'string') {
                paths = [];
            } else if (typeof payload === 'object' && payload.path) {
                paths = [payload.path];
            }

            const relevant = paths.some(p => {
                if (!path) return false;
                const normPath = path.toLowerCase().replace(/\\/g, '/').replace(/\/$/, '');
                const normP = p.toLowerCase().replace(/\\/g, '/');
                return normP.startsWith(normPath);
            });

            if (relevant) {
                refresh(true);
            }
        });

        return () => { unlisten.then(u => u()); };
    }, [path, refresh]);

    useEffect(() => {
        refresh(false);
    }, [refresh]);

    // Trash polling for real-time updates (virtual paths can't be watched natively)
    useEffect(() => {
        if (path === 'trash://') {
            const interval = setInterval(() => {
                refresh(true); // Silent refresh
            }, 3000);
            return () => clearInterval(interval);
        }
    }, [path, refresh]);

    const sortedFiles = useMemo(() => {
        // Always use frontend sorting to ensure consistency with local state (calculated sizes)
        return getSortedFiles(files, sortConfig);
    }, [files, sortConfig]);

    const updateFileSize = useCallback((p: string, size: number) => {
        setFiles(prev => prev.map(f => f.path === p ? { ...f, size, is_calculated: true, is_calculating: false } : f));
    }, []);

    const setFileCalculating = useCallback((p: string, isCalculating: boolean) => {
        setFiles(prev => prev.map(f => f.path === p ? { ...f, is_calculating: isCalculating } : f));
    }, []);

    return { files, sortedFiles, summary, isComplete, loading, error, refresh, updateFileSize, setFileCalculating };
};
