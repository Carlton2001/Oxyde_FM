
import { ActionDefinition, ActionContext } from '../types/actions';
import { invoke } from '@tauri-apps/api/core';
import { getParent } from '../utils/path';
import { FileArchive, FolderArchive } from 'lucide-react';
import { formatCommandError } from '../utils/error';

export const EXTRACT_HERE_ACTION: ActionDefinition = {
    id: 'archive.extract_here',
    label: 'extract_here',
    icon: FolderArchive,
    isEnabled: (ctx) => ctx.activePanel.selected.size === 1 || !!ctx['contextMenuTarget'],
    handler: async (ctx) => {
        const target = ctx['contextMenuTarget'] || (ctx.activePanel.selected.size === 1 ? Array.from(ctx.activePanel.selected)[0] : null);
        if (!target) return;

        const archivePath = target;
        const targetDir = getParent(archivePath);

        if (ctx.setProgress) {
            ctx.setProgress({ visible: true, message: ctx.t('calculating'), cancellable: true });
        } else {
            ctx.notify(ctx.t('calculating'), 'info');
        }

        try {
            await invoke('extract_archive', { archivePath, targetDir });
            ctx.notify(ctx.t('item_restored'), 'success');
            if (typeof ctx.activePanel.refresh === 'function') ctx.activePanel.refresh();
            if (ctx.otherPanel && typeof ctx.otherPanel.refresh === 'function') ctx.otherPanel.refresh();
        } catch (e) {
            if (e === 'Cancelled') {
                ctx.notify(ctx.t('op_cancelled') || "Operation cancelled", 'info');
            } else {
                ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
            }
        } finally {
            if (ctx.setProgress) ctx.setProgress(null);
        }
    }
};

export const EXTRACT_TO_FOLDER_ACTION: ActionDefinition = {
    id: 'archive.extract_to_folder',
    label: 'extract_to_folder',
    icon: FolderArchive,
    isEnabled: (ctx) => ctx.activePanel.selected.size === 1 || !!ctx['contextMenuTarget'],
    handler: async (ctx) => {
        const target = ctx['contextMenuTarget'] || (ctx.activePanel.selected.size === 1 ? Array.from(ctx.activePanel.selected)[0] : null);
        if (!target) return;

        const archivePath = target;
        let targetDir = getParent(archivePath);
        const folderName = archivePath.split('\\').pop()?.split('/').pop()?.split('.').shift() || "extracted";
        targetDir = `${targetDir}\\${folderName}`;

        if (ctx.setProgress) {
            ctx.setProgress({ visible: true, message: ctx.t('calculating'), cancellable: true });
        } else {
            ctx.notify(ctx.t('calculating'), 'info');
        }

        try {
            await invoke('extract_archive', { archivePath, targetDir });
            ctx.notify(ctx.t('item_restored'), 'success');
            if (typeof ctx.activePanel.refresh === 'function') ctx.activePanel.refresh();
            if (ctx.otherPanel && typeof ctx.otherPanel.refresh === 'function') ctx.otherPanel.refresh();
        } catch (e) {
            if (e === 'Cancelled') {
                ctx.notify(ctx.t('op_cancelled') || "Operation cancelled", 'info');
            } else {
                ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
            }
        } finally {
            if (ctx.setProgress) ctx.setProgress(null);
        }
    }
};

const getUniqueArchiveName = (base: string, format: string, existingNames: string[]): string => {
    const namesLower = new Set(existingNames.map(n => n.toLowerCase()));
    const ext = format === 'zst' ? '.tar.zst' : `.${format}`;
    let currentName = `${base}${ext}`;

    if (!namesLower.has(currentName.toLowerCase())) {
        return base;
    }

    let counter = 2;
    while (namesLower.has(`${base} (${counter})${ext}`.toLowerCase())) {
        counter++;
    }
    return `${base} (${counter})`;
};

const compress = async (ctx: ActionContext, format: 'zip' | '7z' | 'tar' | 'zst') => {
    let selection = Array.from(ctx.activePanel.selected);
    const target = ctx['contextMenuTarget'];

    // If no selection but we have a target (context menu on unselected item), use target
    if (selection.length === 0 && target) {
        selection = [target];
    }

    if (selection.length === 0) return;

    const firstItem = selection[0];
    const parentDir = getParent(firstItem);
    const existingNames = ctx.activePanel.files.map(f => f.name);

    let defaultBase = selection.length === 1
        ? (firstItem.split('\\').pop()?.split('/').pop()?.split('.').shift() || "Archive")
        : (parentDir.split('\\').pop()?.split('/').pop() || "Archive");

    // Suggest a unique name by default
    defaultBase = getUniqueArchiveName(defaultBase, format, existingNames);

    // Ask user for name
    const userBaseName = await ctx.dialogs.prompt(
        ctx.t('enter_archive_name') || "Enter archive name",
        ctx.t('compress'),
        defaultBase
    );

    if (!userBaseName) return; // Cancelled

    // Double check uniqueness for the final user input (if they manually typed an existing name)
    const finalBase = getUniqueArchiveName(userBaseName.trim(), format, existingNames);

    const archiveName = format === 'zst' ? `${finalBase}.tar.zst` : `${finalBase}.${format}`;
    const archivePath = `${parentDir}${parentDir.endsWith('\\') ? '' : '\\'}${archiveName}`;

    // Quality defaults from settings
    let quality = 'normal';

    if (ctx.settings) {
        if (format === 'zip') quality = ctx.settings.zipQuality;
        else if (format === '7z') quality = ctx.settings.sevenZipQuality;
        else if (format === 'zst') quality = ctx.settings.zstdQuality;
    }

    if (ctx.setProgress) {
        ctx.setProgress({ visible: true, message: `${ctx.t('compress')}...`, cancellable: true });
    } else {
        ctx.notify(`${ctx.t('compress')}...`, 'info');
    }

    try {
        await invoke('compress_to_archive', { paths: selection, archivePath, format, quality });
        ctx.notify(ctx.t('item_created') || "Archive created", 'success');
        if (typeof ctx.activePanel.refresh === 'function') ctx.activePanel.refresh();
        if (ctx.otherPanel && typeof ctx.otherPanel.refresh === 'function') ctx.otherPanel.refresh();
    } catch (e) {
        if (e === 'Cancelled') {
            ctx.notify(ctx.t('op_cancelled') || "Operation cancelled", 'info');
        } else {
            ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
        }
    } finally {
        if (ctx.setProgress) ctx.setProgress(null);
    }
};

export const COMPRESS_ZIP_ACTION: ActionDefinition = {
    id: 'archive.compress_zip',
    label: 'Zip',
    icon: FileArchive,
    handler: (ctx) => compress(ctx, 'zip')
};

export const COMPRESS_7Z_ACTION: ActionDefinition = {
    id: 'archive.compress_7z',
    label: '7z',
    icon: FileArchive,
    handler: (ctx) => compress(ctx, '7z')
};

export const COMPRESS_TAR_ACTION: ActionDefinition = {
    id: 'archive.compress_tar',
    label: 'Tar',
    icon: FileArchive,
    handler: (ctx) => compress(ctx, 'tar')
};

export const COMPRESS_ZST_ACTION: ActionDefinition = {
    id: 'archive.compress_zst',
    label: 'Zstd',
    icon: FileArchive,
    handler: (ctx) => compress(ctx, 'zst')
};
