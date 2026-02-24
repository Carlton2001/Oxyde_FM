import { useState } from 'react';
import { FileEntry, PanelId } from '../types';
import { isSameVolume } from '../utils/path';

export const useDragDrop = (
    onDropFile: (action: 'copy' | 'move', source: FileEntry[], target: string) => void
) => {
    const [dragState, setDragState] = useState<{ sourcePanel: PanelId; files: FileEntry[] } | null>(null);
    const [dragOverPath, setDragOverPath] = useState<string | null>(null);
    const [dragTargetPath, setDragTargetPath] = useState<string | null>(null);

    const handleDragStart = (sourcePanel: PanelId, files: FileEntry[]) => {
        setDragState({ sourcePanel, files });
    };

    const handleDrop = (e: React.DragEvent | any, targetPath: string | null, currentPath: string) => {
        if (e?.preventDefault) e.preventDefault();
        if (e?.stopPropagation) e.stopPropagation();
        setDragOverPath(null);
        setDragTargetPath(null);

        if (!dragState || !dragState.files.length) return;

        const target = targetPath || currentPath;
        if (!target) return;

        // Default logic: Move on same volume, Copy on different volumes
        const sourcePath = dragState.files[0].path;
        const sameVolume = isSameVolume(sourcePath, target);

        let action: 'copy' | 'move' = sameVolume ? 'move' : 'copy';

        // Modifiers override everything
        if (e?.shiftKey) action = 'move';
        if (e?.ctrlKey) action = 'copy';

        // Check if target is one of the source strings
        if (dragState.files.some(f => f.path === target)) return;

        onDropFile(action, dragState.files, target);

        setDragState(null);
    };

    return {
        dragState,
        dragOverPath,
        dragTargetPath,
        setDragOverPath,
        setDragTargetPath,
        handleDragStart,
        handleDrop,
        setDragState
    };
};
