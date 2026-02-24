import { invoke } from '@tauri-apps/api/core';
import { FileEntry, NotificationType } from '../types';
import { getParent } from '../utils/path';
import { isArchivePath, isSupportedArchiveForAdding } from '../utils/archive';
import { formatCommandError } from '../utils/error';
import { TFunc } from '../i18n';
import { ProgressState } from '../components/ui/ProgressOverlay';

interface UseFileDropProps {
    t: TFunc;
    notify: (message: string, type: NotificationType, duration?: number) => void;

    refreshBothPanels: () => void;
    refreshTreePath: (path: string) => void;
    setProgress: (progress: ProgressState | null) => void;
    initiateFileOp: (action: 'copy' | 'move', paths: string[], targetDir: string, turbo?: boolean) => Promise<boolean>;
    defaultTurboMode?: boolean;
}

export const useFileDrop = ({
    t,
    notify,

    refreshBothPanels,
    refreshTreePath,
    setProgress,
    initiateFileOp,
    defaultTurboMode
}: UseFileDropProps) => {

    const onDropFile = async (action: 'copy' | 'move', files: FileEntry[], targetPath: string) => {
        const filePaths = files.map(f => f.path);

        // Check if dragging from Trash
        const isFromTrash = files.some(f => f.original_path || f.path?.startsWith('trash://'));

        if (isFromTrash) {
            // Only allow "Moving/Restoring" from trash
            setProgress({ visible: true, message: t('restoring') + '...' });
            try {
                await invoke('move_from_trash', { paths: filePaths, targetDir: targetPath });
                const itemCount = files.length;
                const isPlural = itemCount > 1;
                const itemText = isPlural ? t('items') : t('item');
                notify(`${itemCount} ${itemText} ${t('restored')} `, 'success');


                refreshBothPanels();
                refreshTreePath(targetPath);
            } catch (e) {
                console.error("Failed to restore from trash via DnD", e);
                notify(`${t('error')}: ${formatCommandError(e)} `, 'error');
            } finally {
                setProgress(null);
            }
            return;
        }

        if (action === 'move' && filePaths.includes(targetPath)) return;
        if (action === 'move' && filePaths.every(f => getParent(f) === targetPath)) return;

        // Check if target is an ARCHIVE (supported for adding)
        if (isArchivePath(targetPath) && isSupportedArchiveForAdding(targetPath)) {
            setProgress({ visible: true, message: t('adding_to_archive') + '...' });
            try {
                await invoke('add_to_archive', { paths: filePaths, archivePath: targetPath });
                notify(t('added_to_archive_success'), 'success');
                refreshBothPanels();
            } catch (e: any) {
                console.error("Failed to add to archive via DnD", e);
                const errorMsg = formatCommandError(e);
                if (errorMsg.includes('Duplicate filename')) {
                    notify(t('archive_duplicate_error'), 'error');
                } else {
                    notify(`${t('error')}: ${errorMsg} `, 'error');
                }
            } finally {
                setProgress(null);
            }
            return;
        }

        try {
            await initiateFileOp(action, filePaths, targetPath, defaultTurboMode);
        } catch (e) {
            console.error("Initiate file op failed in DnD", e);
        }
    };

    return { onDropFile };
};
