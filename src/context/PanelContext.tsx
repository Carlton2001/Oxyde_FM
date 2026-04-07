import React, { createContext, useContext, useState, useMemo, useEffect, useRef, useCallback, ReactNode } from 'react';
import { usePanel } from '../hooks/usePanel';
import { PanelId, HistoryEntry } from '../types';
import { useTabs } from './TabsContext';
import { normalizePath } from '../utils/path';

interface TabNavSnapshot {
    history: HistoryEntry[];
    historyIndex: number;
    version: number;
}

interface PanelContextType {
    panels: Record<PanelId, any>;
    activePanelId: PanelId;
    setActivePanelId: (id: PanelId) => void;
    activePanel: any;
    isReady: boolean;
}

const PanelContext = createContext<PanelContextType | undefined>(undefined);

const PanelStateHoister: React.FC<{
    id: PanelId,
    initialPath: string,
    activeTabId: string,
    onRegister: (id: PanelId, state: any) => void,
    onUnregister: (id: PanelId) => void
}> = React.memo(({ id, initialPath, activeTabId, onRegister, onUnregister }) => {
    const state = usePanel(initialPath, id, activeTabId);

    useEffect(() => {
        onRegister(id, state);
        return () => onUnregister(id);
    }, [id, state, onRegister, onUnregister]);

    return null;
});

export const PanelProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const { session } = useTabs();
    const [panelStates, setPanels] = useState<Record<PanelId, any>>({});
    const [panelConfigs, setConfigs] = useState<Record<PanelId, { initialPath: string, activeTabId: string }>>({});
    const [activePanelId, setActivePanelId] = useState<PanelId>('left' as PanelId);

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

    useEffect(() => {
        if (session?.active_panel_id && session.active_panel_id !== activePanelId) {
            setActivePanelId(session.active_panel_id as PanelId);
        }
    }, [session?.active_panel_id, activePanelId]);

    // Track tab histories for navigation restoration
    const tabHistoriesRef = useRef<Record<PanelId, Map<string, TabNavSnapshot>>>({});
    const prevTabIdsRef = useRef<Record<PanelId, string>>({});

    const getPanes = useCallback((node: import('../hooks/useRustSession').LayoutNode): Record<string, import('../hooks/useRustSession').PanelState> => {
        if (node.type === 'Pane') return { [node.data.id]: node.data.state };
        return node.data.children.reduce((acc, child) => ({ ...acc, ...getPanes(child) }), {});
    }, []);

    useEffect(() => {
        if (!session) return;
        const allPanels = getPanes(session.root);

        for (const [id, panel] of Object.entries(panelStates)) {
            const panelSession = allPanels[id];
            if (!panel || !panelSession) continue;

            const activeTabId = panelSession.active_tab_id;
            if (!tabHistoriesRef.current[id as PanelId]) tabHistoriesRef.current[id as PanelId] = new Map();
            
            const tabSwitched = activeTabId !== prevTabIdsRef.current[id as PanelId];

            if (tabSwitched && prevTabIdsRef.current[id as PanelId]) {
                tabHistoriesRef.current[id as PanelId]!.set(prevTabIdsRef.current[id as PanelId], {
                    history: panel.history,
                    historyIndex: panel.historyIndex,
                    version: panel.version
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
                }
            }
        }
    }, [session, panelStates, getPanes]);

    useEffect(() => {
        if (session) {
            const allPanels = getPanes(session.root);
            const sessionIds = Object.keys(allPanels);

            // 1. Cleanup removed panels
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

            // 2. Sync configurations (Add new, update existing, REMOVE orphaned)
            setConfigs(prev => {
                const next: Record<PanelId, { initialPath: string, activeTabId: string }> = {};
                let changed = false;

                // Only keep panels that are in the session
                for (const [id, p] of Object.entries(allPanels)) {
                    const panelId = id as PanelId;
                    const existing = prev[panelId];
                    const activeTabPath = p.tabs.find((t: any) => t.id === p.active_tab_id)?.path || 'C:\\';

                    if (!existing || existing.activeTabId !== p.active_tab_id) {
                        next[panelId] = {
                            initialPath: activeTabPath,
                            activeTabId: p.active_tab_id
                        };
                        changed = true;
                    } else {
                        next[panelId] = existing;
                    }
                }

                // Check if we removed any panels
                if (Object.keys(prev).length !== Object.keys(next).length) {
                    changed = true;
                }

                return changed ? next : prev;
            });
        }
    }, [session, getPanes]);

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
        setActivePanelId,
        activePanel,
        isReady
    };

    return (
        <PanelContext.Provider value={value}>
            {Object.entries(panelConfigs).map(([id, config]) => (
                <PanelStateHoister
                    key={id}
                    id={id as PanelId}
                    initialPath={config.initialPath}
                    activeTabId={config.activeTabId}
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
