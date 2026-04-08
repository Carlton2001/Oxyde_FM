import React, { createContext, useContext, useState, useMemo, useEffect, useRef, useCallback, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { usePanel } from '../hooks/usePanel';
import { PanelId, HistoryEntry, PanelInitialConfig, ViewMode, SortConfig } from '../types';
import { useTabs } from './TabsContext';
import { useSplitConfig } from './SplitConfigContext';
import { normalizePath } from '../utils/path';

interface PanelConfig {
    initialPath: string;
    activeTabId: string;
    initialConfig?: PanelInitialConfig;
}

interface TabNavSnapshot {
    history: HistoryEntry[];
    historyIndex: number;
    version: number;
}

interface TabViewConfig {
    viewMode: ViewMode;
    sortConfig: SortConfig;
    groupByDate: boolean;
}

interface PanelContextType {
    panels: Record<PanelId, any>;
    activePanelId: PanelId;
    setActivePanelId: (id: PanelId) => void;
    activePanel: any;
    isReady: boolean;
    getTabConfig: (panelId: PanelId, tabId: string) => PanelInitialConfig | undefined;
}

const PanelContext = createContext<PanelContextType | undefined>(undefined);

const PanelStateHoister: React.FC<{
    id: PanelId,
    initialPath: string,
    activeTabId: string,
    initialConfig?: PanelInitialConfig,
    onRegister: (id: PanelId, state: any) => void,
    onUnregister: (id: PanelId) => void
}> = React.memo(({ id, initialPath, activeTabId, initialConfig, onRegister, onUnregister }) => {
    const state = usePanel(initialPath, id, activeTabId, initialConfig);

    useEffect(() => {
        onRegister(id, state);
        return () => onUnregister(id);
    }, [id, state, onRegister, onUnregister]);

    return null;
});

export const PanelProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const { session, splitPanel: originalSplitPanel } = useTabs();
    const { consumeSplitConfig } = useSplitConfig();
    const [panelStates, setPanels] = useState<Record<PanelId, any>>({});
    const [panelConfigs, setConfigs] = useState<Record<PanelId, PanelConfig>>({});
    const [activePanelId, setActivePanelId] = useState<PanelId>('left' as PanelId);
    const lastManualActiveId = useRef<PanelId | null>(null);

    const onRegister = useCallback((id: PanelId, state: any) => {
        setPanels(prev => ({ ...prev, [id]: state }));
    }, []);

    const onUnregister = useCallback((id: PanelId) => {
        setPanels(prev => {
            const next = { ...prev };
            delete next[id];
            return next;
        });
    }, []);

    const syncActivePanel = useCallback((id: PanelId) => {
        lastManualActiveId.current = id;
        setActivePanelId(id);
        invoke('set_active_panel', { panelId: id }).catch(console.error);
    }, []);

    // Always sync activePanelId from session — backend is source of truth for focus
    useEffect(() => {
        if (session?.active_panel_id && session.active_panel_id !== activePanelId) {
            setActivePanelId(session.active_panel_id as PanelId);
            lastManualActiveId.current = null;
        }
    }, [session?.active_panel_id, activePanelId]);

    const tabHistoriesRef = useRef<Record<PanelId, Map<string, TabNavSnapshot>>>({});
    const tabConfigsRef = useRef<Record<PanelId, Map<string, TabViewConfig>>>({});
    const prevTabIdsRef = useRef<Record<PanelId, string>>({});

    const getPanes = useCallback((node: import('../hooks/useRustSession').LayoutNode): Record<string, import('../hooks/useRustSession').PanelState> => {
        if (node.type === 'Pane') return { [node.data.id]: node.data.state };
        return node.data.children.reduce((acc, child) => ({ ...acc, ...getPanes(child) }), {});
    }, []);

    // Intercept splitPanel to capture source state
    const splitPanel = useCallback(async (tabId: string, sourcePanelId: string, targetPanelId: string, side: 'top' | 'bottom' | 'left' | 'right') => {
        const sourcePanel = panelStates[sourcePanelId];
        if (sourcePanel) {
            const config: PanelInitialConfig = {
                viewMode: sourcePanel.viewMode,
                sortConfig: sourcePanel.sortConfig,
                groupByDate: sourcePanel.groupByDate
            };
            // We need a way to pass this config to the next usePanel.
            // Since splitting will trigger a session change and re-render PanelProvider,
            // we can store the pending config in a ref or state.
            pendingConfigsRef.current.set(sourcePanelId, config); // Use sourcePanelId as a hint? No, new panels have random IDs.
            // Actually, any NEW panel that appears after a split should probably take the source config.
            lastSplitSourceConfigRef.current = config;
        }
        return originalSplitPanel(tabId, sourcePanelId, targetPanelId, side);
    }, [panelStates, originalSplitPanel]);

    const pendingConfigsRef = useRef<Map<string, PanelInitialConfig>>(new Map());
    const lastSplitSourceConfigRef = useRef<PanelInitialConfig | null>(null);

    useEffect(() => {
        if (session) {
            const allPanels = getPanes(session.root);
            const sessionIds = Object.keys(allPanels);

            setPanels(prev => {
                const next = { ...prev };
                let changed = false;
                for (const id of Object.keys(next)) {
                    if (!sessionIds.includes(id)) {
                        delete next[id as PanelId];
                        changed = true;
                    }
                }
                return changed ? next : prev;
            });

            setConfigs(prev => {
                const next: Record<PanelId, PanelConfig> = {};
                let changed = false;
                for (const [id, p] of Object.entries(allPanels)) {
                    const panelId = id as PanelId;
                    const existing = prev[panelId];
                    const activeTabPath = p.tabs.find((t: any) => t.id === p.active_tab_id)?.path || 'C:\\';
                    
                    if (!existing) {
                        // New panel — consume the pending split config (set by the drag or direct split)
                        const initialConfig = consumeSplitConfig() || lastSplitSourceConfigRef.current || undefined;
                        lastSplitSourceConfigRef.current = null;
                        next[panelId] = {
                            initialPath: activeTabPath,
                            activeTabId: p.active_tab_id,
                            initialConfig
                        };
                        changed = true;
                    } else if (existing.activeTabId !== p.active_tab_id) {
                        next[panelId] = { ...existing, initialPath: activeTabPath, activeTabId: p.active_tab_id };
                        changed = true;
                    } else {
                        next[panelId] = existing;
                    }
                }
                if (Object.keys(prev).length !== Object.keys(next).length) {
                    changed = true;
                }
                return changed ? next : prev;
            });
        }
    }, [session, getPanes]);

    useEffect(() => {
        if (!session) return;
        const allPanels = getPanes(session.root);

        for (const [id, panel] of Object.entries(panelStates)) {
            const panelSession = allPanels[id];
            if (!panel || !panelSession) continue;

            const activeTabId = panelSession.active_tab_id;
            if (!tabHistoriesRef.current[id as PanelId]) tabHistoriesRef.current[id as PanelId] = new Map();
            
            const tabSwitched = activeTabId !== prevTabIdsRef.current[id as PanelId];

            if (!tabConfigsRef.current[id as PanelId]) tabConfigsRef.current[id as PanelId] = new Map();

            if (tabSwitched && prevTabIdsRef.current[id as PanelId]) {
                tabHistoriesRef.current[id as PanelId]!.set(prevTabIdsRef.current[id as PanelId], {
                    history: panel.history,
                    historyIndex: panel.historyIndex,
                    version: panel.version
                });
                // Save the outgoing tab's view config
                tabConfigsRef.current[id as PanelId]!.set(prevTabIdsRef.current[id as PanelId], {
                    viewMode: panel.viewMode,
                    sortConfig: panel.sortConfig,
                    groupByDate: panel.groupByDate
                });
            }
            prevTabIdsRef.current[id as PanelId] = activeTabId;

            const activeTab = panelSession.tabs.find((t: any) => t.id === activeTabId);
            if (activeTab) {
                const normRust = normalizePath(activeTab.path);
                if (tabSwitched) {
                    const saved = tabHistoriesRef.current[id as PanelId]!.get(activeTabId);
                    if (saved) {
                        panel.setNavigationState({
                            path: normRust,
                            history: saved.history,
                            historyIndex: saved.historyIndex,
                            version: saved.version
                        });
                    } else {
                        panel.setNavigationState({
                            path: normRust,
                            history: [{ path: normRust, timestamp: Date.now() }],
                            historyIndex: 0,
                            version: activeTab.version || 0
                        });
                    }
                    // Restore the incoming tab's view config (if previously visited)
                    const savedConfig = tabConfigsRef.current[id as PanelId]!.get(activeTabId);
                    if (savedConfig) {
                        panel.setViewMode(savedConfig.viewMode);
                        panel.setSortConfig(savedConfig.sortConfig);
                        panel.setGroupByDate(savedConfig.groupByDate);
                    }
                }
            }
        }
    }, [session, panelStates, getPanes]);

    const getTabConfig = useCallback((panelId: PanelId, tabId: string): PanelInitialConfig | undefined => {
        // If it's the active tab, return live state directly
        const panelState = panelStates[panelId];
        if (panelState && prevTabIdsRef.current[panelId] === tabId) {
            return {
                viewMode: panelState.viewMode,
                sortConfig: panelState.sortConfig,
                groupByDate: panelState.groupByDate
            };
        }
        // Otherwise return saved config for that tab
        return tabConfigsRef.current[panelId]?.get(tabId);
    }, [panelStates]);

    const activePanel = useMemo(() => {
        if (Object.keys(panelStates).length === 0) return null;
        return panelStates[activePanelId] || Object.values(panelStates)[0];
    }, [panelStates, activePanelId]);

    const isReady = useMemo(() => {
        const configIds = Object.keys(panelConfigs);
        return configIds.length > 0 && configIds.every(id => panelStates[id as PanelId]);
    }, [panelConfigs, panelStates]);

    const value = {
        panels: panelStates,
        activePanelId,
        setActivePanelId: syncActivePanel,
        activePanel,
        isReady,
        splitPanel, // Overridden splitPanel
        getTabConfig
    };

    return (
        <PanelContext.Provider value={value}>
            {Object.entries(panelConfigs).map(([id, config]) => (
                <PanelStateHoister
                    key={id}
                    id={id as PanelId}
                    initialPath={config.initialPath}
                    activeTabId={config.activeTabId}
                    initialConfig={config.initialConfig}
                    onRegister={onRegister}
                    onUnregister={onUnregister}
                />
            ))}
            {children}
        </PanelContext.Provider>
    );
};

export const usePanelContext = () => {
    const context = useContext(PanelContext);
    if (!context) {
        throw new Error('usePanelContext must be used within a PanelProvider');
    }
    return context;
};
