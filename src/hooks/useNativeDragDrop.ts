import { useEffect, useRef, RefObject } from 'react';
import { PanelState, FileEntry, PanelId } from '../types';
import { isArchivePath, isSupportedArchiveForAdding } from '../utils/archive';
import { startDrag } from '@crabnebula/tauri-plugin-drag';

interface UseNativeDragDropProps {
    leftPanel: PanelState;
    rightPanel: PanelState;
    activePanelId: PanelId;
    dragState: { sourcePanel: PanelId; files: FileEntry[] } | null;
    dragGhostRef: RefObject<HTMLDivElement | null>;
    handleFileDrop: (e: any, targetPath: string, currentPath: string) => void;
    setDragState: (state: { sourcePanel: PanelId; files: FileEntry[] } | null) => void;
    setDragOverPath: (path: string | null) => void;
    setDragTargetPath: (path: string | null) => void;
    modifiers: { ctrl: boolean; shift: boolean; alt: boolean };
    setModifiers: (mods: { ctrl: boolean; shift: boolean; alt: boolean }) => void;
    isNativeActive: boolean;
    setIsNativeActive: (active: boolean) => void;
    dragTargetPath: string | null;
}

export const useNativeDragDrop = ({
    leftPanel,
    rightPanel,
    activePanelId,
    dragState,
    dragGhostRef,
    handleFileDrop,
    setDragState,
    setDragOverPath,
    setDragTargetPath,
    modifiers,
    setModifiers,
    isNativeActive,
    setIsNativeActive,
    dragTargetPath
}: UseNativeDragDropProps) => {

    const modifiersRef = useRef(modifiers);
    useEffect(() => {
        modifiersRef.current = modifiers;
    }, [modifiers]);

    const stateRef = useRef({ leftPanel, rightPanel, activePanelId });
    stateRef.current = { leftPanel, rightPanel, activePanelId };

    const handlerRef = useRef<{ handleFileDrop: (e: any, targetPath: string, currentPath: string) => void }>({ handleFileDrop });
    handlerRef.current = { handleFileDrop };

    const dragStateRef = useRef(dragState);
    dragStateRef.current = dragState;

    const nativeStartedRef = useRef(false);
    const isNativeActiveRef = useRef(isNativeActive);
    const dragTargetPathRef = useRef(dragTargetPath);
    useEffect(() => {
        isNativeActiveRef.current = isNativeActive;
    }, [isNativeActive]);

    useEffect(() => {
        dragTargetPathRef.current = dragTargetPath;
    }, [dragTargetPath]);

    useEffect(() => {
        if (!dragState) {
            nativeStartedRef.current = false;
            setIsNativeActive(false);
        }
    }, [dragState, setIsNativeActive]);

    const getTargetPathsFromElement = (target: HTMLElement): { targetPath: string | null, overPath: string | null } => {
        // 1. Try specifically a Folder/Archive in the list
        const itemEntry = target.closest('.file-item');
        if (itemEntry) {
            const p = itemEntry.getAttribute('data-path');
            const isDir = itemEntry.getAttribute('data-is-dir') === 'true';
            if (p && (isDir || isArchivePath(p) && isSupportedArchiveForAdding(p))) return { targetPath: p, overPath: p };
        }

        // 2. Try Tree nodes
        const treeEntry = target.closest('.tree-node-content');
        if (treeEntry) {
            const p = treeEntry.getAttribute('data-path');
            return { targetPath: p || null, overPath: p || null };
        }

        // 3. Try Breadcrumb segments
        const pathSegment = target.closest('.path-segment');
        if (pathSegment) {
            const p = pathSegment.getAttribute('data-path');
            return { targetPath: p || null, overPath: p || null };
        }

        // 4. Try Tabs
        const tabsWrapper = target.closest('.tabs-wrapper');
        if (tabsWrapper) {
            if (dragStateRef.current?.files.some(f => f.is_dir)) return { targetPath: '__TABS__', overPath: null };
            return { targetPath: null, overPath: null };
        }

        // 5. Fallback to the Panel itself
        const panel = target.closest('.panel');
        if (panel) {
            const panelContainer = document.querySelector('.panel-container');
            if (panelContainer) {
                const panels = panelContainer.querySelectorAll('.panel');
                const targetId: PanelId = panels[0] === panel ? 'left' : 'right';
                const panelPath = stateRef.current[targetId === 'left' ? 'leftPanel' : 'rightPanel'].path;
                return { targetPath: panelPath, overPath: null };
            }
        }
        return { targetPath: null, overPath: null };
    };

    const updateDragUI = (clientX: number, clientY: number) => {
        if (dragGhostRef.current) {
            const el = dragGhostRef.current;
            // Only update transform if NOT docked, otherwise docked CSS !important wins
            if (!isNativeActive) {
                el.style.transform = `translate3d(${clientX + 20}px, ${clientY + 20}px, 0)`;
            }
        }

        const target = document.elementFromPoint(clientX, clientY) as HTMLElement;
        if (target) {
            const { targetPath, overPath } = getTargetPathsFromElement(target);
            setDragTargetPath(targetPath);
            setDragOverPath(overPath);
        }
    };

    useEffect(() => {
        let unlistenDrop: (() => void) | undefined;

        const setupTauriListeners = async () => {
            try {
                const { getCurrentWindow } = await import('@tauri-apps/api/window');
                const win = getCurrentWindow();
                const scaleFactor = await win.scaleFactor();

                const unlisten = await win.onDragDropEvent((event) => {
                    const payload = event.payload as any;
                    // We only use Tauri for 'drop' and 'leave'
                    // Positioning is now handled by the 'dragover' event below for better performance
                    if (payload.type === 'drop') {
                        const position = payload.position;
                        const lx = position ? Math.round(position.x / scaleFactor) : 0;
                        const ly = position ? Math.round(position.y / scaleFactor) : 0;

                        if ((payload.paths && payload.paths.length > 0) || dragStateRef.current) {
                            const target = document.elementFromPoint(lx, ly) as HTMLElement;
                            let targetPath = stateRef.current.activePanelId === 'left' ? stateRef.current.leftPanel.path : stateRef.current.rightPanel.path;
                            if (target) {
                                const { targetPath: infoPath } = getTargetPathsFromElement(target);
                                if (infoPath) targetPath = infoPath;
                            }
                            const files = (payload.paths && payload.paths.length > 0) ? payload.paths.map((p: string) => ({
                                path: p, name: p.split(/[\\/]/).pop() || p, is_dir: false
                            })) : [];
                            const mockEvent = {
                                dataTransfer: { files: files },
                                ctrlKey: modifiersRef.current.ctrl,
                                shiftKey: modifiersRef.current.shift,
                                altKey: modifiersRef.current.alt,
                                preventDefault: () => { }, stopPropagation: () => { }
                            };
                            handlerRef.current.handleFileDrop(mockEvent, targetPath, targetPath);
                        }
                        setDragState(null);
                        setDragTargetPath(null);
                        setDragOverPath(null);
                        setIsNativeActive(false);
                    } else if (payload.type === 'leave') {
                        // Only hide/clear if we're not in native docked mode
                        const isNativeActiveNow = isNativeActiveRef.current;
                        if (!isNativeActiveNow && dragGhostRef.current) {
                            dragGhostRef.current.style.setProperty('opacity', '0', 'important');
                            dragGhostRef.current.style.setProperty('visibility', 'hidden', 'important');
                        }
                        setDragOverPath(null);

                        // If internal drag, and we leave without native handoff, we should probably reset?
                        // Actually, keep it for returning. But if no button, clear.
                    }
                });
                unlistenDrop = unlisten;
            } catch (err) {
                console.error("Failed to setup Tauri drag-drop listeners:", err);
            }
        };

        setupTauriListeners();

        const handlePreventDrop = (e: DragEvent) => {
            if (dragStateRef.current) e.preventDefault();
        };

        const handleDocumentMouseUp = (e: MouseEvent) => {
            if (!dragStateRef.current) return;

            const targetPath = dragTargetPathRef.current;
            if (targetPath) {
                const { leftPanel, rightPanel, activePanelId } = stateRef.current;
                // If we have a target path (folder, tree, panel), we use it.
                // We need a source path to help the handler decide the fallback folder if needed
                const sourcePanelPath = activePanelId === 'left' ? leftPanel.path : rightPanel.path;
                handlerRef.current.handleFileDrop(e as any, targetPath, sourcePanelPath);
            }

            setDragState(null);
            setDragTargetPath(null);
            setDragOverPath(null);
            setIsNativeActive(false);
            nativeStartedRef.current = false;
        };

        const handleMouseMove = (e: MouseEvent) => {
            const currentDragState = dragStateRef.current;
            if (!currentDragState) return;

            // SAFETY RESET: if the user let go outside or something failed, reset on first mouse move return
            if (e.buttons === 0) {
                setDragState(null);
                setDragTargetPath(null);
                setDragOverPath(null);
                nativeStartedRef.current = false;
                setIsNativeActive(false);
                return;
            }

            if (!nativeStartedRef.current) {
                updateDragUI(e.clientX, e.clientY);
                const edgeThreshold = 5;
                const isAtEdge = e.clientX < edgeThreshold || e.clientY < edgeThreshold ||
                    e.clientX > window.innerWidth - edgeThreshold ||
                    e.clientY > window.innerHeight - edgeThreshold;
                if (isAtEdge || modifiersRef.current.alt) {
                    triggerNativeDrag();
                }
            }
        };

        const triggerNativeDrag = () => {
            const currentDragState = dragStateRef.current;
            if (!currentDragState || nativeStartedRef.current) return;
            nativeStartedRef.current = true;
            const paths = currentDragState.files.map(f => f.path);
            if (paths.length > 0) {
                setIsNativeActive(true);
                startDrag({ item: paths, icon: '' }).catch(err => {
                    console.error("Native handoff failed:", err);
                    nativeStartedRef.current = false;
                    setIsNativeActive(false);
                });
            }
        };

        const handleKeyChange = (e: KeyboardEvent) => {
            if (dragStateRef.current) {
                if (e.key === 'Escape') {
                    setDragState(null);
                    setDragTargetPath(null);
                    setDragOverPath(null);
                }
                const mods = { ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey };
                modifiersRef.current = mods;
                setModifiers(mods);
            }
        };

        // Standard HTML5 Drag Events
        window.addEventListener('dragenter', handlePreventDrop, true);
        window.addEventListener('dragover', handlePreventDrop, true);
        window.addEventListener('drop', handlePreventDrop, true);

        // Internal Mouse/Keyboard Events
        window.addEventListener('mouseup', handleDocumentMouseUp, true);
        document.addEventListener('mousemove', handleMouseMove);
        document.addEventListener('keydown', handleKeyChange);
        document.addEventListener('keyup', handleKeyChange);

        return () => {
            if (unlistenDrop) unlistenDrop();
            window.removeEventListener('dragenter', handlePreventDrop, true);
            window.removeEventListener('dragover', handlePreventDrop, true);
            window.removeEventListener('drop', handlePreventDrop, true);
            window.removeEventListener('mouseup', handleDocumentMouseUp, true);
            document.removeEventListener('mousemove', handleMouseMove);
            document.removeEventListener('keydown', handleKeyChange);
            document.removeEventListener('keyup', handleKeyChange);
        };
    }, []);
};
