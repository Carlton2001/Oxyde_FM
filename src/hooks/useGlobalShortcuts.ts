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
            const target = e.target as HTMLElement;
            const isInput = target.matches('input, textarea, [contenteditable="true"]');
            const isCtrlF = (e.ctrlKey && (e.key === 'f' || e.key === 'F'));

            // 0. Block standard browser shortcuts (Save, Print, Find Next, Refresh, etc.)
            // to prevent WebView2/Edge overrides in production.
            if (e.ctrlKey && ['s', 'S', 'p', 'P', 'g', 'G', 'l', 'L', 'r', 'R'].includes(e.key)) {
                e.preventDefault();
                e.stopPropagation();
            }
            if (e.key === 'F5') {
                e.preventDefault();
                e.stopPropagation();
            }

            if (e.defaultPrevented || (isInput && !isCtrlF)) {
                return;
            }

            const isTab = (e.key === 'Tab');
            const isPageDown = (e.code === 'PageDown' || e.key === 'PageDown');
            const isPageUp = (e.code === 'PageUp' || e.key === 'PageUp');
            const isNextTab = (e.ctrlKey && isTab && !e.shiftKey) || (e.ctrlKey && isPageDown);
            const isPrevTab = (e.ctrlKey && isTab && e.shiftKey) || (e.ctrlKey && isPageUp);

            if (isNextTab || isPrevTab) {
                e.preventDefault();
                e.stopPropagation();
                const currentTabs = tabsRef.current;
                const currentActiveId = activeTabIdRef.current;
                const currentIndex = currentTabs.findIndex(t => t.id === currentActiveId);
                if (currentIndex === -1) return;
                let nextIndex = isNextTab ? (currentIndex + 1) % currentTabs.length : (currentIndex - 1 + currentTabs.length) % currentTabs.length;
                handleTabSwitch(currentTabs[nextIndex].id);
                return;
            }

            // Tab key alone switches between panels
            if (isTab && !e.ctrlKey && !e.altKey && !e.metaKey) {
                e.preventDefault();
                e.stopPropagation();
                const ctx = contextRef.current;
                if (ctx.panels && ctx.activePanelId && ctx.setActivePanelId) {
                    const panelIds = Object.keys(ctx.panels);
                    if (panelIds.length > 1) {
                        const currentIndex = panelIds.indexOf(ctx.activePanelId);
                        const nextIndex = e.shiftKey ? 
                            (currentIndex - 1 + panelIds.length) % panelIds.length : 
                            (currentIndex + 1) % panelIds.length;
                        ctx.setActivePanelId(panelIds[nextIndex]);
                    }
                }
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
            if (e.code === 'Space') key = 'Space';
            else if (e.code === 'Delete') key = 'Delete';
            else if (e.code === 'F2') key = 'F2';
            else if (e.code === 'Enter') key = 'Enter';
            else if (e.code === 'ContextMenu') key = 'ContextMenu';
            else if (e.code === 'Backspace') key = 'Backspace';
            else if (key.length === 1) key = key.toUpperCase();

            parts.push(key);
            const combo = parts.join('+');

            // 3. Specialized shortcuts
            if (combo === 'Ctrl+N') {
                e.preventDefault();
                e.stopPropagation();
                return;
            }

            if (combo === 'Ctrl+F' || (e.ctrlKey && (e.key === 'f' || e.key === 'F'))) {
                e.preventDefault();
                e.stopPropagation();
                // Find the search input in the active panel wrapper
                const selector = '.individual-panel-wrapper.active .path-bar-search-box input';
                const searchInput = document.querySelector(selector) as HTMLInputElement;
                if (searchInput) {
                    searchInput.focus();
                    searchInput.select();
                }
                return;
            }

            if (combo === 'F5') {
                e.preventDefault();
                e.stopPropagation();
                if (contextRef.current.refreshBothPanels) {
                    contextRef.current.refreshBothPanels();
                }
                return;
            }

            if (combo === 'Ctrl+=' || combo === 'Ctrl++' || combo === 'Ctrl+-') {
                e.preventDefault();
                e.stopPropagation();
                // We need to look up the current font size from context
                // but since it's inside contextRef, we can access it
                const currentSize = (contextRef.current as any).settings?.fontSize || 16;
                const delta = (combo === 'Ctrl+=' || combo === 'Ctrl++') ? 1 : -1;
                const setFontSize = (contextRef.current as any).settings?.setFontSize;
                if (setFontSize) {
                    setFontSize(currentSize + delta);
                }
                return;
            }

            let actionId = getActionId(combo);

            // Special Fallback: Shift+Delete should trigger the 'file.delete' action if it exists,
            // as that action internally handles the permanent delete logic.
            if (!actionId && combo === 'Shift+Delete') {
                actionId = getActionId('Delete');
            }

            if (actionId) {
                const action = actionService.get(actionId);
                const isEnabled = action && (!action.isEnabled || action.isEnabled({ ...contextRef.current, shortcutCombo: combo }));

                if (isEnabled) {
                    e.preventDefault();
                    e.stopPropagation();
                    // console.log(`[Shortcuts] Executing ${actionId} for combo ${combo}`);
                    await actionService.execute(actionId, { ...contextRef.current, shortcutCombo: combo });
                }
            }
        };

        window.addEventListener('keydown', handleKeyDown, false);
        return () => window.removeEventListener('keydown', handleKeyDown, false);
    }, [getActionId, handleTabSwitch]); // handleTabSwitch is typically stable
};
