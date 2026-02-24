import React from 'react';
import { FileEntry } from '../types';

export const useFileStats = (files: FileEntry[], selected: Set<string>) => {
    const totalStats = React.useMemo(() => {
        let tFiles = 0;
        let tFolders = 0;
        let tSize = 0;
        let hasFolders = false;
        let allFoldersCalculated = true;

        for (const f of files) {
            tSize += f.size;
            if (f.is_dir) {
                tFolders++;
                hasFolders = true;
                if (!f.is_calculated) allFoldersCalculated = false;
            } else {
                tFiles++;
            }
        }

        return { tFiles, tFolders, tSize, hasFolders, allFoldersCalculated };
    }, [files]);

    const stats = React.useMemo(() => {
        let sFiles = 0;
        let sFolders = 0;
        let sSize = 0;

        if (selected.size > 0) {
            // Only build the map for lookup if there are items selected
            // Using a simple object/map lookup is much faster than find()
            const pathMap = new Map();
            for (const f of files) pathMap.set(f.path, f);

            for (const path of selected) {
                const f = pathMap.get(path);
                if (f) {
                    sSize += f.size;
                    if (f.is_dir) sFolders++;
                    else sFiles++;
                }
            }
        }

        return {
            totalFiles: totalStats.tFiles,
            totalFolders: totalStats.tFolders,
            selectedFiles: sFiles,
            selectedFolders: sFolders,
            selectedSize: sSize
        };
    }, [selected, files, totalStats]);

    return { stats, totalStats };
};
