import React, { createContext, useContext, useCallback, ReactNode } from 'react';
import { useRustSession, SessionState, Tab } from '../hooks/useRustSession';
import { invoke } from '@tauri-apps/api/core';
import { PanelId } from '../types';

export interface UiTab extends Tab {
    label: string;
    state?: any;
}

interface TabsContextType {
    tabs: UiTab[];
    activeTabId: string;
    addTab: (path: string, optionsOrId?: string | { id?: string, background?: boolean, index?: number }, background?: boolean) => Promise<void>;
    closeTab: (id: string, newActiveId?: string) => void;
    setActiveTab: (id: string, currentPanelState?: any) => void;
    updateTabPath: (id: string, path: string) => void;
    updateTabState: (id: string, state: any) => void; // Legacy hook compat
    duplicateTab: (id: string) => void;
    closeOtherTabs: (id: string) => void;
    reorderTabs: (sourceIndex: number, targetIndex: number) => void;

    // Rust session exposed
    session: SessionState | null;
    isLoading: boolean;
}

const TabsContext = createContext<TabsContextType | undefined>(undefined);

// Helper to determine which panel (left/right) we are acting on based on tab ID
// For now, this logic is a bit tricky without modifying the PanelContext heavily.
// We'll rely on activePanelId being passed or inferred in the new design.
// But for Phase 1 migration compatibility, we will focus on wiring commands.

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

    // Map the Rust session state to the "current active panel's tabs" 
    // to mimic the old single-list behavior for App.tsx compatibility, 
    // OR expose the full session.

    // For this Proof of Concept, let's assume we are viewing the Active Panel's tabs
    // or we might need to change how consumers use this.

    // Actually, App.tsx expects a single list of tabs because it was designed for single panel tabbing?
    // Wait, let's check App.tsx again. It calls useTabs().
    // App.tsx seems to manage Left panel tabs only? Or is it global?
    // Looking at App.tsx: lines 76-82 use tabs from useTabs().
    // Line 412: handleTabSwitch calls setActiveTab.

    // Strategy: We will bridge the Rust "Active Panel" tabs to this context.

    const activePanelId = session?.active_panel || 'left';
    const activePanelState = activePanelId === 'left' ? session?.left_panel : session?.right_panel;

    const currentTabs = activePanelState?.tabs.map(t => ({
        ...t,
        label: t.path.split('\\').pop() || t.path, // Simple label deriv
        state: null // Legacy state not yet supported in Rust
    })) || [];

    // Debug log
    const activeTabId = activePanelState?.active_tab_id || '';

    const reorderTabs = useCallback(async (sourceIndex: number, targetIndex: number) => {
        await invoke('reorder_tabs', { sourceIndex, targetIndex });
    }, []);

    const addTab = useCallback(async (path: string, optionsOrId?: string | { id?: string, background?: boolean, index?: number }, backgroundArg?: boolean) => {
        let background: boolean | undefined;
        let index: number | undefined;

        if (typeof optionsOrId === 'string') {
            // id = optionsOrId; // Ignored
            background = backgroundArg;
        } else if (typeof optionsOrId === 'object') {
            // id = optionsOrId.id; // Ignored
            background = optionsOrId.background;
            index = optionsOrId.index;
        } else {
            background = backgroundArg;
        }

        await createTab(activePanelId as PanelId, path, background);

        if (typeof index === 'number') {
            // New tab is appended at the end. Source index is currentTabs.length.
            await reorderTabs(currentTabs.length, index);
        }
    }, [createTab, activePanelId, currentTabs.length, reorderTabs]);

    const closeTab = useCallback((id: string, _newActiveId?: string) => {
        rustCloseTab(id);
    }, [rustCloseTab]);

    const setActiveTab = useCallback((id: string, _currentPanelState?: any) => {
        switchTab(id);
    }, [switchTab]);

    const updateTabPath = useCallback((id: string, path: string) => {
        // Identifying which panel this tab belongs to is tricky purely by ID if we don't know it.
        // But activeTabNavigate only works for the ACTIVE tab of a panel.
        // If the updated tab is the active one, we use activeTabNavigate.
        if (id === activeTabId) {
            activeTabNavigate(activePanelId as PanelId, path);
        } else {
            console.warn("Updating background tab path not yet fully supported in this hybrid phase");
        }
    }, [activeTabId, activePanelId, activeTabNavigate]);

    // Stubs for legacy features not yet migrated
    const updateTabState = () => { };
    const duplicateTab = useCallback((id: string) => {
        activeDuplicateTab(id);
    }, [activeDuplicateTab]);

    const closeOtherTabs = useCallback((id: string) => {
        activeCloseOtherTabs(id);
    }, [activeCloseOtherTabs]);



    const value = {
        tabs: currentTabs,
        activeTabId,
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

