import { useState, useCallback } from 'react';
import { getParent } from '../utils/path';
import { HistoryEntry } from '../types';

interface NavigationState {
    path: string;
    history: HistoryEntry[];
    historyIndex: number;
}

export const useNavigation = (initialPath: string = "C:\\") => {
    const [state, setState] = useState<NavigationState>(() => {
        const initialEntry: HistoryEntry = { path: initialPath, selected: [] };
        return {
            path: initialPath,
            history: [initialEntry],
            historyIndex: 0
        };
    });

    const navigate = useCallback((newPath: string, currentSelection?: string[]) => {
        if (!newPath) return;

        setState(prev => {
            if (newPath === prev.path) return prev;

            let newHistory = prev.history.slice(0, prev.historyIndex + 1);
            // Update selection for current entry before moving away
            if (currentSelection !== undefined && newHistory[prev.historyIndex]) {
                newHistory[prev.historyIndex] = { ...newHistory[prev.historyIndex], selected: currentSelection };
            }

            newHistory.push({ path: newPath, selected: [] });

            // Limit history size to prevent unbounded growth
            const MAX_HISTORY = 100;
            let newIndex = newHistory.length - 1;
            if (newHistory.length > MAX_HISTORY) {
                newHistory = newHistory.slice(newHistory.length - MAX_HISTORY);
                newIndex = newHistory.length - 1;
            }

            return {
                path: newPath,
                history: newHistory,
                historyIndex: newIndex
            };
        });
    }, []);

    const goBack = useCallback((currentSelection?: string[]) => {
        setState(prev => {
            if (prev.historyIndex > 0) {
                const newHistory = [...prev.history];
                // Save current selection before going back
                if (currentSelection !== undefined && newHistory[prev.historyIndex]) {
                    newHistory[prev.historyIndex] = { ...newHistory[prev.historyIndex], selected: currentSelection };
                }

                const idx = prev.historyIndex - 1;
                const entry = newHistory[idx];
                if (entry) {
                    return {
                        ...prev,
                        path: entry.path,
                        history: newHistory,
                        historyIndex: idx
                    };
                }
            }
            return prev;
        });
    }, []);

    const goForward = useCallback((currentSelection?: string[]) => {
        setState(prev => {
            if (prev.historyIndex < prev.history.length - 1) {
                const newHistory = [...prev.history];
                // Save current selection before going forward
                if (currentSelection !== undefined && newHistory[prev.historyIndex]) {
                    newHistory[prev.historyIndex] = { ...newHistory[prev.historyIndex], selected: currentSelection };
                }

                const idx = prev.historyIndex + 1;
                const entry = newHistory[idx];
                if (entry) {
                    return {
                        ...prev,
                        path: entry.path,
                        history: newHistory,
                        historyIndex: idx
                    };
                }
            }
            return prev;
        });
    }, []);

    const goUp = useCallback((currentSelection?: string[]) => {
        setState(prev => {
            const parent = getParent(prev.path);
            if (parent && parent !== prev.path) {
                const newHistory = prev.history.slice(0, prev.historyIndex + 1);
                // Save current selection
                if (currentSelection !== undefined && newHistory[prev.historyIndex]) {
                    newHistory[prev.historyIndex] = { ...newHistory[prev.historyIndex], selected: currentSelection };
                }

                newHistory.push({ path: parent, selected: [] });
                return {
                    path: parent,
                    history: newHistory,
                    historyIndex: newHistory.length - 1
                };
            }
            return prev;
        });
    }, []);

    const updateCurrentSelection = useCallback((selected: string[]) => {
        setState(prev => {
            const newHistory = [...prev.history];
            if (newHistory[prev.historyIndex]) {
                newHistory[prev.historyIndex] = { ...newHistory[prev.historyIndex], selected };
            }
            return { ...prev, history: newHistory };
        });
    }, []);

    const setNavigationState = useCallback((state: NavigationState) => {
        setState(state);
    }, []);

    return {
        path: state.path,
        history: state.history,
        historyIndex: state.historyIndex,
        currentEntry: state.history[state.historyIndex],
        navigate,
        goBack,
        goForward,
        goUp,
        updateCurrentSelection,
        setNavigationState
    };
};
