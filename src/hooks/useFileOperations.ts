import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ConflictEntry, ConflictAction, HistoryState, FileOperation, Transaction, ConflictResponse } from '../types';

export const useFileOperations = (notify?: (message: string, type: 'error' | 'success' | 'info' | 'warning', duration?: number) => void, t?: any) => {
    // Stable refs for notify/t to avoid re-subscribing the event listener on every render
    const notifyRef = useRef(notify);
    const tRef = useRef(t);
    notifyRef.current = notify;
    tRef.current = t;
    // Conflict State
    const [pendingOp, setPendingOp] = useState<{ action: 'copy' | 'move', paths: string[], targetDir: string, turbo: boolean, estimates?: { total_bytes: number, total_files: number, is_cross_volume: boolean, likely_large: boolean } } | null>(null);
    const [conflicts, setConflicts] = useState<ConflictEntry[] | null>(null);
    const [deleteConfirm, setDeleteConfirm] = useState<{ isOpen: boolean, count: number } | null>(null);

    // History State (synced from Rust)
    const [historyState, setHistoryState] = useState<HistoryState>({ undo_stack: [], redo_stack: [] });

    // Operations State
    const [activeOps, setActiveOps] = useState<Map<string, FileOperation>>(new Map());

    // Track completed IDs to prevent race condition where "Completed" event arrives before "invoke" returns
    const completedOpsRef = useRef<Set<string>>(new Set());


    const fetchHistory = useCallback(async () => {
        try {
            const state = await invoke<HistoryState>('get_history');
            setHistoryState(state);
        } catch (e) {
            console.error("Failed to fetch history", e);
        }
    }, []);

    // Initial fetch and Event Listeners
    useEffect(() => {
        fetchHistory();

        // Listen for file operation events
        const unlisten = listen<FileOperation>('file_op_event', (event) => {
            // console.log("File Op Event:", event.payload);
            const op = event.payload;

            // If it's final, mark it as completed so we don't accidentally add it later
            const isFinal = op.status === 'Completed' || op.status === 'Cancelled' || (typeof op.status === 'object' && 'Error' in op.status);
            if (isFinal) {
                completedOpsRef.current.add(op.id);
            }

            setActiveOps(prev => {
                const newMap = new Map(prev);

                if (isFinal) {
                    newMap.delete(op.id);
                } else if (!completedOpsRef.current.has(op.id)) {
                    // Only update if it hasn't already been marked as final
                    newMap.set(op.id, op);
                }

                return newMap;
            });

            if (op.status === 'Completed') {
                fetchHistory();
                const _notify = notifyRef.current;
                const _t = tRef.current;
                if (_notify && _t) {
                    const count = op.total_files || op.sources.length;
                    const itemStr = count > 1 ? _t('items') : _t('item');
                    let msg = "";

                    switch (op.op_type) {
                        case 'Copy':
                            msg = `${count} ${itemStr} ${count > 1 ? _t('pasted_copied_plural') : _t('pasted_copied')}`;
                            break;
                        case 'Move':
                            msg = `${count} ${itemStr} ${count > 1 ? _t('pasted_moved_plural') : _t('pasted_moved')}`;
                            break;
                        case 'Trash':
                            msg = `${count} ${itemStr} ${count > 1 ? _t('moved_to_recycle_bin_plural') : _t('moved_to_recycle_bin')}`;
                            break;
                        case 'Delete':
                            msg = `${count} ${itemStr} ${_t('permanently_deleted')}`;
                            break;
                    }

                    if (msg) _notify(msg, 'success');
                }
            } else if (typeof op.status === 'object' && op.status && 'Error' in op.status) {
                const _notify = notifyRef.current;
                const _t = tRef.current;
                if (_notify && _t) {
                    _notify(`${_t('error')}: ${(op.status as any).Error}`, 'error');
                }
            }
        });

        return () => {
            unlisten.then(f => f());
        };
    }, [fetchHistory]);

    // Safety net: periodically clean stale operations from activeOps
    useEffect(() => {
        const interval = setInterval(() => {
            setActiveOps(prev => {
                let changed = false;
                const next = new Map(prev);
                for (const [id, op] of next) {
                    const isFinal = op.status === 'Completed' || op.status === 'Cancelled' || (typeof op.status === 'object' && 'Error' in op.status);
                    const isStuck = op.total_bytes > 0 && op.processed_bytes >= op.total_bytes;
                    if (isFinal || isStuck) {
                        next.delete(id);
                        completedOpsRef.current.add(id);
                        changed = true;
                    }
                }
                return changed ? next : prev;
            });
        }, 3000);
        return () => clearInterval(interval);
    }, []);

    const executeFileOp = async (action: 'copy' | 'move' | 'trash' | 'delete', paths: string[], targetDir?: string, turbo: boolean = false, initialEstimates?: { total_bytes: number, total_files: number, is_cross_volume: boolean, likely_large: boolean }) => {
        try {
            let opId: string;
            if (action === 'copy' || action === 'move') {
                opId = await invoke<string>(action === 'copy' ? 'copy_items' : 'move_items', {
                    paths,
                    targetDir,
                    turbo,
                    total_size: initialEstimates?.total_bytes,
                    total_files: initialEstimates?.total_files,
                    is_cross_volume: initialEstimates?.is_cross_volume
                });
            } else {
                opId = await invoke<string>(action === 'trash' ? 'delete_items' : 'purge_items', { paths, turbo });
            }

            // Add to active ops immediately with initial state (queued)
            if (completedOpsRef.current.has(opId)) {
                return;
            }

            setActiveOps(prev => {
                if (completedOpsRef.current.has(opId)) {
                    return prev;
                }

                const newMap = new Map(prev);
                newMap.set(opId, {
                    id: opId,
                    op_type: action === 'copy' ? 'Copy' : (action === 'move' ? 'Move' : (action === 'trash' ? 'Trash' : 'Delete')),
                    sources: paths,
                    destination: targetDir,
                    status: 'Queued',
                    total_bytes: initialEstimates?.total_bytes || 0,
                    processed_bytes: 0,
                    total_files: initialEstimates?.total_files || 0,
                    processed_files: 0,
                    bytes_per_second: 0,
                    turbo: turbo,
                    is_cross_volume: initialEstimates?.is_cross_volume || false,
                    likely_large: initialEstimates?.likely_large || false
                });
                return newMap;
            });

        } catch (e) {
            console.error(`Failed to start file operation: ${action}`, e);
            throw e; // Re-throw to caller (e.g. for notifications)
        }
    };

    const initiateFileOp = async (action: 'copy' | 'move', paths: string[], targetDir: string, turbo: boolean = false) => {
        try {
            const response = await invoke<ConflictResponse>('check_conflicts', { paths, targetDir });
            const estimates = {
                total_bytes: response.total_size,
                total_files: response.total_files,
                is_cross_volume: response.is_cross_volume,
                likely_large: response.likely_large
            };

            if (response.conflicts.length > 0) {
                setPendingOp({ action, paths, targetDir, turbo, estimates });
                setConflicts(response.conflicts);
                return false;
            } else {
                await executeFileOp(action, paths, targetDir, turbo, estimates);
                return true;
            }
        } catch (e) {
            console.error("Check conflicts failed", e);
            return false;
        }
    };

    const resolveConflicts = async (resolutions: Map<string, ConflictAction>) => {
        if (!pendingOp) return;
        const op = pendingOp;
        setConflicts(null);
        setPendingOp(null);

        const finalPaths = op.paths.filter(path => resolutions.get(path) !== 'skip');
        if (finalPaths.length > 0) {
            await executeFileOp(op.action, finalPaths, op.targetDir, op.turbo, op.estimates);
        }
    };

    const cancelOp = async (id?: string) => {
        if (id) {
            await invoke('cancel_file_operation', { id });
        } else {
            setPendingOp(null);
            setConflicts(null);
        }
    };

    const deleteItems = async (paths: string[], permanent: boolean = false, turbo: boolean = false) => {
        const isTrashView = paths.some(p => p.toLowerCase().includes('$recycle.bin') || p.toLowerCase().includes('$r'));

        if (isTrashView) {
            // When in trash view, 'delete' means permanent removal from recycle bin
            await invoke('purge_recycle_bin', { paths });
        } else {
            // Normal view: move to trash or permanent delete
            await executeFileOp(permanent ? 'delete' : 'trash', paths, undefined, turbo);
        }
        await fetchHistory();
    };

    const renameItem = async (oldPath: string, newPath: string) => {
        await invoke('rename_item', { oldPath, newPath });
        await fetchHistory();
    };

    const createFolder = async (path: string) => {
        await invoke('create_dir', { path });
        await fetchHistory();
    };

    const undo = async () => {
        try {
            const tx = await invoke<Transaction | null>('undo_last_action');
            await fetchHistory();
            return tx;
        } catch (e) {
            console.error("Undo failed", e);
        }
    };

    const redo = async () => {
        try {
            const tx = await invoke<Transaction | null>('redo_last_action');
            await fetchHistory();
            return tx;
        } catch (e) {
            console.error("Redo failed", e);
        }
    };

    const canUndo = historyState.undo_stack.length > 0;
    const canRedo = historyState.redo_stack.length > 0;
    const isExecuting = activeOps.size > 0;

    // Derived progress for UI (if we want to show a single progress bar for the "main" op)
    // We can expose activeOps directly for detailed UI
    const activeOperation = activeOps.values().next().value; // Access first value

    return {
        // State
        pendingOp,
        conflicts,
        deleteConfirm,
        isExecuting,
        activeOps, // Expose full map
        activeOperation, // Expose one for simple UIs
        canUndo,
        canRedo,
        historyState,

        // Actions
        initiateFileOp,
        resolveConflicts,
        cancelOp,
        deleteItems,
        renameItem,
        createFolder,
        setDeleteConfirm,
        setConflicts,
        setPendingOp,
        undo,
        redo
    };
};
