
import React, { createContext, useContext, useState, useMemo, useEffect, ReactNode } from 'react';
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

    // Defaults while loading or if fallback
    const defaultPath = "C:\\";

    const leftActiveTabId = session?.left_panel.active_tab_id;
    const rightActiveTabId = session?.right_panel.active_tab_id;

    const left = usePanel(initialLeftPath || defaultPath, 'left', leftActiveTabId);
    const right = usePanel(initialRightPath || defaultPath, 'right', rightActiveTabId);

    const [activePanelId, setActivePanelIdState] = useState<PanelId>('left');

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

        const leftTabArr = session.left_panel.tabs.find(t => t.id === session.left_panel.active_tab_id);
        if (leftTabArr) {
            const normRust = normalizePath(leftTabArr.path);
            const normReact = normalizePath(left.path);

            if (leftTabArr.version > left.version) {
                console.log(`[Sync] Left Backend is ahead (v${leftTabArr.version} > v${left.version}). Catching up and moving to: ${normRust}`);
                left.navigate(normRust, [], leftTabArr.version);
            } else if (leftTabArr.version === left.version && normRust !== normReact) {
                console.log(`[Sync] Left Path mismatch at same version (v${left.version}). Correcting to: ${normRust}`);
                left.navigate(normRust, [], left.version);
            }
        }

        const rightTabArr = session.right_panel.tabs.find(t => t.id === session.right_panel.active_tab_id);
        if (rightTabArr) {
            const normRust = normalizePath(rightTabArr.path);
            const normReact = normalizePath(right.path);

            if (rightTabArr.version > right.version) {
                console.log(`[Sync] Right Backend is ahead (v${rightTabArr.version} > v${right.version}). Catching up and moving to: ${normRust}`);
                right.navigate(normRust, [], rightTabArr.version);
            } else if (rightTabArr.version === right.version && normRust !== normReact) {
                console.log(`[Sync] Right Path mismatch at same version (v${right.version}). Correcting to: ${normRust}`);
                right.navigate(normRust, [], right.version);
            }
        }
    }, [session, left.version, left.path, right.version, right.path, activePanelId]);

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
