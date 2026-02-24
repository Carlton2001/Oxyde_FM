import React from 'react';

interface SelectionMarqueeProps {
    selectionRect: { x: number; y: number; w: number; h: number; active: boolean } | null;
    containerRef: React.RefObject<HTMLDivElement>;
}

export const SelectionMarquee: React.FC<SelectionMarqueeProps> = React.memo(({ selectionRect, containerRef }) => {
    if (!selectionRect) return null;

    const scrollContainer = containerRef.current?.querySelector('.virtualized-list > div');
    const scrollLeft = scrollContainer?.scrollLeft || 0;
    const scrollTop = scrollContainer?.scrollTop || 0;

    return (
        <div
            className="selection-rect"
            style={{
                left: (selectionRect.w >= 0 ? selectionRect.x : selectionRect.x + selectionRect.w) - scrollLeft,
                top: (selectionRect.h >= 0 ? selectionRect.y : selectionRect.y + selectionRect.h) - scrollTop,
                width: Math.abs(selectionRect.w),
                height: Math.abs(selectionRect.h)
            }}
        />
    );
});

