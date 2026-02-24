import React from 'react';
import { ColumnWidths, FileEntry } from '../../types';
import { useAutoFitColumns } from '../../hooks/useAutoFitColumns';

interface ResizeHandleProps {
    field: keyof ColumnWidths;
    panelRef: React.RefObject<HTMLDivElement | null>;
    onResize: (field: keyof ColumnWidths, size: number) => void;
    onResizeMultiple?: (updates: Partial<ColumnWidths>) => void;
    colWidths: ColumnWidths;
    files: FileEntry[];
    searchResults: boolean;
    isTrashView?: boolean;
    t: any;
}

export const ResizeHandle: React.FC<ResizeHandleProps> = ({
    field,
    panelRef,
    onResize,
    onResizeMultiple,
    colWidths,
    files,
    searchResults,
    isTrashView = false,
    t
}) => {
    const autoFit = useAutoFitColumns({
        panelRef, files, searchResults, isTrashView, t,
        onResizeMultiple, onResize
    });

    return (
        <div
            className="resize-handle"
            onClick={(e) => e.stopPropagation()}
            onDoubleClick={(e) => {
                e.stopPropagation();
                autoFit();
            }}
            onMouseDown={(e) => {
                e.stopPropagation();

                const startX = e.clientX;
                const startWidth = colWidths[field];
                let finalWidth = startWidth;

                document.body.style.userSelect = 'none';
                document.body.classList.add('resizing');

                const dynamicMinWidth = 20;

                let rafId: number | null = null;
                const onMove = (mv: MouseEvent) => {
                    const delta = mv.clientX - startX;
                    finalWidth = Math.max(dynamicMinWidth, startWidth + delta);

                    if (rafId) cancelAnimationFrame(rafId);
                    rafId = requestAnimationFrame(() => {
                        onResize(field, finalWidth);
                    });
                };

                const onUp = () => {
                    if (rafId) cancelAnimationFrame(rafId);
                    document.body.style.userSelect = '';
                    document.body.classList.remove('resizing');
                    window.removeEventListener('mousemove', onMove);
                    window.removeEventListener('mouseup', onUp);
                    onResize(field, finalWidth);
                };

                window.addEventListener('mousemove', onMove);
                window.addEventListener('mouseup', onUp);
            }}
        />
    );
};

