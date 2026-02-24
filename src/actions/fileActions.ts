import { ActionDefinition, ActionContext } from '../types/actions';
import { invoke } from '@tauri-apps/api/core';
import {
    Copy, Scissors, ClipboardPaste, Trash2, Edit2,
    FolderPlus, Info, Undo, Redo, ExternalLink
} from 'lucide-react';
import { formatCommandError } from '../utils/error';
import { getParent } from '../utils/path';

export const UNDO_ACTION: ActionDefinition = {
    id: 'file.undo',
    label: 'undo',
    getLabel: (ctx: ActionContext) => {
        const stack = ctx.fileOps.historyState.undo_stack;
        if (stack.length > 0) {
            const tx = stack[stack.length - 1];
            let key = tx.op_type.toLowerCase();
            if (key === 'newfolder') key = 'new_folder';
            return `${ctx.t('undo')} ${ctx.t(`op_${key}`)}`;
        }
        return ctx.t('undo');
    },
    icon: Undo,
    shortcut: 'Ctrl+Z',
    isEnabled: (ctx: ActionContext) => ctx.fileOps.canUndo,
    handler: async (ctx: ActionContext) => {
        try {
            const tx = await ctx.fileOps.undo();
            if (tx) {
                let key = tx.op_type.toLowerCase();
                if (key === 'newfolder') key = 'new_folder';
                ctx.notify(`${ctx.t('undo_action')} ${ctx.t(`op_${key}`)}`, 'success', 2000);
                if (ctx.refreshBothPanels) ctx.refreshBothPanels();

                // Refresh affected tree paths
                if (tx.details.target_dir) {
                    ctx.refreshTreePath?.(tx.details.target_dir);
                }
                if (tx.details.paths && tx.details.paths.length > 0) {
                    const uniqueParents = new Set<string>(
                        tx.details.paths
                            .map((p: string) => getParent(p))
                            .filter((p: string | null): p is string => !!p)
                    );
                    uniqueParents.forEach(parent => ctx.refreshTreePath?.(parent));
                }
                if (tx.details.old_path) {
                    const parent = getParent(tx.details.old_path);
                    if (parent) ctx.refreshTreePath?.(parent);
                }
                if (tx.details.new_path) {
                    const parent = getParent(tx.details.new_path);
                    if (parent) ctx.refreshTreePath?.(parent);
                }
            }
        } catch (e) {
            ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }
};

export const REDO_ACTION: ActionDefinition = {
    id: 'file.redo',
    label: 'redo',
    getLabel: (ctx: ActionContext) => {
        const stack = ctx.fileOps.historyState.redo_stack;
        if (stack.length > 0) {
            const tx = stack[stack.length - 1];
            let key = tx.op_type.toLowerCase();
            if (key === 'newfolder') key = 'new_folder';
            return `${ctx.t('redo')} ${ctx.t(`op_${key}`)}`;
        }
        return ctx.t('redo');
    },
    icon: Redo,
    shortcut: 'Ctrl+Y',
    isEnabled: (ctx: ActionContext) => ctx.fileOps.canRedo,
    handler: async (ctx: ActionContext) => {
        try {
            const tx = await ctx.fileOps.redo();
            if (tx) {
                let key = tx.op_type.toLowerCase();
                if (key === 'newfolder') key = 'new_folder';
                ctx.notify(`${ctx.t('redo_action')} ${ctx.t(`op_${key}`)}`, 'success', 2000);
                if (ctx.refreshBothPanels) ctx.refreshBothPanels();

                // Refresh affected tree paths
                if (tx.details.target_dir) {
                    ctx.refreshTreePath?.(tx.details.target_dir);
                }
                if (tx.details.paths && tx.details.paths.length > 0) {
                    const uniqueParents = new Set<string>(
                        tx.details.paths
                            .map((p: string) => getParent(p))
                            .filter((p: string | null): p is string => !!p)
                    );
                    uniqueParents.forEach(parent => ctx.refreshTreePath?.(parent));
                }
                if (tx.details.old_path) {
                    const parent = getParent(tx.details.old_path);
                    if (parent) ctx.refreshTreePath?.(parent);
                }
                if (tx.details.new_path) {
                    const parent = getParent(tx.details.new_path);
                    if (parent) ctx.refreshTreePath?.(parent);
                }
            }
        } catch (e) {
            ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }
};

export const COPY_ACTION: ActionDefinition = {
    id: 'file.copy',
    label: 'copy',
    icon: Copy,
    shortcut: 'Ctrl+C',
    isEnabled: (ctx: ActionContext) => ctx.activePanel.selected.size > 0,
    handler: async (ctx: ActionContext) => {
        const selection = Array.from(ctx.activePanel.selected);
        if (selection.length > 0) {
            try {
                await ctx.clipboard.copy(selection);
                ctx.notify(ctx.t('copied_to_clipboard'), 'info', 2000);
            } catch (e) {
                ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
            }
        }
    }
};

export const CUT_ACTION: ActionDefinition = {
    id: 'file.cut',
    label: 'cut',
    icon: Scissors,
    shortcut: 'Ctrl+X',
    isEnabled: (ctx: ActionContext) => ctx.activePanel.selected.size > 0 || !!ctx['contextMenuTarget'],
    handler: async (ctx: ActionContext) => {
        const { activePanel, clipboard, notify, t } = ctx;
        const selection = Array.from(activePanel.selected);
        const targetPath = ctx['contextMenuTarget'] as string | undefined;
        const finalPaths = selection.length > 0 ? selection : (targetPath ? [targetPath] : []);

        if (finalPaths.length === 0) return;

        const isInTrash = activePanel.path?.startsWith('trash://');

        if (isInTrash) {
            try {
                // Special trash cut logic
                await invoke('set_clipboard_from_trash', { trash_paths: finalPaths });
                await clipboard.refreshClipboard();
                notify(`${t('cut_to_clipboard')} (${t('restored')})`, 'info', 2000);
                if (ctx.refreshBothPanels) ctx.refreshBothPanels();
            } catch (e) {
                notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
            }
        } else {
            try {
                await clipboard.cut(finalPaths);
                notify(t('cut_to_clipboard'), 'info', 2000);
            } catch (e) {
                notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
            }
        }
    }
};

export const PASTE_ACTION: ActionDefinition = {
    id: 'file.paste',
    label: 'paste',
    icon: ClipboardPaste,
    shortcut: 'Ctrl+V',
    isEnabled: () => true,
    handler: async (ctx: ActionContext) => {
        const { activePanel, fileOps, clipboard, notify, t, refreshBothPanels } = ctx;

        try {
            const [files, isCut] = await invoke<[string[], boolean]>('get_clipboard_files');

            if (files && files.length > 0) {
                const targetPath = activePanel.path;
                // Check if any files are from trash
                const isFromTrash = files.some(f =>
                    f.toLowerCase().includes('$recycle.bin') ||
                    f.toLowerCase().includes('$r')
                );

                if (isFromTrash && isCut) {
                    await invoke('move_from_trash', { paths: files, targetDir: targetPath });
                    const count = files.length;
                    notify(`${count} ${count > 1 ? t('items') : t('item')} ${t('restored')}`, 'success');
                    await clipboard.clearClipboard();
                    if (refreshBothPanels) refreshBothPanels();
                } else {
                    const success = await fileOps.initiateFileOp(
                        isCut ? 'move' : 'copy',
                        files,
                        targetPath,
                        ctx.settings['defaultTurboMode']
                    );

                    if (success) {
                        if (isCut) {
                            await clipboard.clearClipboard();
                        }
                        if (refreshBothPanels) refreshBothPanels();
                    }
                }
            }
        } catch (e) {
            console.error("Paste failed", e);
            notify(`${t('error')}: ${formatCommandError(e)}`, 'error');
        }
    }
};

export const DELETE_ACTION: ActionDefinition = {
    id: 'file.delete',
    label: 'delete',
    getLabel: (ctx: ActionContext) => {
        const isTrashView = ctx.activePanel.path?.startsWith('trash://') || false;
        if (ctx.modifiers?.shift || isTrashView) return 'perm_delete';
        return 'delete';
    },
    icon: Trash2,
    shortcut: 'Delete',
    isEnabled: (ctx: ActionContext) => ctx.activePanel.selected.size > 0 || !!ctx['contextMenuTarget'],
    handler: async (ctx: ActionContext) => {
        const selection = Array.from(ctx.activePanel.selected);
        const targetPath = ctx['contextMenuTarget'] as string | undefined;
        const finalPaths = selection.length > 0 ? selection : (targetPath ? [targetPath] : []);

        if (finalPaths.length > 0) {
            const isTrashView = ctx.activePanel.path?.startsWith('trash://') || false;
            const isPermanent = ctx.modifiers?.shift || isTrashView;

            ctx.dialogs.openDeleteDialog({
                paths: finalPaths,
                isPermanent,
                onConfirm: async () => {
                    try {
                        await ctx.fileOps.deleteItems(finalPaths, isPermanent, ctx.settings['defaultTurboMode']);

                        // If we deleted from tree, refresh the parent in tree
                        if (targetPath && ctx.refreshTreePath) {
                            const parent = getParent(targetPath);
                            if (parent) ctx.refreshTreePath(parent);
                        }

                        // Fallback navigation for both panels
                        [ctx.activePanel, ctx.otherPanel].forEach(panel => {
                            if (!panel) return;
                            const path = panel.path;
                            if (path) {
                                const isDeleted = finalPaths.some(p =>
                                    path === p || path.startsWith(p.endsWith('\\') ? p : p + '\\')
                                );
                                if (isDeleted) {
                                    const parent = getParent(finalPaths[0]);
                                    if (parent) panel.navigate(parent);
                                }
                            }
                        });

                        if (ctx.refreshBothPanels) ctx.refreshBothPanels();
                    } catch (e) {
                        ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
                    }
                }
            });
        }
    }
};

export const RENAME_ACTION: ActionDefinition = {
    id: 'file.rename',
    label: 'rename',
    icon: Edit2,
    shortcut: 'F2',
    isEnabled: (ctx: ActionContext) => ctx.activePanel.selected.size === 1 || !!ctx['renameValue'] || !!ctx['contextMenuTarget'],
    handler: async (ctx: ActionContext) => {
        const selection = Array.from(ctx.activePanel.selected);
        const renameValue = ctx['renameValue'] as string | undefined;
        const targetPath = (ctx['contextMenuTarget'] as string | undefined) || (selection.length === 1 ? selection[0] : null);

        if (!targetPath) return;

        const performRename = async (newName: string) => {
            if (newName && newName !== targetPath) {
                try {
                    // Ensure we have an absolute path for the target
                    // If newName is just a filename, prepend the parent directory
                    let finalNewPath = newName;
                    if (!newName.includes('\\') && !newName.includes('/')) {
                        const parent = targetPath.substring(0, targetPath.lastIndexOf('\\') + 1);
                        finalNewPath = `${parent}${newName}`;
                    }

                    await ctx.fileOps.renameItem(targetPath, finalNewPath);
                    if (ctx.refreshBothPanels) ctx.refreshBothPanels();

                    // Update panel paths if renamed
                    [ctx.activePanel, ctx.otherPanel].forEach(panel => {
                        if (!panel) return;
                        const path = panel.path;
                        if (path && (path === targetPath || path.startsWith(targetPath.endsWith('\\') ? targetPath : targetPath + '\\'))) {
                            const relative = path.slice(targetPath.length);
                            panel.navigate(finalNewPath + relative);
                        }
                    });

                    // If we renamed from tree, refresh the parent in tree
                    if (ctx['contextMenuTarget'] && ctx.refreshTreePath) {
                        const parent = getParent(targetPath);
                        if (parent) ctx.refreshTreePath(parent);
                    }
                } catch (e) {
                    ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
                }
            }
        };

        if (renameValue) {
            await performRename(renameValue);
        } else {
            ctx.dialogs.openRenameDialog(targetPath).then((val) => {
                if (val) performRename(val);
            });
        }
    }
};

export const NEW_FOLDER_ACTION: ActionDefinition = {
    id: 'file.new_folder',
    label: 'new_folder',
    icon: FolderPlus,
    handler: (ctx: ActionContext) => {
        const targetPath = ctx['contextMenuTarget'] as string | undefined;
        const parentPath = targetPath || ctx.activePanel.path;

        ctx.dialogs.openNewFolderDialog({
            parentPath: parentPath,
            onCreate: async (name) => {
                const sep = parentPath.endsWith('\\') ? '' : '\\';
                const cleanName = name.trim().replace(/[. ]+$/, '');
                if (!cleanName) return;

                const newPath = `${parentPath}${sep}${cleanName}`;
                try {
                    await ctx.fileOps.createFolder(newPath);
                    if (ctx.refreshBothPanels) ctx.refreshBothPanels();

                    // Refresh tree if created under a specific tree node
                    if (targetPath && ctx.refreshTreePath) {
                        ctx.refreshTreePath(targetPath);
                    }
                } catch (e) {
                    ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
                }
            }
        });
    }
};

export const PROPERTIES_ACTION: ActionDefinition = {
    id: 'file.properties',
    label: 'properties',
    icon: Info,
    shortcut: 'Alt+Enter',
    handler: (ctx: ActionContext) => {
        const selection = Array.from(ctx.activePanel.selected);
        const paths = selection.length > 0 ? selection : [ctx.activePanel.path];
        ctx.dialogs.openPropertiesDialog(paths);
    }
};

export const COPY_PATH_ACTION: ActionDefinition = {
    id: 'file.copy_path',
    label: 'copy_path',
    handler: async (ctx: ActionContext) => {
        const selection = Array.from(ctx.activePanel.selected);
        if (selection.length > 0) {
            const paths = selection.join('\n');
            await ctx.clipboard.copyToSystem(paths);
            ctx.notify(`${ctx.t('copy_path')}: ${selection.length}`, 'success', 2000);
        }
    }
};

export const COPY_NAME_ACTION: ActionDefinition = {
    id: 'file.copy_name',
    label: 'copy_name',
    handler: async (ctx: ActionContext) => {
        const selection = Array.from(ctx.activePanel.selected);
        if (selection.length > 0) {
            const names = selection.map(p => p.split('\\').pop() || '').join('\n');
            await ctx.clipboard.copyToSystem(names);
            ctx.notify(`${ctx.t('copy_name')}: ${selection.length}`, 'success', 2000);
        }
    }
};

export const OPEN_ACTION: ActionDefinition = {
    id: 'file.open',
    label: 'open',
    icon: ExternalLink,
    shortcut: 'Enter',
    isEnabled: (ctx: ActionContext) => ctx.activePanel.selected.size === 1 || !!ctx['contextMenuTarget'],
    handler: async (ctx: ActionContext) => {
        const selection = Array.from(ctx.activePanel.selected);
        const targetPath = (ctx['contextMenuTarget'] as string | undefined) || (selection.length === 1 ? selection[0] : null);

        if (!targetPath) return;

        const fileEntry = ctx.activePanel.files.find(f => f.path === targetPath) || ctx.activePanel.searchResults?.find(f => f.path === targetPath);

        if (fileEntry && fileEntry.is_dir) {
            ctx.activePanel.navigate(targetPath);
        } else {
            // It's a file, invoke the system open command
            try {
                await invoke('open_item', { path: targetPath });
            } catch (e) {
                ctx.notify(`${ctx.t('error')}: ${formatCommandError(e)}`, 'error');
            }
        }
    }
};
