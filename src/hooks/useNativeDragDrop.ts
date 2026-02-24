import { useEffect, useRef, RefObject } from 'react';
import { PanelState, FileEntry, PanelId } from '../types';
import { isArchivePath, isSupportedArchiveForAdding } from '../utils/archive';

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
    setModifiers: (mods: { ctrl: boolean; shift: boolean; alt: boolean }) => void;
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
    setModifiers
}: UseNativeDragDropProps) => {

    const stateRef = useRef({ leftPanel, rightPanel, activePanelId });
    stateRef.current = { leftPanel, rightPanel, activePanelId };

    const handlerRef = useRef<{ handleFileDrop: (e: any, targetPath: string, currentPath: string) => void }>({ handleFileDrop });
    handlerRef.current = { handleFileDrop };

    useEffect(() => {
        // Native Drag Drop Handlers (External -> App)
        const handleNativeDragOver = (e: DragEvent) => {
            e.preventDefault();
        };

        const handleNativeDrop = (e: DragEvent) => {
            e.preventDefault();

            const { leftPanel, rightPanel, activePanelId } = stateRef.current;
            const activePanel = activePanelId === 'left' ? leftPanel : rightPanel;

            if (e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files.length > 0) {
                const files = Array.from(e.dataTransfer.files).map((f: any) => ({
                    path: f.path || f.name,
                    name: f.name,
                    is_dir: false // Simple assumption for external drop if not full Entry
                }));

                const target = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement;
                let targetPath = activePanel.path;

                if (target) {
                    const itemEntry = target.closest('.file-item');
                    if (itemEntry) {
                        const p = itemEntry.getAttribute('data-path');
                        if (p) {
                            const { leftPanel, rightPanel } = stateRef.current;
                            const allFiles = [...leftPanel.files, ...rightPanel.files];
                            if (leftPanel.searchResults) allFiles.push(...leftPanel.searchResults);
                            if (rightPanel.searchResults) allFiles.push(...rightPanel.searchResults);
                            const entry = allFiles.find(f => f.path === p);
                            if (entry && entry.is_dir) {
                                targetPath = p;
                            } else {
                                // Not a directory, resolve to panel path
                                const panelElement = target.closest('.panel');
                                if (panelElement) {
                                    const panelContainer = document.querySelector('.panel-container');
                                    if (panelContainer) {
                                        const panels = panelContainer.querySelectorAll('.panel');
                                        const targetId: PanelId = panels[0] === panelElement ? 'left' : 'right';
                                        targetPath = stateRef.current[targetId === 'left' ? 'leftPanel' : 'rightPanel'].path;
                                    }
                                }
                            }
                        }
                    } else {
                        const panel = target.closest('.panel');
                        if (panel) {
                            const panelContainer = document.querySelector('.panel-container');
                            if (panelContainer) {
                                const panels = panelContainer.querySelectorAll('.panel');
                                const targetId: PanelId = panels[0] === panel ? 'left' : 'right';
                                targetPath = stateRef.current[targetId === 'left' ? 'leftPanel' : 'rightPanel'].path;
                            }
                        }
                    }
                }

                const mockEvent = {
                    dataTransfer: { files: files },
                    ctrlKey: e.ctrlKey,
                    shiftKey: e.shiftKey,
                    preventDefault: () => { },
                    stopPropagation: () => { }
                };

                // Use handlerRef to call the latest handleFileDrop
                handlerRef.current.handleFileDrop(mockEvent, targetPath, targetPath);
            }
        };

        window.addEventListener('dragover', handleNativeDragOver);
        window.addEventListener('drop', handleNativeDrop);

        return () => {
            window.removeEventListener('dragover', handleNativeDragOver);
            window.removeEventListener('drop', handleNativeDrop);
        };
    }, []);


    useEffect(() => {
        const handleDocumentMouseUp = (e: MouseEvent) => {
            if (!dragState) return;

            const target = document.elementFromPoint(e.clientX, e.clientY);
            if (!target) {
                setDragState(null);
                setDragTargetPath(null);
                setDragOverPath(null);
                return;
            }

            const itemElement = target.closest('.file-item');
            if (itemElement) {
                const path = itemElement.getAttribute('data-path');
                if (path) {
                    const { leftPanel, rightPanel, activePanelId } = stateRef.current;
                    const allFiles = [...leftPanel.files, ...rightPanel.files];
                    if (leftPanel.searchResults) allFiles.push(...leftPanel.searchResults);
                    if (rightPanel.searchResults) allFiles.push(...rightPanel.searchResults);

                    const entry = allFiles.find(f => f.path === path);

                    // ONLY drop into item if it is a directory OR a SUPPORTED archive!
                    if (entry && (entry.is_dir || (isArchivePath(entry.path) && isSupportedArchiveForAdding(entry.path)))) {
                        const activePanel = activePanelId === 'left' ? leftPanel : rightPanel;
                        handlerRef.current.handleFileDrop(e as any, path, activePanel.path);
                        setDragState(null);
                        setDragTargetPath(null);
                        setDragOverPath(null);
                        return;
                    }
                    // Otherwise, fall through to panel detection (logic below will find the parent panel)
                }
            }

            // 1b. Detect Directory Tree Node
            const treeElement = target.closest('.tree-node-content');
            if (treeElement) {
                const path = treeElement.getAttribute('data-path');
                if (path) {
                    const { leftPanel, rightPanel, activePanelId } = stateRef.current;
                    const activePanel = activePanelId === 'left' ? leftPanel : rightPanel;
                    handlerRef.current.handleFileDrop(e as any, path, activePanel.path);
                    setDragState(null);
                    setDragTargetPath(null);
                    setDragOverPath(null);
                    return;
                }
            }

            // 1c. Detect Breadcrumb segment
            const breadcrumbElement = target.closest('.path-segment');
            if (breadcrumbElement) {
                const path = breadcrumbElement.getAttribute('data-path');
                if (path) {
                    const { leftPanel, rightPanel, activePanelId } = stateRef.current;
                    const activePanel = activePanelId === 'left' ? leftPanel : rightPanel;
                    handlerRef.current.handleFileDrop(e as any, path, activePanel.path);
                    setDragState(null);
                    setDragTargetPath(null);
                    setDragOverPath(null);
                    return;
                }
            }


            const panelElement = target.closest('.panel');
            if (panelElement) {
                const panelContainer = document.querySelector('.panel-container');
                if (panelContainer) {
                    const panels = panelContainer.querySelectorAll('.panel');
                    const targetId: PanelId = panels[0] === panelElement ? 'left' : 'right';
                    const targetPath = stateRef.current[targetId === 'left' ? 'leftPanel' : 'rightPanel'].path;

                    handlerRef.current.handleFileDrop(e as any, targetPath, targetPath);
                    setDragState(null);
                    setDragTargetPath(null);
                    setDragOverPath(null);
                    return;
                }
            }

            // 3. Ignore Tabs Wrapper (Let Tabs component handle it)
            if (target.closest('.tabs-wrapper')) {
                return;
            }

            setDragState(null);
            setDragTargetPath(null);
            setDragOverPath(null);
        };



        const handleMouseMove = (e: MouseEvent) => {
            if (dragState) {
                if (dragGhostRef.current) {
                    dragGhostRef.current.style.transform = `translate3d(${e.clientX + 20}px, ${e.clientY + 20}px, 0)`;
                }

                const target = document.elementFromPoint(e.clientX, e.clientY);
                if (target) {
                    // 1. Detect Folder Item
                    const itemElement = target.closest('.file-item');
                    if (itemElement) {
                        // ... existing ... 
                        const path = itemElement.getAttribute('data-path');
                        // Check if it's a directory by looking in BOTH panels using ref
                        const { leftPanel, rightPanel } = stateRef.current;
                        const allFiles = [...leftPanel.files, ...rightPanel.files];
                        if (leftPanel.searchResults) allFiles.push(...leftPanel.searchResults);
                        if (rightPanel.searchResults) allFiles.push(...rightPanel.searchResults);

                        const entry = allFiles.find(f => f.path === path);

                        if (entry && (entry.is_dir || (isArchivePath(entry.path) && isSupportedArchiveForAdding(entry.path)))) {
                            setDragOverPath(path);
                        } else {
                            setDragOverPath(null);
                        }
                    } else if (target.closest('.tree-node-content')) {
                        const path = target.closest('.tree-node-content')?.getAttribute('data-path');
                        setDragOverPath(path || null);
                    } else if (target.closest('.path-segment')) {
                        const path = target.closest('.path-segment')?.getAttribute('data-path');
                        setDragOverPath(path || null);
                    } else if (target.closest('.tabs-wrapper')) {
                        // 1.5 Detect Tabs Wrapper
                        // Only if we are dragging directories
                        if (dragState.files.some(f => f.is_dir)) {
                            setDragOverPath(null);
                            setDragTargetPath('__TABS__');
                        } else {
                            setDragOverPath(null);
                            setDragTargetPath(null);
                        }
                    } else {
                        setDragOverPath(null);
                    }

                    // 2. Detect Panel (If NOT over tabs)
                    if (!target.closest('.tabs-wrapper')) {
                        const panelElement = target.closest('.panel');
                        if (panelElement) {
                            const panelContainer = document.querySelector('.panel-container');
                            if (panelContainer) {
                                const panels = panelContainer.querySelectorAll('.panel');
                                const targetId: PanelId = panels[0] === panelElement ? 'left' : 'right';
                                setDragTargetPath(stateRef.current[targetId === 'left' ? 'leftPanel' : 'rightPanel'].path);
                            }
                        } else {
                            // Only reset if we didn't set TABS
                            setDragTargetPath(null);
                        }
                    }
                }
            }
        };

        const handleKeyChange = (e: KeyboardEvent) => {
            if (dragState) {
                if (e.key === 'Escape') {
                    setDragState(null);
                    setDragTargetPath(null);
                    setDragOverPath(null);
                }
                setModifiers({ ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey });
            }
        };

        window.addEventListener('mouseup', handleDocumentMouseUp, true);
        document.addEventListener('mousemove', handleMouseMove);
        document.addEventListener('keydown', handleKeyChange);
        document.addEventListener('keyup', handleKeyChange);
        return () => {
            window.removeEventListener('mouseup', handleDocumentMouseUp, true);
            document.removeEventListener('mousemove', handleMouseMove);
            document.removeEventListener('keydown', handleKeyChange);
            document.removeEventListener('keyup', handleKeyChange);
        };
    }, [dragState]);
};
