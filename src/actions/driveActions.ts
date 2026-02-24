
import { ActionDefinition } from '../types/actions';
import { invoke } from '@tauri-apps/api/core';
import { HardDrive } from 'lucide-react';
import { formatCommandError } from '../utils/error';

export const MOUNT_IMAGE_ACTION: ActionDefinition = {
    id: 'drive.mount_image',
    label: 'mount',
    icon: HardDrive,
    // Visible only if selected item is disk image
    isVisible: (ctx) => {
        // Can be context menu target OR selected item
        const target = ctx['contextMenuTarget'] || (ctx.activePanel.selected.size === 1 ? Array.from(ctx.activePanel.selected)[0] : null);
        if (!target) return false;
        const ext = target.split('.').pop()?.toLowerCase();
        return ['iso', 'img', 'vhd', 'vhdx'].includes(ext || '');
    },
    handler: async (ctx) => {
        const target = ctx['contextMenuTarget'] || (ctx.activePanel.selected.size === 1 ? Array.from(ctx.activePanel.selected)[0] : null);
        if (!target) return;

        try {
            await invoke('mount_disk_image', { path: target });
            ctx.notify(ctx.t('mount_success') || "Mounted successfully", 'success');
            // We need to refresh Drives. ActionContext doesn't have refreshDrives?
            // "refreshDrives" comes from useApp.
            if (typeof ctx['refreshDrives'] === 'function') ctx['refreshDrives']();
            if (typeof ctx.activePanel.refresh === 'function') ctx.activePanel.refresh();
        } catch (e) {
            ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }
};

export const UNMOUNT_IMAGE_ACTION: ActionDefinition = {
    id: 'drive.unmount_image',
    label: 'unmount',
    icon: HardDrive,
    // Visible only if current item or target is a mount point or virtual drive
    isVisible: (ctx) => {
        const target = ctx['contextMenuTarget'] || ctx.activePanel.path;
        if (!target) return false;

        // Condition A: It's a drive (sidebar or background of root)
        const isDrive = ctx['isDrive'] || /^[a-zA-Z]:[\\/]?$/.test(target);
        if (isDrive) {
            // Skip C: drive
            if (target.toLowerCase().startsWith('c:')) return false;
            // Check for root path format (X:\ or X:)
            return /^[a-zA-Z]:[\\/]?$/.test(target);
        }

        // Condition B: It's an image file that IS mounted
        const mountedImages = ctx['mountedImages'] as string[] || [];
        const normTarget = target.toLowerCase().replace(/\\/g, '/');
        if (mountedImages.some(img => img.toLowerCase().replace(/\\/g, '/') === normTarget)) {
            return true;
        }

        return false;
    },
    handler: async (ctx) => {
        const target = ctx['contextMenuTarget'] || ctx.activePanel.path;
        if (!target) return;

        try {
            // Backend handles session state (closing tabs) atomically before unmount.
            // If it's a drive-like path, use the 3-char root.
            // If it's a file path, use the full path.
            const isDrive = /^[a-zA-Z]:[\\/]?$/.test(target);
            const finalPath = isDrive ? target.substring(0, 3) : target;

            await invoke('unmount_disk_image', { path: finalPath });
            ctx.notify(ctx.t('unmount_success') || "Unmounted successfully", 'success');

            if (typeof ctx['refreshDrives'] === 'function') ctx['refreshDrives']();
        } catch (e) {
            ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }
};
