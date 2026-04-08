import React, { createContext, useContext, useCallback, ReactNode } from 'react';
import { useRustSession, SessionState, Tab } from '../hooks/useRustSession';
import { invoke } from '@tauri-apps/api/core';
import { PanelId, PanelInitialConfig } from '../types';
import { useSplitConfig } from './SplitConfigContext';

export interface UiTab extends Tab {
    label: string;
    state?: any;
}

interface TabsContextType {
    panels: Record<string, { tabs: UiTab[], activeTabId: string }>;
    activePanelId: string;
    setActivePanelId: (id: string) => void;
    addTab: (path: string, optionsOrId?: string | { id?: string, background?: boolean, index?: number, panelId?: PanelId }, background?: boolean) => Promise<string | undefined>;
    closeTab: (id: string, panelId?: PanelId) => void;
    setActiveTab: (id: string, panelId?: PanelId) => void;
    updateTabPath: (id: string, path: string, panelId?: PanelId, version?: number) => void;
    updateTabState: (id: string, state: any) => void; // Legacy hook compat
    duplicateTab: (id: string, panelId?: PanelId) => void;
    closeOtherTabs: (id: string, panelId?: PanelId) => void;
    reorderTabs: (sourceIndex: number, targetIndex: number, panelId?: PanelId) => void;

    // Unified drag state
    draggedTab: { id: string, panelId: PanelId, path: string, label: string, initialConfig?: PanelInitialConfig } | null;
    setDraggedTab: (tab: { id: string, panelId: PanelId, path: string, label: string, initialConfig?: PanelInitialConfig } | null) => void;
    dragPos: { x: number, y: number };
    internalDropIndex: number | null;
    targetPanelId: PanelId | null;
    markerOffset: number | null;
    registerPanel: (panelId: PanelId, ref: React.RefObject<HTMLDivElement>) => void;
    moveTab: (tabId: string, sourcePanel: PanelId, targetPanelId: PanelId, targetIndex: number) => Promise<void>;
    splitPanel: (tabId: string, sourcePanelId: PanelId, targetPanelId: PanelId, side: 'top' | 'bottom' | 'left' | 'right', config?: PanelInitialConfig) => Promise<void>;
    setActiveDropZone: (zone: { panelId: PanelId; side: 'top' | 'bottom' | 'left' | 'right' } | null) => void;

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
        duplicateTab: activeDuplicateTab,
        closeOtherTabs: activeCloseOtherTabs,
        activeTabNavigate
    } = useRustSession();

    const [activePanelId, setActivePanelId] = React.useState('left');

    const mapTabs = (panel: any) => {
        if (!panel) return [];
        return panel.tabs.map((t: any) => ({
            id: t.id,
            label: t.path.split('\\').filter(Boolean).pop() || t.path,
            path: t.path
        })) as UiTab[];
    };

    const panels = React.useMemo(() => {
        const result: Record<string, { tabs: UiTab[], activeTabId: string }> = {};
        
        const traverse = (node: import('../hooks/useRustSession').LayoutNode) => {
            if (node.type === 'Pane') {
                const { id, state } = node.data;
                result[id] = {
                    tabs: mapTabs(state),
                    activeTabId: state.active_tab_id || ''
                };
            } else {
                node.data.children.forEach(traverse);
            }
        };

        if (session) {
            traverse(session.root);
        }
        return result;
    }, [session]);

    // Sync activePanelId with session once loaded
    React.useEffect(() => {
        if (session?.active_panel_id) {
            setActivePanelId(session.active_panel_id);
        }
    }, [session?.active_panel_id]);

    const [draggedTab, setDraggedTab] = React.useState<{ id: string, panelId: PanelId, path: string, label: string, initialConfig?: PanelInitialConfig } | null>(null);
    const [dragPos, setDragPos] = React.useState({ x: 0, y: 0 });
    const [internalDropIndex, setInternalDropIndex] = React.useState<number | null>(null);
    const [targetPanelId, setTargetPanelId] = React.useState<PanelId | null>(null);
    const [markerOffset, setMarkerOffset] = React.useState<number | null>(null);

    // Refs for hit-testing across panels
    const panelsRefs = React.useRef<Map<PanelId, React.RefObject<HTMLDivElement>>>(new Map());
    const registerPanel = useCallback((panelId: PanelId, ref: React.RefObject<HTMLDivElement>) => {
        panelsRefs.current.set(panelId, ref);
    }, []);

    // Ref for tracking which drop zone is currently hovered (set by MultipaneDropZones)
    const activeDropZoneRef = React.useRef<{ panelId: PanelId; side: 'top' | 'bottom' | 'left' | 'right' } | null>(null);
    const setActiveDropZone = useCallback((zone: { panelId: PanelId; side: 'top' | 'bottom' | 'left' | 'right' } | null) => {
        activeDropZoneRef.current = zone;
    }, []);

    const reorderTabs = useCallback(async (sourceIndex: number, targetIndex: number, panelId?: PanelId) => {
        const targetPanel = panelId || (activePanelId as PanelId);
        try {
            await invoke('reorder_tabs', { panelId: targetPanel, sourceIndex, targetIndex });
        } catch (e) {
            console.error("[reorderTabs] Failed:", e);
        }
    }, [activePanelId]);

    const moveTab = useCallback(async (tabId: string, sourcePanel: string, targetPanel: string, targetIndex: number) => {
        try {
            if (sourcePanel === targetPanel) {
                // It's a reorder
                const panel = panels[sourcePanel];
                const sourceIndex = panel?.tabs.findIndex(t => t.id === tabId);
                if (sourceIndex !== undefined && sourceIndex !== -1) {
                    await invoke('reorder_tabs', { panelId: sourcePanel, sourceIndex, targetIndex });
                }
            } else {
                // Cross-panel move
                await invoke('move_tab_between_panels', { tabId, sourcePanelId: sourcePanel, targetPanelId: targetPanel, targetIndex });
            }
        } catch (e) {
            console.error("[moveTab] Failed:", e);
        }
    }, [panels]);

    const { captureSplitConfig } = useSplitConfig();

    const splitPanel = useCallback(async (tabId: string, sourcePanelId: string, targetPanelId: string, side: 'top' | 'bottom' | 'left' | 'right', config?: PanelInitialConfig) => {
        try {
            if (config) {
                captureSplitConfig(config);
            }
            await invoke('split_panel', { tabId, sourcePanelId, targetPanelId, side });
        } catch (e) {
            console.error("[splitPanel] Failed:", e);
        }
    }, [captureSplitConfig]);

    const dragTargetRef = React.useRef<{ panelId: PanelId | null, index: number | null, offset: number | null }>({ panelId: null, index: null, offset: null });

    // Global drag effects
    React.useEffect(() => {
        if (!draggedTab) {
            dragTargetRef.current = { panelId: null, index: null, offset: null };
            return;
        }

        const handleMouseMove = (e: MouseEvent) => {
            setDragPos({ x: e.clientX, y: e.clientY });

            let bestPanel: PanelId | null = null;
            let bestIndexResult: number | null = null;
            let bestOffset: number | null = null;

            for (const [id, ref] of panelsRefs.current.entries()) {
                if (!ref.current) continue;
                const rect = ref.current.getBoundingClientRect();
                const isOver = e.clientX >= rect.left && e.clientX <= rect.right &&
                    e.clientY >= rect.top && e.clientY <= rect.bottom;

                if (isOver) {
                    bestPanel = id;
                    const panel = panels[id];
                    const tabElements = Array.from(ref.current.querySelectorAll('.tab'));
                    let bestIndex = panel.tabs.length;

                    if (tabElements.length > 0) {
                        const lastTabRect = tabElements[tabElements.length - 1].getBoundingClientRect();
                        bestOffset = lastTabRect.right - rect.left;
                    } else {
                        bestOffset = 0;
                    }

                    for (let i = 0; i < tabElements.length; i++) {
                        const tRect = tabElements[i].getBoundingClientRect();
                        const mid = tRect.left + tRect.width / 2;
                        if (e.clientX < mid) {
                            bestIndex = i;
                            bestOffset = tRect.left - rect.left;
                            break;
                        }
                    }
                    bestIndexResult = bestIndex;
                    break;
                }
            }

            if (bestPanel !== dragTargetRef.current.panelId || bestIndexResult !== dragTargetRef.current.index || bestOffset !== dragTargetRef.current.offset) {
                // Logic to hide marker if drop is redundant (same position)
                let finalOffset = bestOffset;
                if (draggedTab && bestPanel === draggedTab.panelId) {
                    const sourceIndex = panels[draggedTab.panelId].tabs.findIndex(t => t.id === draggedTab.id);
                    if (bestIndexResult === sourceIndex || bestIndexResult === sourceIndex + 1) {
                        finalOffset = null;
                    }
                }

                dragTargetRef.current = { panelId: bestPanel, index: bestIndexResult, offset: bestOffset };
                setTargetPanelId(bestPanel);
                setInternalDropIndex(bestIndexResult);
                setMarkerOffset(finalOffset);
            }
        };

        const handleMouseUp = async (_e: MouseEvent) => {
            if (!draggedTab) return;

            // Capture all state before clearing
            const capturedTab = draggedTab;
            const capturedDropZone = activeDropZoneRef.current;
            const { panelId: tPanel, index: tIndex } = dragTargetRef.current;

            if (capturedDropZone) {
                // Dropped on a split zone — create a new panel
                await splitPanel(capturedTab.id, capturedTab.panelId, capturedDropZone.panelId, capturedDropZone.side, capturedTab.initialConfig);
            } else if (tPanel !== null && tIndex !== null) {
                // Dropped on a tab bar — move the tab
                await moveTab(capturedTab.id, capturedTab.panelId, tPanel, tIndex);
            }

            setDraggedTab(null);
            setTargetPanelId(null);
            setInternalDropIndex(null);
            setMarkerOffset(null);
            activeDropZoneRef.current = null;
        };

        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', handleMouseUp);
        return () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };
    }, [draggedTab, panels, moveTab]);

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
                // The tab was appended, so its source index is tabs.length - 1
                // Wait for state to sync or use the count from session
                const currentTabs = panels[targetPanel]?.tabs || [];
                await invoke('reorder_tabs', { 
                    panelId: targetPanel, 
                    sourceIndex: currentTabs.length, // createTab already added it
                    targetIndex: index 
                });
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
        panels,
        activePanelId,
        setActivePanelId,
        addTab,
        closeTab,
        setActiveTab,
        updateTabPath,
        updateTabState,
        duplicateTab,
        closeOtherTabs,
        reorderTabs,
        draggedTab,
        setDraggedTab,
        dragPos,
        internalDropIndex,
        targetPanelId,
        markerOffset,
        registerPanel,
        moveTab,
        splitPanel,
        setActiveDropZone,
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

