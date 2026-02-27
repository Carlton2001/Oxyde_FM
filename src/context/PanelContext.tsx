
import React, { createContext, useContext, useState, useMemo, useEffect, useRef, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { usePanel } from '../hooks/usePanel';
import { PanelId } from '../types';
import { useRustSession } from '../hooks/useRustSession';
import { normalizePath } from '../utils/path';

// Infers the return type of usePanel automatically
type PanelState = ReturnType<typeof usePanel>;

interface PanelContextType {
    left: PanelState;
    right: PanelState;
    activePanelId: PanelId;
    activePanel: PanelState;
    setActivePanelId: (id: PanelId) => void;
    // Helper to get the other panel
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
    // Phase 1 Migration: Use Rust Session for paths
    const { session, isLoading } = useRustSession();

    // Compute initial paths from the FIRST loaded session snapshot.
    // useState initializer ensures this only runs once, on first mount after session loads.
    const [resolvedPaths] = useState<{ left: string; right: string } | null>(() => {
        if (!session) return null;
        const leftTab = session.left_panel.tabs.find(t => t.id === session.left_panel.active_tab_id);
        const rightTab = session.right_panel.tabs.find(t => t.id === session.right_panel.active_tab_id);
        const defaultPath = "C:\\";
        return {
            left: initialLeftPath || (leftTab ? normalizePath(leftTab.path) : defaultPath),
            right: initialRightPath || (rightTab ? normalizePath(rightTab.path) : defaultPath),
        };
    });

    // Block rendering until session is loaded and initial paths are resolved
    // If resolvedPaths is null (session wasn't ready at first useState call), we need to wait
    const [initialPaths, setInitialPaths] = useState(resolvedPaths);

    useEffect(() => {
        if (initialPaths || !session) return;
        const leftTab = session.left_panel.tabs.find(t => t.id === session.left_panel.active_tab_id);
        const rightTab = session.right_panel.tabs.find(t => t.id === session.right_panel.active_tab_id);
        const defaultPath = "C:\\";
        setInitialPaths({
            left: initialLeftPath || (leftTab ? normalizePath(leftTab.path) : defaultPath),
            right: initialRightPath || (rightTab ? normalizePath(rightTab.path) : defaultPath),
        });
    }, [session, initialPaths, initialLeftPath, initialRightPath]);

    // Don't render until we have initial paths
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
 * This component receives the session from the parent so there is only
 * ONE useRustSession() instance in the tree.
 */
const PanelProviderReady: React.FC<{
    children: ReactNode;
    session: any;
    isLoading: boolean;
    initialLeftPath: string;
    initialRightPath: string;
}> = ({ children, session, isLoading, initialLeftPath, initialRightPath }) => {
    const leftActiveTabId = session?.left_panel.active_tab_id;
    const rightActiveTabId = session?.right_panel.active_tab_id;

    const left = usePanel(initialLeftPath, 'left', leftActiveTabId);
    const right = usePanel(initialRightPath, 'right', rightActiveTabId);

    const [activePanelId, setActivePanelIdState] = useState<PanelId>('left');

    // Track previous active tab IDs to detect tab switches
    const prevLeftTabIdRef = useRef(leftActiveTabId);
    const prevRightTabIdRef = useRef(rightActiveTabId);

    const setActivePanelId = (id: PanelId) => {
        setActivePanelIdState(id);
        invoke('set_active_panel', { panelId: id }).catch(console.error);
    };

    // Sync React Panel state with Rust Session
    useEffect(() => {
        if (!session) return;

        // Update Active ID
        if (session.active_panel === 'left' || session.active_panel === 'right') {
            if (activePanelId !== session.active_panel) {
                setActivePanelIdState(session.active_panel as PanelId);
            }
        }

        // Detect tab switches
        const leftTabSwitched = leftActiveTabId !== prevLeftTabIdRef.current;
        const rightTabSwitched = rightActiveTabId !== prevRightTabIdRef.current;
        prevLeftTabIdRef.current = leftActiveTabId;
        prevRightTabIdRef.current = rightActiveTabId;

        const leftTabArr = session.left_panel.tabs.find((t: any) => t.id === session.left_panel.active_tab_id);
        if (leftTabArr) {
            const normRust = normalizePath(leftTabArr.path);
            const normReact = normalizePath(left.path);

            if (leftTabSwitched && normRust !== normReact) {
                // Tab switch: force navigate regardless of version
                console.log(`[Sync] Left Tab switched to ${leftActiveTabId}. Navigating to: ${normRust}`);
                left.navigate(normRust, [], leftTabArr.version);
            } else if (leftTabArr.version > left.version) {
                console.log(`[Sync] Left Backend is ahead (v${leftTabArr.version} > v${left.version}). Catching up and moving to: ${normRust}`);
                left.navigate(normRust, [], leftTabArr.version);
            } else if (leftTabArr.version === left.version && normRust !== normReact) {
                console.log(`[Sync] Left Path mismatch at same version (v${left.version}). Correcting to: ${normRust}`);
                left.navigate(normRust, [], left.version);
            }
        }

        const rightTabArr = session.right_panel.tabs.find((t: any) => t.id === session.right_panel.active_tab_id);
        if (rightTabArr) {
            const normRust = normalizePath(rightTabArr.path);
            const normReact = normalizePath(right.path);

            if (rightTabSwitched && normRust !== normReact) {
                // Tab switch: force navigate regardless of version
                console.log(`[Sync] Right Tab switched to ${rightActiveTabId}. Navigating to: ${normRust}`);
                right.navigate(normRust, [], rightTabArr.version);
            } else if (rightTabArr.version > right.version) {
                console.log(`[Sync] Right Backend is ahead (v${rightTabArr.version} > v${right.version}). Catching up and moving to: ${normRust}`);
                right.navigate(normRust, [], rightTabArr.version);
            } else if (rightTabArr.version === right.version && normRust !== normReact) {
                console.log(`[Sync] Right Path mismatch at same version (v${right.version}). Correcting to: ${normRust}`);
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
    }), [left, right, activePanelId, isLoading]);

    return (
        <PanelContext.Provider value={value}>
            {children}
        </PanelContext.Provider>
    );
};
