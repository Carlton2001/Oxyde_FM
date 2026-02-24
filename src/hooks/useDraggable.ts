import { useState, useCallback, useEffect, useRef } from 'react';

interface Position {
    x: number;
    y: number;
}

interface DraggableOptions {
    initialPosition?: Position;
    dragRef: React.RefObject<HTMLElement | null>;
}

export const useDraggable = ({ initialPosition, dragRef }: DraggableOptions) => {
    const [position, setPosition] = useState<Position>(initialPosition || { x: 0, y: 0 });
    const [isDragging, setIsDragging] = useState(false);
    const dragStartPos = useRef<Position>({ x: 0, y: 0 });
    const initialElementPos = useRef<Position>({ x: 0, y: 0 });

    const handleMouseDown = useCallback((e: React.MouseEvent | MouseEvent) => {
        // Only left click
        if ('button' in e && e.button !== 0) return;

        setIsDragging(true);
        dragStartPos.current = { x: e.clientX, y: e.clientY };
        initialElementPos.current = { x: position.x, y: position.y };

        // Prevent text selection and other defaults during drag
        document.body.style.userSelect = 'none';

        e.preventDefault();
        e.stopPropagation();
    }, [position]);

    const handleMouseMove = useCallback((e: MouseEvent) => {
        if (!isDragging) return;

        const deltaX = e.clientX - dragStartPos.current.x;
        const deltaY = e.clientY - dragStartPos.current.y;

        let newX = initialElementPos.current.x + deltaX;
        let newY = initialElementPos.current.y + deltaY;

        // Clamping logic
        if (dragRef.current) {
            const rect = dragRef.current.getBoundingClientRect();
            const viewportWidth = window.innerWidth;
            const viewportHeight = window.innerHeight;

            // Calculate margins or limits
            // Since we use translate, we need to know where the element is in the viewport
            // without the current translate offset to properly clamp.
            // But we can simplify: check the current rect and adjust newX/newY

            // Limit horizontally
            if (rect.left + (newX - (position.x)) < 0) {
                newX = position.x - rect.left;
            } else if (rect.right + (newX - (position.x)) > viewportWidth) {
                newX = position.x + (viewportWidth - rect.right);
            }

            // Limit vertically
            if (rect.top + (newY - (position.y)) < 0) {
                newY = position.y - rect.top;
            } else if (rect.bottom + (newY - (position.y)) > viewportHeight) {
                newY = position.y + (viewportHeight - rect.bottom);
            }
        }

        setPosition({ x: newX, y: newY });
    }, [isDragging, dragRef, position]);

    const handleMouseUp = useCallback(() => {
        setIsDragging(false);
        document.body.style.userSelect = '';
    }, []);

    useEffect(() => {
        if (isDragging) {
            window.addEventListener('mousemove', handleMouseMove);
            window.addEventListener('mouseup', handleMouseUp);
        } else {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        }

        return () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', handleMouseUp);
        };
    }, [isDragging, handleMouseMove, handleMouseUp]);

    return {
        position,
        isDragging,
        handleMouseDown,
        setPosition
    };
};
