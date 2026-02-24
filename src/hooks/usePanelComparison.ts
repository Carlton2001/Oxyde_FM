import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { PanelState, PanelId, NotificationType } from '../types';
import { TFunc } from '../i18n';

interface UsePanelComparisonProps {
    left: PanelState;
    right: PanelState;
    activePanelId: PanelId;
    notify: (msg: string, type: NotificationType, duration?: number) => void;
    t: TFunc;
}

export const usePanelComparison = ({
    left,
    right,
    activePanelId,
    notify,
    t,
}: UsePanelComparisonProps) => {

    const [histogramPanels, setHistogramPanels] = useState<Set<PanelId>>(new Set());
    const [diffPaths, setDiffPaths] = useState<Set<string>>(new Set());
    const [isComparing, setIsComparing] = useState(false);
    const [compareSnapshot, setCompareSnapshot] = useState<{ left: string, right: string } | null>(null);

    // Keep track of which paths have histograms enabled to clear them on navigation
    const [histogramActivePaths, setHistogramActivePaths] = useState<{ left: string | null, right: string | null }>({ left: null, right: null });

    const normalizePath = (p: string | null) => p ? p.toLowerCase().replace(/[\\/]+$/, '') : '';

    // Auto-clear comparison markers when ANY panel navigates
    useEffect(() => {
        if (isComparing && compareSnapshot) {
            if (normalizePath(left.path) !== normalizePath(compareSnapshot.left) ||
                normalizePath(right.path) !== normalizePath(compareSnapshot.right)) {
                setDiffPaths(new Set());
                setIsComparing(false);
                setCompareSnapshot(null);
            }
        }
    }, [left.path, right.path, isComparing, compareSnapshot]);

    // Auto-clear histograms when a panel navigates away from the path where they were enabled
    useEffect(() => {
        if (histogramPanels.has('left') && histogramActivePaths.left) {
            if (normalizePath(left.path) !== normalizePath(histogramActivePaths.left)) {
                const next = new Set(histogramPanels);
                next.delete('left');
                setHistogramPanels(next);
            }
        }
    }, [left.path, histogramPanels, histogramActivePaths.left]);

    useEffect(() => {
        if (histogramPanels.has('right') && histogramActivePaths.right) {
            if (normalizePath(right.path) !== normalizePath(histogramActivePaths.right)) {
                const next = new Set(histogramPanels);
                next.delete('right');
                setHistogramPanels(next);
            }
        }
    }, [right.path, histogramPanels, histogramActivePaths.right]);

    const getPanel = (id: PanelId) => id === 'left' ? left : right;

    const handleComparePanels = () => {
        if (isComparing) {
            setDiffPaths(new Set());
            setIsComparing(false);
            setCompareSnapshot(null);
            return;
        }

        const diffs = new Set<string>();
        const leftFiles = left.files.filter(f => !f.is_dir);
        const rightFiles = right.files.filter(f => !f.is_dir);

        const leftMap = new Map(leftFiles.map(f => [f.name, f]));
        const rightMap = new Map(rightFiles.map(f => [f.name, f]));

        // Check files in left
        leftFiles.forEach(lf => {
            const rf = rightMap.get(lf.name);
            if (!rf || rf.size !== lf.size || rf.modified !== lf.modified || rf.is_dir !== lf.is_dir) {
                diffs.add(lf.path);
            }
        });

        // Check files in right (just for presence)
        rightFiles.forEach(rf => {
            const lf = leftMap.get(rf.name);
            if (!lf) {
                diffs.add(rf.path);
            }
        });

        setDiffPaths(diffs);
        setIsComparing(true);
        setCompareSnapshot({ left: left.path, right: right.path });

        if (diffs.size === 0) {
            notify(t('compare_no_diff'), 'info', 2000);
        }
    };

    const handleCalculateAllSizes = async () => {
        const panel = getPanel(activePanelId);
        const folderTargets = panel.files.filter(f => f.is_dir).map(f => f.path);

        if (!histogramPanels.has(activePanelId)) {
            const next = new Set(histogramPanels);
            next.add(activePanelId);
            setHistogramPanels(next);
            setHistogramActivePaths(prev => ({ ...prev, [activePanelId]: panel.path }));
        }

        try {
            // Set all to calculating first
            folderTargets.forEach(path => panel.setFileCalculating(path, true));

            // Parallel calculation for better performance
            await Promise.all(folderTargets.map(async (path) => {
                const result = await invoke<{ size: number }>('calculate_folder_size', { path });
                panel.updateFileSize(path, result.size);
            }));
        } catch (e) {
            console.error(`Failed to calculate all sizes`, e);
            notify(t('error') + ': ' + e, 'error');
            // Reset calculating state on error
            folderTargets.forEach(path => panel.setFileCalculating(path, false));
        }
    };

    return {
        histogramPanels,
        diffPaths,
        isComparing,
        setIsComparing,
        setDiffPaths,
        setHistogramPanels,
        handleComparePanels,
        handleCalculateAllSizes
    };
};
