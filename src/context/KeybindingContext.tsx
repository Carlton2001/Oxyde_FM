import React, { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import { actionService } from '../services/ActionService';

// Shortcut string format: "Ctrl+C", "Alt+Enter", "F2", etc.
type Shortcut = string;
type ActionId = string;

interface KeybindingContextType {
    keybindings: Map<Shortcut, ActionId>;
    getActionId: (shortcut: Shortcut) => ActionId | undefined;
    getShortcut: (actionId: ActionId) => Shortcut | undefined;
    registerKeybinding: (shortcut: Shortcut, actionId: ActionId) => void;
    unregisterKeybinding: (shortcut: Shortcut) => void;
}

const KeybindingContext = createContext<KeybindingContextType | null>(null);

export const KeybindingProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [keybindings, setKeybindings] = useState<Map<Shortcut, ActionId>>(new Map());

    // Initialize from ActionRegistry
    useEffect(() => {
        const initialBindings = new Map<Shortcut, ActionId>();
        const actions = actionService.getAll();

        actions.forEach(action => {
            if (action.shortcut) {
                // Normalize shortcut string if needed, for now assume exact match
                initialBindings.set(action.shortcut, action.id);
            }
        });

        setKeybindings(initialBindings);
        // console.log(`KeybindingContext: Loaded ${initialBindings.size} shortcuts.`);
    }, []);

    const getActionId = React.useCallback((shortcut: Shortcut) => {
        return keybindings.get(shortcut);
    }, [keybindings]);

    const getShortcut = React.useCallback((actionId: ActionId) => {
        for (const [shortcut, id] of keybindings.entries()) {
            if (id === actionId) return shortcut;
        }
        return undefined;
    }, [keybindings]);

    const registerKeybinding = React.useCallback((shortcut: Shortcut, actionId: ActionId) => {
        setKeybindings(prev => {
            const next = new Map(prev);
            next.set(shortcut, actionId);
            return next;
        });
    }, []);

    const unregisterKeybinding = React.useCallback((shortcut: Shortcut) => {
        setKeybindings(prev => {
            const next = new Map(prev);
            next.delete(shortcut);
            return next;
        });
    }, []);

    return (
        <KeybindingContext.Provider value={{
            keybindings,
            getActionId,
            getShortcut,
            registerKeybinding,
            unregisterKeybinding
        }}>
            {children}
        </KeybindingContext.Provider>
    );
};

export const useKeybindings = () => {
    const context = useContext(KeybindingContext);
    if (!context) {
        throw new Error('useKeybindings must be used within a KeybindingProvider');
    }
    return context;
};

