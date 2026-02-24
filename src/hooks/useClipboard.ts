import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface ClipboardState {
    paths: string[];
    action: 'copy' | 'cut';
}

export const useClipboard = () => {
    const [clipboard, setClipboard] = useState<ClipboardState | null>(null);

    // Sync from system clipboard on mount and focus
    const refreshClipboard = useCallback(async () => {
        try {
            const result = await invoke<[string[], boolean]>('get_clipboard_files');
            if (result && Array.isArray(result)) {
                const [paths, isCut] = result;
                if (paths && paths.length > 0) {
                    setClipboard({ paths, action: isCut ? 'cut' : 'copy' });
                    return;
                }
            }
            setClipboard(null);
        } catch (error) {
            console.error('Failed to sync system clipboard:', error);
            setClipboard(null);
        }
    }, []);

    useEffect(() => {
        refreshClipboard();
        window.addEventListener('focus', refreshClipboard);
        return () => window.removeEventListener('focus', refreshClipboard);
    }, [refreshClipboard]);

    const copy = useCallback(async (paths: string[]) => {
        try {
            // Using snake_case for Rust compatibility
            await invoke('set_clipboard_files', { paths, is_cut: false });
            setClipboard({ paths, action: 'copy' });
        } catch (error) {
            console.error('Failed to copy to system clipboard:', error);
        }
    }, []);

    const cut = useCallback(async (paths: string[]) => {
        try {
            // Using snake_case for Rust compatibility
            await invoke('set_clipboard_files', { paths, is_cut: true });
            setClipboard({ paths, action: 'cut' });
        } catch (error) {
            console.error('Failed to cut to system clipboard:', error);
        }
    }, []);

    const clearClipboard = useCallback(async () => {
        try {
            await invoke('set_clipboard_files', { paths: [], is_cut: false });
            setClipboard(null);
        } catch (error) {
            console.error('Failed to clear clipboard:', error);
        }
    }, []);

    const copyToSystem = useCallback(async (text: string) => {
        try {
            await navigator.clipboard.writeText(text);
            return true;
        } catch (error) {
            console.error('Failed to copy text to system clipboard:', error);
            return false;
        }
    }, []);

    return { clipboard, copy, cut, clearClipboard, copyToSystem, refreshClipboard };
};
