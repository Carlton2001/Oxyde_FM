import React, { createContext, useContext, useCallback, ReactNode } from 'react';
import { useRustSession, SessionState, Tab } from '../hooks/useRustSession';
import { invoke } from '@tauri-apps/api/core';
import { PanelId } from '../types';
import { useApp } from './AppContext';

export interface UiTab extends Tab {
    label: string;
    state?: any;
}

interface TabsContextType {
    tabs: UiTab[]; // Compatibility with active panel
    activeTabId: string; // Compatibility with active panel
    leftTabs: UiTab[];
    rightTabs: UiTab[];
    leftActiveTabId: string;
    rightActiveTabId: string;
    addTab: (path: string, optionsOrId?: string | { id?: string, background?: boolean, index?: number, panelId?: PanelId }, background?: boolean) => Promise<string | undefined>;
    closeTab: (id: string, panelId?: PanelId) => void;
    setActiveTab: (id: string, panelId?: PanelId) => void;
    updateTabPath: (id: string, path: string, panelId?: PanelId, version?: number) => void;
    updateTabState: (id: string, state: any) => void; // Legacy hook compat
    duplicateTab: (id: string, panelId?: PanelId) => void;
    closeOtherTabs: (id: string, panelId?: PanelId) => void;
    reorderTabs: (sourceIndex: number, targetIndex: number, panelId?: PanelId) => void;

    // Rust session exposed
    session: SessionState | null;
    isLoading: boolean;
}

const TabsContext = createContext<TabsContextType | undefined>(undefined);

export const TabsProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const {
        session,
        isLoading,
        createTab,
        closeTab: rustCloseTab,
        switchTab,
        activeTabNavigate,
        duplicateTab: activeDuplicateTab,
        closeOtherTabs: activeCloseOtherTabs
    } = useRustSession();

    const { layout } = useApp();
    const activePanelId = layout === 'standard' ? 'left' : (session?.active_panel || 'left');

    const mapTabs = (panel: any): UiTab[] => panel?.tabs.map((t: any) => ({
        ...t,
        label: t.path.split('\\').pop() || t.path,
        state: null
    })) || [];

    const leftTabs = mapTabs(session?.left_panel);
    const rightTabs = mapTabs(session?.right_panel);
    const leftActiveTabId = session?.left_panel?.active_tab_id || '';
    const rightActiveTabId = session?.right_panel?.active_tab_id || '';

    // Active panel compatibility
    const tabs = activePanelId === 'left' ? leftTabs : rightTabs;
    const activeTabId = activePanelId === 'left' ? leftActiveTabId : rightActiveTabId;

    const reorderTabs = useCallback(async (sourceIndex: number, targetIndex: number, panelId?: PanelId) => {
        const targetPanel = panelId || (activePanelId as PanelId);
        // The Rust command might need updating if it doesn't take panelId. 
        // For now we assume active panel reorder.
        await invoke('reorder_tabs', { panelId: targetPanel, sourceIndex, targetIndex });
    }, [activePanelId]);

    const addTab = useCallback(async (path: string, optionsOrId?: string | { id?: string, background?: boolean, index?: number, panelId?: PanelId }, backgroundArg?: boolean): Promise<string | undefined> => {
        let background: boolean | undefined;
        let index: number | undefined;
        let panelId: PanelId | undefined;

        if (typeof optionsOrId === 'string') {
            background = backgroundArg;
        } else if (typeof optionsOrId === 'object') {
            background = optionsOrId.background;
            index = optionsOrId.index;
            panelId = optionsOrId.panelId;
        } else {
            background = backgroundArg;
        }

        const targetPanel = panelId || (activePanelId as PanelId);
        const newId = await createTab(targetPanel, path, background);

        if (newId && typeof index === 'number') {
            try {
                // Wait for state to sync or refresh session
                await invoke('reorder_tabs', { panelId: targetPanel, sourceIndex: -1, targetIndex: index, tabId: newId });
            } catch (e) {
                console.error("Failed to reorder new tab after creation:", e);
            }
        }

        return newId;
    }, [createTab, activePanelId]);

    const closeTab = useCallback((id: string, _panelId?: PanelId) => {
        rustCloseTab(id);
    }, [rustCloseTab]);

    const setActiveTab = useCallback((id: string, _panelId?: PanelId) => {
        switchTab(id);
    }, [switchTab]);

    const updateTabPath = useCallback((_id: string, path: string, panelId?: PanelId, version?: number) => {
        const targetPanel = panelId || (activePanelId as PanelId);
        // id is used implicitly if this is current tab, otherwise we might need a specific rust command.
        // For now Ox FM logic usually navigates the active tab of the target panel.
        activeTabNavigate(targetPanel, path, version);
    }, [activePanelId, activeTabNavigate]);

    const updateTabState = () => { };
    const duplicateTab = useCallback((id: string, _panelId?: PanelId) => {
        activeDuplicateTab(id);
    }, [activeDuplicateTab]);

    const closeOtherTabs = useCallback((id: string, _panelId?: PanelId) => {
        activeCloseOtherTabs(id);
    }, [activeCloseOtherTabs]);

    const value = {
        tabs,
        activeTabId,
        leftTabs,
        rightTabs,
        leftActiveTabId,
        rightActiveTabId,
        addTab,
        closeTab,
        setActiveTab,
        updateTabPath,
        updateTabState,
        duplicateTab,
        closeOtherTabs,
        reorderTabs,
        session,
        isLoading
    };

    return (
        <TabsContext.Provider value={value}>
            {children}
        </TabsContext.Provider>
    );
};

export const useTabs = () => {
    const context = useContext(TabsContext);
    if (!context) {
        throw new Error('useTabs must be used within a TabsProvider');
    }
    return context;
};

