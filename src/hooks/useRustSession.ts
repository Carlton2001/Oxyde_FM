import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PanelId } from '../types';

export interface Tab {
    id: string;
    path: string;
    version: number;
}

export interface PanelState {
    tabs: Tab[];
    active_tab_id: string;
}

export interface SessionState {
    panels: Record<string, PanelState>;
    active_panel_id: string;
}

export const useRustSession = () => {
    const [session, setSession] = useState<SessionState | null>(null);
    const [isLoading, setIsLoading] = useState(true);

    const refreshSession = useCallback(async () => {
        try {
            const current = await invoke<SessionState>('get_session_state');
            setSession(current);
        } catch (e) {
            console.error("Failed to fetch session state:", e);
        } finally {
            setIsLoading(false);
        }
    }, []);

    useEffect(() => {
        refreshSession();

        const unlisten = listen<SessionState>('session_changed', (event) => {
            setSession(event.payload);
        });

        return () => {
            unlisten.then(f => f());
        };
    }, [refreshSession]);

    const activeTabNavigate = useCallback(async (panelId: PanelId, path: string, version?: number) => {
        try {
            await invoke('active_tab_navigate', { panelId, path, version });
        } catch (e) {
            console.error("activeTabNavigate failed:", e);
        }
    }, []);

    const createTab = useCallback(async (panelId: PanelId, path: string, background?: boolean): Promise<string | undefined> => {
        try {
            return await invoke<string>('create_tab', { panelId, path, background });
        } catch (e) {
            console.error("createTab failed:", e);
            return undefined;
        }
    }, []);

    const closeTab = useCallback(async (tabId: string) => {
        try {
            await invoke('close_tab', { tabId });
        } catch (e) {
            console.error("closeTab failed:", e);
        }
    }, []);

    const switchTab = useCallback(async (tabId: string) => {
        try {
            await invoke('switch_tab', { tabId });
        } catch (e) {
            console.error("switchTab failed:", e);
        }
    }, []);

    const duplicateTab = useCallback(async (tabId: string) => {
        try {
            await invoke('duplicate_tab', { tabId });
        } catch (e) {
            console.error("duplicateTab failed:", e);
        }
    }, []);

    const closeOtherTabs = useCallback(async (tabId: string) => {
        try {
            await invoke('close_other_tabs', { tabId });
        } catch (e) {
            console.error("closeOtherTabs failed:", e);
        }
    }, []);

    return {
        session,
        isLoading,
        activeTabNavigate,
        createTab,
        closeTab,
        switchTab,
        duplicateTab,
        closeOtherTabs
    };
};
