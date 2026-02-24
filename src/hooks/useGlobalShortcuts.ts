import { useEffect, useRef } from 'react';
import { useKeybindings } from '../context/KeybindingContext';
import { actionService } from '../services/ActionService';
import { ActionContext } from '../types/actions';

export const useGlobalShortcuts = (context: ActionContext, tabs: any[], activeTabId: string, handleTabSwitch: (id: string) => void) => {
    const { getActionId } = useKeybindings();
    const contextRef = useRef(context);
    const tabsRef = useRef(tabs);
    const activeTabIdRef = useRef(activeTabId);

    // Update refs on every render to ensure we have the latest state in the listener
    useEffect(() => {
        contextRef.current = context;
        tabsRef.current = tabs;
        activeTabIdRef.current = activeTabId;
    });

    useEffect(() => {
        const handleKeyDown = async (e: KeyboardEvent) => {
            // Ignore if input/textarea is focused
            const target = e.target as HTMLElement;
            if (target.matches('input, textarea, [contenteditable="true"]')) {
                return;
            }

            // 1. Specialized Tab Switching logic
            const isTab = (e.code === 'Tab' || e.key === 'Tab');
            const isPageDown = (e.code === 'PageDown' || e.key === 'PageDown');
            const isPageUp = (e.code === 'PageUp' || e.key === 'PageUp');
            const isNext = (e.ctrlKey && isTab && !e.shiftKey) || (e.ctrlKey && isPageDown);
            const isPrev = (e.ctrlKey && isTab && e.shiftKey) || (e.ctrlKey && isPageUp);

            if (isNext || isPrev) {
                e.preventDefault();
                e.stopPropagation();
                const currentTabs = tabsRef.current;
                const currentActiveId = activeTabIdRef.current;
                const currentIndex = currentTabs.findIndex(t => t.id === currentActiveId);
                if (currentIndex === -1) return;
                let nextIndex = isNext ? (currentIndex + 1) % currentTabs.length : (currentIndex - 1 + currentTabs.length) % currentTabs.length;
                handleTabSwitch(currentTabs[nextIndex].id);
                return;
            }

            // 2. Construct shortcut string
            const parts = [];
            if (e.ctrlKey) parts.push('Ctrl');
            if (e.altKey) parts.push('Alt');
            if (e.shiftKey) parts.push('Shift');

            let key = e.key;
            if (key === 'Control' || key === 'Alt' || key === 'Shift') return;

            // Standardize key names for Registry
            if (e.code === 'Delete') key = 'Delete';
            else if (e.code === 'F2') key = 'F2';
            else if (e.code === 'Enter') key = 'Enter';
            else if (e.code === 'Backspace') key = 'Backspace';
            else if (key.length === 1) key = key.toUpperCase();

            parts.push(key);
            const combo = parts.join('+');

            // 3. Block unwanted shortcuts
            if (combo === 'Ctrl+N' || combo === 'Ctrl+F') {
                e.preventDefault();
                e.stopPropagation();
                return;
            }

            let actionId = getActionId(combo);

            // Special Fallback: Shift+Delete should trigger the 'file.delete' action if it exists,
            // as that action internally handles the permanent delete logic.
            if (!actionId && combo === 'Shift+Delete') {
                actionId = getActionId('Delete');
            }

            if (actionId) {
                // console.log(`[Shortcuts] Executing ${actionId} for combo ${combo}`);
                e.preventDefault();
                e.stopPropagation();
                await actionService.execute(actionId, contextRef.current);
            }
        };

        window.addEventListener('keydown', handleKeyDown, true);
        return () => window.removeEventListener('keydown', handleKeyDown, true);
    }, [getActionId, handleTabSwitch]); // handleTabSwitch is typically stable
};
