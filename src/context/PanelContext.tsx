
import React, { createContext, useContext, useState, useMemo, useEffect, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { usePanel } from '../hooks/usePanel';
import { PanelId } from '../types';
import { useRustSession } from '../hooks/useRustSession';

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

    // Initialize hooks
    // Note: usePanel handles its own internal logic for files/selection/etc.
    // We just need to feed it the path from the session potentially?
    // Actually, usePanel manages 'path' state internally.
    // We need to sync it with session.

    // For now, let's keep usePanel independent but initialize it with what we can?
    // Or better: Watch session changes and call navigate() on usePanel.

    const [isHydrated, setIsHydrated] = useState(false);

    const leftActiveTabId = session?.left_panel.active_tab_id;
    const rightActiveTabId = session?.right_panel.active_tab_id;

    const left = usePanel(initialLeftPath || defaultPath, 'left', isHydrated, leftActiveTabId);
    const right = usePanel(initialRightPath || defaultPath, 'right', isHydrated, rightActiveTabId);

    // Refs to track the last path we successfully synced FROM the backend.
    // This allows us to distinguish between "Backend changed" (Tab switch) vs "Frontend changed" (Navigation)
    const lastSyncedLeft = React.useRef<string | null>(null);
    const lastSyncedRight = React.useRef<string | null>(null);

    const [activePanelId, setActivePanelIdState] = useState<PanelId>('left');

    const setActivePanelId = (id: PanelId) => {
        setActivePanelIdState(id);
        invoke('set_active_panel', { panelId: id }).catch(console.error);
    };

    // Sync React Panel state with Rust Session
    useEffect(() => {
        if (!session) return;

        // Initial Hydration
        if (!isHydrated) {
            const leftTab = session.left_panel.tabs.find(t => t.id === session.left_panel.active_tab_id);
            if (leftTab && leftTab.path !== left.path) {
                left.navigate(leftTab.path);
            }

            const activePanel = session.active_panel as PanelId;
            setActivePanelIdState(activePanel);

            const rightTab = session.right_panel.tabs.find(t => t.id === session.right_panel.active_tab_id);
            if (rightTab && rightTab.path !== right.path) {
                right.navigate(rightTab.path);
            }

            setIsHydrated(true);
            return;
        }

        // Normal Sync (Rust -> React)
        // If Rust changes (e.g. via command), update React (navigate)
        // This handles "Tab Click" -> Rust Command -> Event -> Session Update -> React Navigate

        // Update Active ID
        if (session.active_panel === 'left' || session.active_panel === 'right') {
            if (activePanelId !== session.active_panel) {
                setActivePanelIdState(session.active_panel as PanelId);
            }
        }

        const leftTab = session.left_panel.tabs.find(t => t.id === session.left_panel.active_tab_id);
        if (leftTab) {
            // Logic: Only update Frontend IF Backend has moved from what we last saw.
            // If Backend is same as last sync, but Frontend is different, assume Frontend is "ahead" (local nav).
            if (leftTab.path !== lastSyncedLeft.current) {
                if (leftTab.path !== left.path) {
                    left.navigate(leftTab.path);
                }
                lastSyncedLeft.current = leftTab.path;
            }
        }

        const rightTab = session.right_panel.tabs.find(t => t.id === session.right_panel.active_tab_id);
        if (rightTab) {
            if (rightTab.path !== lastSyncedRight.current) {
                if (rightTab.path !== right.path) {
                    right.navigate(rightTab.path);
                }
                lastSyncedRight.current = rightTab.path;
            }
        }
    }, [session, isHydrated]);
    // Dependency on 'left' and 'right' omitted to avoid deep re-renders causing loops, 
    // assuming left.navigate is stable or we check path equality.

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

