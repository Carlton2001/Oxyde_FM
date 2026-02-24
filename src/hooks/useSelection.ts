import { useState, useCallback, useRef, useEffect } from 'react';
import { FileEntry } from '../types';

interface SelectionState {
    selected: Set<string>;
    lastSelectedPath: string | undefined;
}

export const useSelection = (files: FileEntry[]) => {
    const [state, setState] = useState<SelectionState>({
        selected: new Set(),
        lastSelectedPath: undefined
    });

    // Use ref to access latest files without needing to recreate handleSelect constantly
    // (though we still recreate it if we want to follow React rules strictly, but this avoids closure staleness issues)
    const filesRef = useRef(files);
    useEffect(() => {
        filesRef.current = files;
    }, [files]);

    const clearSelection = useCallback(() => {
        setState({ selected: new Set(), lastSelectedPath: undefined });
    }, []);

    const selectMultiple = useCallback((paths: string[], isAdditive: boolean) => {
        setState(prev => {
            const newSelected = new Set(isAdditive ? prev.selected : []);
            paths.forEach(p => newSelected.add(p));
            return {
                selected: newSelected,
                lastSelectedPath: paths.length > 0 ? paths[paths.length - 1] : prev.lastSelectedPath
            };
        });
    }, []);

    const handleSelect = useCallback((path: string, isMulti: boolean, isRange: boolean) => {
        setState(prev => {
            const currentFiles = filesRef.current;
            const newSelected = new Set(isMulti ? prev.selected : []);

            if (isRange && prev.lastSelectedPath && prev.lastSelectedPath !== path) {
                // Files are already sorted in the hook arguments (passed from usePanel -> displayFiles)
                // So we can find indices directly in `currentFiles`
                const startIdx = currentFiles.findIndex(f => f.path === prev.lastSelectedPath);
                const endIdx = currentFiles.findIndex(f => f.path === path);

                if (startIdx !== -1 && endIdx !== -1) {
                    const [min, max] = [Math.min(startIdx, endIdx), Math.max(startIdx, endIdx)];
                    for (let i = min; i <= max; i++) {
                        newSelected.add(currentFiles[i].path);
                    }
                } else {
                    newSelected.add(path);
                }
                return { ...prev, selected: newSelected };
            } else {
                // Normal click or Ctrl+Click
                const isSelected = newSelected.has(path);

                if (isSelected && isMulti) {
                    newSelected.delete(path);
                } else {
                    newSelected.add(path);
                }
                return { selected: newSelected, lastSelectedPath: path };
            }
        });
    }, []); // Empty dependency array means handleSelect is stable!

    const setSelected = useCallback((paths: Set<string> | ((prev: Set<string>) => Set<string>)) => {
        setState(prev => {
            const newSet = typeof paths === 'function' ? paths(prev.selected) : paths;
            return {
                selected: newSet,
                lastSelectedPath: prev.lastSelectedPath
            };
        });
    }, []);

    return {
        selected: state.selected,
        setSelected,
        lastSelectedPath: state.lastSelectedPath,
        handleSelect,
        selectMultiple,
        clearSelection
    };
};
