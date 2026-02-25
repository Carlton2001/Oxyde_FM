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
    left_panel: PanelState;
    right_panel: PanelState;
    active_panel: string;
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
        await invoke('active_tab_navigate', { panelId, path, version });
    }, []);

    const createTab = useCallback(async (panelId: PanelId, path: string, background?: boolean) => {
        await invoke('create_tab', { panelId, path, background });
    }, []);

    const closeTab = useCallback(async (tabId: string) => {
        await invoke('close_tab', { tabId });
    }, []);

    const switchTab = useCallback(async (tabId: string) => {
        await invoke('switch_tab', { tabId });
    }, []);

    const duplicateTab = useCallback(async (tabId: string) => {
        await invoke('duplicate_tab', { tabId });
    }, []);

    const closeOtherTabs = useCallback(async (tabId: string) => {
        await invoke('close_other_tabs', { tabId });
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
