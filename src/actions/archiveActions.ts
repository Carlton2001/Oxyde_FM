
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

        try {
            await invoke('extract_archive', { archivePath, targetDir });
            // Progress and refresh are handled via file_op_event
        } catch (e) {
            ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
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

        try {
            await invoke('extract_archive', { archivePath, targetDir });
            // Progress and refresh are handled via file_op_event
        } catch (e) {
            ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
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

    if (selection.length === 0 && target) {
        selection = [target];
    }

    if (selection.length === 0) return;

    const firstItem = selection[0];
    const parentDir = getParent(firstItem);
    const existingNames = ctx.activePanel.files.map(f => f.name);

    let defaultBase = selection.length === 1
        ? (firstItem.split('\\').pop()?.split('/').pop()?.split('.').shift() || "Archive")
        : (parentDir?.split('\\').pop()?.split('/').pop() || "Archive");

    defaultBase = getUniqueArchiveName(defaultBase, format, existingNames);

    const userBaseName = await ctx.dialogs.prompt(
        ctx.t('enter_archive_name') || "Enter archive name",
        ctx.t('compress'),
        defaultBase
    );

    if (!userBaseName) return;

    const finalBase = getUniqueArchiveName(userBaseName.trim(), format, existingNames);

    const archiveName = format === 'zst' ? `${finalBase}.tar.zst` : `${finalBase}.${format}`;
    const archivePath = `${parentDir || ''}${parentDir?.endsWith('\\') ? '' : '\\'}${archiveName}`;

    let quality = 'normal';

    try {
        await invoke('compress_to_archive', { paths: selection, archivePath, format, quality });
        // Progress and refresh are handled via file_op_event
    } catch (e) {
        ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
    }
};

export const COMPRESS_ZIP_ACTION: ActionDefinition = {
    id: 'archive.compress_zip',
    label: 'Zip',
    icon: FileArchive,
    isEnabled: (ctx) => ctx.supportedFormats.includes('zip'),
    handler: (ctx) => compress(ctx, 'zip')
};

export const COMPRESS_7Z_ACTION: ActionDefinition = {
    id: 'archive.compress_7z',
    label: '7z',
    icon: FileArchive,
    isEnabled: (ctx) => ctx.supportedFormats.includes('7z'),
    handler: (ctx) => compress(ctx, '7z')
};

export const COMPRESS_TAR_ACTION: ActionDefinition = {
    id: 'archive.compress_tar',
    label: 'Tar',
    icon: FileArchive,
    isEnabled: (ctx) => ctx.supportedFormats.includes('tar'),
    handler: (ctx) => compress(ctx, 'tar')
};

export const COMPRESS_ZST_ACTION: ActionDefinition = {
    id: 'archive.compress_zst',
    label: 'Zstd',
    icon: FileArchive,
    isEnabled: (ctx) => ctx.supportedFormats.includes('zst'),
    handler: (ctx) => compress(ctx, 'zst')
};
