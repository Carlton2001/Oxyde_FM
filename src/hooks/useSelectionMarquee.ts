import { useState, useRef, useCallback } from 'react';

interface Rect {
    x: number;
    y: number;
    w: number;
    h: number;
    active: boolean;
}

interface ItemRect {
    path: string;
    rect: {
        left: number;
        top: number;
        right: number;
        bottom: number;
    };
}

export const useSelectionMarquee = (
    containerRef: React.RefObject<HTMLDivElement>,
    onSelectMultiple: (paths: string[], additive: boolean) => void,
    onClearSelection: () => void,
    onActivate: () => void,
    isDragging: boolean,
    renamingPath: string | null,
    cancelRename: () => void
) => {
    const [selectionRect, setSelectionRect] = useState<Rect | null>(null);
    const [pendingSelection, setPendingSelection] = useState<Set<string>>(new Set());
    const isMarqueeRef = useRef(false);
    const pendingSelectionRef = useRef<Set<string>>(new Set());
    const itemRectsCache = useRef<ItemRect[]>([]);

    const handleMouseDown = useCallback((e: React.MouseEvent) => {
        if ((e.target as HTMLElement).tagName === 'INPUT') return;
        if (e.button !== 0 || isDragging) return;

        const isFileItem = (e.target as HTMLElement).closest('.file-item');
        if (isFileItem) return;

        if (renamingPath) cancelRename();
        onActivate();

        const scrollContainer = (containerRef.current?.querySelector('.virtualized-list > div') || containerRef.current) as HTMLElement;
        if (!scrollContainer) return;

        const rect = containerRef.current!.getBoundingClientRect();
        const startX = e.clientX - rect.left + scrollContainer.scrollLeft;
        const startY = e.clientY - rect.top + scrollContainer.scrollTop;

        // Pre-calculate visible item positions
        const items = containerRef.current!.querySelectorAll('.file-item');
        itemRectsCache.current = Array.from(items).map(item => {
            const itemElem = item as HTMLElement;
            const itemRect = itemElem.getBoundingClientRect();
            return {
                path: itemElem.getAttribute('data-path') || '',
                rect: {
                    left: itemRect.left - rect.left + scrollContainer.scrollLeft,
                    top: itemRect.top - rect.top + scrollContainer.scrollTop,
                    right: itemRect.right - rect.left + scrollContainer.scrollLeft,
                    bottom: itemRect.bottom - rect.top + scrollContainer.scrollTop
                }
            };
        }).filter(i => i.path);

        setSelectionRect({ x: startX, y: startY, w: 0, h: 0, active: true });
        setPendingSelection(new Set());
        pendingSelectionRef.current = new Set();
        isMarqueeRef.current = false;

        const onMouseMove = (mv: MouseEvent) => {
            if (!containerRef.current || isDragging) return;

            if (Math.abs(mv.clientX - e.clientX) > 4 || Math.abs(mv.clientY - e.clientY) > 4) {
                isMarqueeRef.current = true;
                if (renamingPath) cancelRename();
            }

            const SCROLL_EDGE_THRESHOLD = 40;
            const SCROLL_SPEED = 15;
            const cursorY = mv.clientY - rect.top;
            const containerHeight = containerRef.current.clientHeight;
            let didScroll = false;

            if (cursorY < SCROLL_EDGE_THRESHOLD && scrollContainer.scrollTop > 0) {
                scrollContainer.scrollTop -= SCROLL_SPEED;
                didScroll = true;
            } else if (cursorY > containerHeight - SCROLL_EDGE_THRESHOLD) {
                scrollContainer.scrollTop += SCROLL_SPEED;
                didScroll = true;
            }

            if (didScroll) {
                const items = containerRef.current.querySelectorAll('.file-item');
                const existingPaths = new Set(itemRectsCache.current.map(i => i.path));
                items.forEach(item => {
                    const itemElem = item as HTMLElement;
                    const path = itemElem.getAttribute('data-path') || '';
                    if (path && !existingPaths.has(path)) {
                        const itemRect = itemElem.getBoundingClientRect();
                        itemRectsCache.current.push({
                            path,
                            rect: {
                                left: itemRect.left - rect.left + scrollContainer.scrollLeft,
                                top: itemRect.top - rect.top + scrollContainer.scrollTop,
                                right: itemRect.right - rect.left + scrollContainer.scrollLeft,
                                bottom: itemRect.bottom - rect.top + scrollContainer.scrollTop
                            }
                        });
                    }
                });
            }

            const currentX = mv.clientX - rect.left + scrollContainer.scrollLeft;
            const currentY = mv.clientY - rect.top + scrollContainer.scrollTop;

            setSelectionRect(prev => prev ? { ...prev, w: currentX - prev.x, h: currentY - prev.y } : null);

            const newRect = {
                left: Math.min(startX, currentX),
                top: Math.min(startY, currentY),
                right: Math.max(startX, currentX),
                bottom: Math.max(startY, currentY)
            };

            const newPending = new Set<string>();
            itemRectsCache.current.forEach(({ path, rect: itemRect }) => {
                const intersects = !(newRect.right < itemRect.left ||
                    newRect.left > itemRect.right ||
                    newRect.bottom < itemRect.top ||
                    newRect.top > itemRect.bottom);

                if (intersects) {
                    newPending.add(path);
                }
            });

            if (newPending.size !== pendingSelectionRef.current.size ||
                Array.from(newPending).some(p => !pendingSelectionRef.current.has(p))) {
                pendingSelectionRef.current = newPending;
                setPendingSelection(new Set(newPending));
            }
        };

        const onMouseUp = (mu: MouseEvent) => {
            window.removeEventListener('mousemove', onMouseMove);
            window.removeEventListener('mouseup', onMouseUp);

            setSelectionRect(null);
            itemRectsCache.current = [];

            if (isMarqueeRef.current) {
                onSelectMultiple(Array.from(pendingSelectionRef.current), mu.ctrlKey);
            } else {
                if (!mu.ctrlKey) onClearSelection();
            }

            setPendingSelection(new Set());
            pendingSelectionRef.current = new Set();

            setTimeout(() => {
                isMarqueeRef.current = false;
            }, 200);
        };

        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);
    }, [containerRef, isDragging, renamingPath, cancelRename, onActivate, onSelectMultiple, onClearSelection]);

    return {
        selectionRect,
        pendingSelection,
        isMarqueeRef,
        handleMouseDown,
        setPendingSelection
    };
};
