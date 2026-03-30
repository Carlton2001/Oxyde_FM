import { ActionDefinition } from '../types/actions';
import { Plus } from 'lucide-react';

export const NEW_TAB_ACTION: ActionDefinition = {
    id: 'tab.new',
    label: 'new_tab',
    icon: Plus,
    shortcut: 'Ctrl+T',
    isVisible: (ctx) => {
        // Only visible/relevant in standard (single panel) mode as dual panel has fixed tabs per panel
        // Actually, even in dual panel we might want to add tabs to the active panel.
        // But the user specifically asked for "single panel mode".
        return ctx.layout === 'standard';
    },
    handler: async (ctx) => {
        if (typeof ctx.addTab === 'function') {
            // Replicate the 'New Tab' button behavior: open C:\
            const id = await ctx.addTab('C:\\', { background: false });
            if (id && typeof ctx.setActiveTab === 'function') {
                ctx.setActiveTab(id);
                // The switch to C:\ will be handled by the tab session sync in App.tsx
                // or we can explicitly navigate the panel.
                if (ctx.activePanel && typeof ctx.activePanel.navigate === 'function') {
                    ctx.activePanel.navigate('C:\\');
                }
            }
        }
    }
};
