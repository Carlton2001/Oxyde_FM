import { useState, useCallback, useEffect, useRef } from 'react';

interface Size {
    width: number;
    height: number;
}

interface ResizableOptions {
    initialSize: Size;
    minSize?: Size;
}

export const useResizable = ({ initialSize, minSize = { width: 400, height: 300 } }: ResizableOptions) => {
    const [size, setSize] = useState<Size>(initialSize);
    const [isResizing, setIsResizing] = useState(false);
    const resizeStartPos = useRef({ x: 0, y: 0 });
    const initialSizeRef = useRef<Size>(initialSize);

    const handleResizeStart = useCallback((e: React.MouseEvent) => {
        if (e.button !== 0) return;
        setIsResizing(true);
        resizeStartPos.current = { x: e.clientX, y: e.clientY };
        initialSizeRef.current = { ...size };
        document.body.style.userSelect = 'none';
        e.preventDefault();
        e.stopPropagation();
    }, [size]);

    const handleResizeMove = useCallback((e: MouseEvent) => {
        if (!isResizing) return;
        const deltaX = e.clientX - resizeStartPos.current.x;
        const deltaY = e.clientY - resizeStartPos.current.y;

        setSize({
            width: Math.max(minSize.width, initialSizeRef.current.width + deltaX),
            height: Math.max(minSize.height, initialSizeRef.current.height + deltaY)
        });
    }, [isResizing, minSize]);

    const handleResizeUp = useCallback(() => {
        setIsResizing(false);
        document.body.style.userSelect = '';
    }, []);

    useEffect(() => {
        if (isResizing) {
            window.addEventListener('mousemove', handleResizeMove);
            window.addEventListener('mouseup', handleResizeUp);
        } else {
            window.removeEventListener('mousemove', handleResizeMove);
            window.removeEventListener('mouseup', handleResizeUp);
        }
        return () => {
            window.removeEventListener('mousemove', handleResizeMove);
            window.removeEventListener('mouseup', handleResizeUp);
        };
    }, [isResizing, handleResizeMove, handleResizeUp]);

    return { size, isResizing, handleResizeStart, setSize };
};
