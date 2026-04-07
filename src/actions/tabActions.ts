import { ActionDefinition } from '../types/actions';
import { Plus } from 'lucide-react';

export const NEW_TAB_ACTION: ActionDefinition = {
    id: 'tab.new',
    label: 'new_tab',
    icon: Plus,
    shortcut: 'Ctrl+T',
    isVisible: () => true,
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
