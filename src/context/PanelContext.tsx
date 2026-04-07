import React, { createContext, useContext, useState, useMemo, useEffect, useRef, useCallback, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { usePanel } from '../hooks/usePanel';
import { PanelId } from '../types';
import { useRustSession } from '../hooks/useRustSession';
import { normalizePath } from '../utils/path';
import { HistoryEntry } from '../types';

// Infers the return type of usePanel automatically
type PanelState = ReturnType<typeof usePanel>;

interface TabNavSnapshot {
    history: HistoryEntry[];
    historyIndex: number;
    version: number;
}

interface PanelContextType {
    left: PanelState;
    right: PanelState;
    activePanelId: PanelId;
    activePanel: PanelState;
    setActivePanelId: (id: PanelId) => void;
    otherPanel: PanelState;
    isLoading: boolean;
}

const PanelContext = createContext<PanelContextType | null>(null);

export const usePanelContext = () => {
    const context = useContext(PanelContext);
    if (!context) {
        throw new Error('usePanelContext must be used within a PanelProvider');
    }
    return context;
};

interface PanelProviderProps {
    children: ReactNode;
    initialLeftPath?: string;
    initialRightPath?: string;
}

export const PanelProvider: React.FC<PanelProviderProps> = ({
    children,
    initialLeftPath,
    initialRightPath
}) => {
    const { session, isLoading } = useRustSession();

    const [resolvedPaths] = useState<{ left: string; right: string } | null>(() => {
        if (!session) return null;
        const leftPanel = session.panels['left'];
        const rightPanel = session.panels['right'];
        const leftTab = leftPanel?.tabs.find((t: any) => t.id === leftPanel.active_tab_id);
        const rightTab = rightPanel?.tabs.find((t: any) => t.id === rightPanel.active_tab_id);
        const defaultPath = "C:\\";
        return {
            left: initialLeftPath || (leftTab ? normalizePath(leftTab.path) : defaultPath),
            right: initialRightPath || (rightTab ? normalizePath(rightTab.path) : defaultPath),
        };
    });

    const [initialPaths, setInitialPaths] = useState(resolvedPaths);

    useEffect(() => {
        if (initialPaths || !session) return;
        const leftPanel = session.panels['left'];
        const rightPanel = session.panels['right'];
        const leftTab = leftPanel?.tabs.find((t: any) => t.id === leftPanel.active_tab_id);
        const rightTab = rightPanel?.tabs.find((t: any) => t.id === rightPanel.active_tab_id);
        const defaultPath = "C:\\";
        setInitialPaths({
            left: initialLeftPath || (leftTab ? normalizePath(leftTab.path) : defaultPath),
            right: initialRightPath || (rightTab ? normalizePath(rightTab.path) : defaultPath),
        });
    }, [session, initialPaths, initialLeftPath, initialRightPath]);

    if (isLoading || !initialPaths) {
        return null;
    }

    return (
        <PanelProviderReady
            session={session}
            isLoading={isLoading}
            initialLeftPath={initialPaths.left}
            initialRightPath={initialPaths.right}
        >
            {children}
        </PanelProviderReady>
    );
};

/**
 * Inner component that creates panels. Separated so that usePanel hooks
 * are only called once initial paths are definitively known.
 */
const PanelProviderReady: React.FC<{
    children: ReactNode;
    session: any;
    isLoading: boolean;
    initialLeftPath: string;
    initialRightPath: string;
}> = ({ children, session, isLoading, initialLeftPath, initialRightPath }) => {
    const leftActiveTabId = session?.panels?.['left']?.active_tab_id;
    const rightActiveTabId = session?.panels?.['right']?.active_tab_id;

    const left = usePanel(initialLeftPath, 'left', leftActiveTabId);
    const right = usePanel(initialRightPath, 'right', rightActiveTabId);

    const [activePanelId, setActivePanelIdState] = useState<PanelId>('left');

    // --- Per-tab isolated navigation history ---
    // Each tab has its own history stack. On tab switch, we save the outgoing tab's
    // stack and restore the incoming tab's stack via setNavigationState (no navigate() call,
    // which would wrongly push a new entry into the shared history).
    const leftTabHistoriesRef = useRef<Map<string, TabNavSnapshot>>(new Map());
    const rightTabHistoriesRef = useRef<Map<string, TabNavSnapshot>>(new Map());

    // Track previous active tab IDs to detect switches
    const prevLeftTabIdRef = useRef<string>(leftActiveTabId);
    const prevRightTabIdRef = useRef<string>(rightActiveTabId);

    const setActivePanelId = useCallback((id: PanelId) => {
        setActivePanelIdState(id);
        invoke('set_active_panel', { panelId: id }).catch(console.error);
    }, []);

    // Sync React panel state with Rust session
    useEffect(() => {
        if (!session) return;

        // Sync active panel
        if (session.active_panel === 'left' || session.active_panel === 'right') {
            if (activePanelId !== session.active_panel) {
                setActivePanelIdState(session.active_panel as PanelId);
            }
        }

        const leftTabSwitched = leftActiveTabId !== prevLeftTabIdRef.current;
        const rightTabSwitched = rightActiveTabId !== prevRightTabIdRef.current;

        // ─── LEFT PANEL ──────────────────────────────────────────────────
        if (leftTabSwitched && prevLeftTabIdRef.current) {
            // Save outgoing tab's full nav state
            leftTabHistoriesRef.current.set(prevLeftTabIdRef.current, {
                history: left.history,
                historyIndex: left.historyIndex,
                version: left.version
            });
        }
        prevLeftTabIdRef.current = leftActiveTabId;

        const leftPanelSession = session.panels?.['left'];
        const leftTabArr = leftPanelSession?.tabs?.find((t: any) => t.id === leftPanelSession.active_tab_id);
        if (leftTabArr) {
            const normRust = normalizePath(leftTabArr.path);

            if (leftTabSwitched) {
                // Tab switch → restore saved history or start fresh
                const saved = leftTabHistoriesRef.current.get(leftActiveTabId);
                if (saved) {
                    left.setNavigationState({
                        path: normRust,
                        history: saved.history,
                        historyIndex: saved.historyIndex,
                        version: saved.version
                    });
                } else {
                    left.setNavigationState({
                        path: normRust,
                        history: [{ path: normRust, selected: [] }],
                        historyIndex: 0,
                        version: leftTabArr.version
                    });
                }
            } else if (leftTabArr.version > left.version) {
                left.navigate(normRust, [], leftTabArr.version);
            } else if (leftTabArr.version === left.version && normalizePath(left.path) !== normRust) {
                left.navigate(normRust, [], left.version);
            }
        }

        // ─── RIGHT PANEL ─────────────────────────────────────────────────
        if (rightTabSwitched && prevRightTabIdRef.current) {
            // Save outgoing tab's full nav state
            rightTabHistoriesRef.current.set(prevRightTabIdRef.current, {
                history: right.history,
                historyIndex: right.historyIndex,
                version: right.version
            });
        }
        prevRightTabIdRef.current = rightActiveTabId;

        const rightPanelSession = session.panels?.['right'];
        const rightTabArr = rightPanelSession?.tabs?.find((t: any) => t.id === rightPanelSession.active_tab_id);
        if (rightTabArr) {
            const normRust = normalizePath(rightTabArr.path);

            if (rightTabSwitched) {
                // Tab switch → restore saved history or start fresh
                const saved = rightTabHistoriesRef.current.get(rightActiveTabId);
                if (saved) {
                    right.setNavigationState({
                        path: normRust,
                        history: saved.history,
                        historyIndex: saved.historyIndex,
                        version: saved.version
                    });
                } else {
                    right.setNavigationState({
                        path: normRust,
                        history: [{ path: normRust, selected: [] }],
                        historyIndex: 0,
                        version: rightTabArr.version
                    });
                }
            } else if (rightTabArr.version > right.version) {
                right.navigate(normRust, [], rightTabArr.version);
            } else if (rightTabArr.version === right.version && normalizePath(right.path) !== normRust) {
                right.navigate(normRust, [], right.version);
            }
        }
    }, [session, left.version, left.path, right.version, right.path, activePanelId, leftActiveTabId, rightActiveTabId]);

    const value = useMemo(() => ({
        left,
        right,
        activePanelId,
        activePanel: activePanelId === 'left' ? left : right,
        otherPanel: activePanelId === 'left' ? right : left,
        setActivePanelId,
        isLoading
    }), [left, right, activePanelId, isLoading, setActivePanelId]);

    return (
        <PanelContext.Provider value={value}>
            {children}
        </PanelContext.Provider>
    );
};
