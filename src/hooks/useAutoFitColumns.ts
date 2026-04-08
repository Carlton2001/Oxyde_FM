/**
 * useAutoFitColumns — Extracts the Canvas-based column auto-fit measurement
 * logic from ResizeHandle into a reusable hook.
 *
 * Performs adaptive sampling, font detection, Canvas text measurement,
 * and proportional space allocation to compute optimal column widths.
 */

import { useCallback } from 'react';
import { ColumnWidths, FileEntry } from '../types';
import { formatSize, formatDate, getFileTypeString } from '../utils/format';
import { getParent } from '../utils/path';
import { useApp } from '../context/AppContext';
import { TFunc } from '../i18n';
import { COLUMNS } from '../config/columnDefinitions';

interface AutoFitOptions {
    panelRef: React.RefObject<HTMLDivElement | null>;
    files: FileEntry[];
    searchResults: boolean;
    isTrashView: boolean;
    t: TFunc;
    onResizeMultiple?: (updates: Partial<ColumnWidths>) => void;
    onResize: (field: keyof ColumnWidths, size: number) => void;
}

/**
 * Returns a stable `autoFit()` callback that measures all visible columns
 * and applies optimal widths in a single batch.
 */
export const useAutoFitColumns = ({
    panelRef, files, searchResults, isTrashView, t,
    onResizeMultiple, onResize
}: AutoFitOptions) => {
    const { dateFormat } = useApp();

    const autoFit = useCallback(() => {
        if (!panelRef.current) return;

        try {
            // ---------------------------------------------------------
            // 1. Structure Constants
            // ---------------------------------------------------------
            const rootStyles = getComputedStyle(document.documentElement);
            const rootSize = parseFloat(rootStyles.fontSize) || 16;
            const HEADER_PADDING = rootSize * 1.5;
            const COL_PADDING = rootSize * 1.25; // Increased from rootSize (1.0) for better precision
            const DATE_PADDING = rootSize * 1.5; // Dates with seconds need slightly more room
            const SORT_ICON = 16;
            const SAFETY_MARGIN = 14; // Increased from 10
            const NAME_STRUCT = (rootSize * 3.75) + SAFETY_MARGIN;

            // ---------------------------------------------------------
            // 2. Fetch Fonts
            // ---------------------------------------------------------
            const dummyHeader = document.createElement('div');
            dummyHeader.className = 'file-header';
            panelRef.current.appendChild(dummyHeader);
            const headerStyles = window.getComputedStyle(dummyHeader);
            const headerFont = `${headerStyles.fontWeight} ${headerStyles.fontSize} ${headerStyles.fontFamily}`;
            dummyHeader.remove();

            const dummySpan = document.createElement('span');
            dummySpan.className = 'file-name';
            const dummyPanel = document.createElement('div');
            dummyPanel.className = 'panel file-list details';
            dummyPanel.style.position = 'absolute';
            dummyPanel.style.visibility = 'hidden';
            dummyPanel.appendChild(dummySpan);
            document.body.appendChild(dummyPanel);
            const contentFont = window.getComputedStyle(dummySpan).font;
            dummyPanel.remove();

            // ---------------------------------------------------------
            // 3. Adaptive Sampling
            // ---------------------------------------------------------
            const MAX_TOTAL_SAMPLE = 2000;
            let subset: FileEntry[] = files;

            if (files.length > MAX_TOTAL_SAMPLE) {
                const start = files.slice(0, 1000);
                const end = files.slice(-500);
                const midCount = 500;
                const mid: FileEntry[] = [];
                for (let i = 0; i < midCount; i++) {
                    const idx = 1000 + Math.floor(Math.random() * (files.length - 1500));
                    mid.push(files[idx]);
                }
                subset = [...start, ...mid, ...end];
            }

            // ---------------------------------------------------------
            // 4. Measure
            // ---------------------------------------------------------
            const canvas = document.createElement('canvas');
            const context = canvas.getContext('2d');
            if (!context) return;

            // 4a. Headers
            context.font = headerFont;
            const measureHeader = (str: string) => Math.ceil(context.measureText(str).width + HEADER_PADDING + SORT_ICON);

            let maxName = measureHeader(t('name'));
            let maxType = measureHeader(t('type'));
            let maxSize = measureHeader(t('size'));
            let maxDate = measureHeader(t('date'));
            let maxDeletedDate = measureHeader(t('deleted_date' as any));
            let maxLocation = measureHeader(isTrashView ? t('original_location') : t('location'));

            // 4b. Scanning logic
            context.font = contentFont;
            let maxNameText = 0;
            let maxTypeText = 0;
            let maxSizeText = 0;
            let maxDateText = 0;
            let maxDeletedDateText = 0;
            let maxLocationText = 0;

            const typeCache: Record<string, number> = {};
            const sizeCache: Record<string, number> = {};

            // Date widths will be measured dynamically within the loop

            for (const f of subset) {
                const nw = context.measureText(f.name).width;
                if (nw > maxNameText) maxNameText = nw;

                const typeStr = getFileTypeString(f, t);
                if (typeCache[typeStr] === undefined) {
                    typeCache[typeStr] = context.measureText(typeStr).width;
                }
                const tw = typeCache[typeStr];
                if (tw > maxTypeText) maxTypeText = tw;

                if (!f.is_dir || f.is_calculated) {
                    const sizeStr = formatSize(f.size, 1, t);
                    if (sizeCache[sizeStr] === undefined) {
                        sizeCache[sizeStr] = context.measureText(sizeStr).width;
                    }
                    const sw = sizeCache[sizeStr];
                    if (sw > maxSizeText) maxSizeText = sw;
                }

                const fDateStr = formatDate(f.modified || 0, dateFormat);
                const dw = context.measureText(fDateStr).width;
                if (dw > maxDateText) maxDateText = dw;

                if (isTrashView) {
                    const fDelDateStr = formatDate(f.deleted_time || 0, dateFormat);
                    const ddw = context.measureText(fDelDateStr).width;
                    if (ddw > maxDeletedDateText) maxDeletedDateText = ddw;
                }


                if (searchResults || isTrashView) {
                    const loc = isTrashView ? (f.original_path || '') : ((getParent(f.path) || f.path) || '');
                    const lw = context.measureText(loc).width;
                    if (lw > maxLocationText) maxLocationText = lw;
                }
            }

            maxName = Math.max(maxName, Math.ceil(maxNameText + NAME_STRUCT));
            maxType = Math.max(maxType, Math.ceil(maxTypeText + COL_PADDING));
            maxSize = Math.max(maxSize, Math.ceil(maxSizeText + COL_PADDING));
            maxDate = Math.max(maxDate, Math.ceil(maxDateText + DATE_PADDING));
            maxLocation = Math.max(maxLocation, Math.ceil(maxLocationText + COL_PADDING));

            // ---------------------------------------------------------
            // 5. Batch Apply (Robust Allocation)
            // ---------------------------------------------------------
            const panelWidth = panelRef.current.clientWidth;

            // Clamping measured values to their respective minWidths from COLUMNS registry
            const getSafeWidth = (key: string, measured: number) => {
                const def = COLUMNS.find(c => c.key === key);
                return Math.max(measured, def?.minWidth || 20);
            };

            const safeType = getSafeWidth('type', maxType);
            const safeSize = getSafeWidth('size', maxSize);
            const safeDate = getSafeWidth('date', maxDate);
            const safeDeletedDate = isTrashView ? getSafeWidth('deletedDate', Math.max(maxDeletedDate, Math.ceil(maxDeletedDateText + DATE_PADDING))) : 0;

            const updates: Partial<ColumnWidths> = {
                type: safeType,
                size: safeSize,
                date: safeDate
            };
            if (isTrashView) updates.deletedDate = safeDeletedDate;

            const fixedSum = safeType + safeSize + safeDate + safeDeletedDate;

            if (searchResults || isTrashView) {
                const safeLocationMeasured = getSafeWidth('location', maxLocation);
                const safeNameMeasured = getSafeWidth('name', Math.ceil(maxNameText + NAME_STRUCT));

                // Available space for the two "variable" columns (Name & Location)
                const availableForPair = Math.max(0, panelWidth - fixedSum - 40); // 40px safety/gutter
                const minLocation = 80;
                const minName = 150;

                if (availableForPair >= (safeLocationMeasured + safeNameMeasured)) {
                    // Plenty of space: give Location what it needs, Name takes the rest (extra flex)
                    updates.location = safeLocationMeasured;
                    updates.name = availableForPair - safeLocationMeasured;
                } else {
                    // Tight space: Prioritize Name as the primary identifier
                    // Give Name its full measured width to ensure it's not truncated if possible
                    updates.name = safeNameMeasured;
                    
                    // Location gets the remaining share, but never falls below its absolute minimum
                    updates.location = Math.max(minLocation, availableForPair - safeNameMeasured);
                }
            } else {
                // Normal mode: Standard flex calculation for Name
                updates.name = Math.max(150, panelWidth - fixedSum - 32); 
            }

            if (onResizeMultiple) {
                onResizeMultiple(updates);
            } else {
                Object.entries(updates).forEach(([key, val]) => {
                    onResize(key as keyof ColumnWidths, val as number);
                });
            }

        } catch (err) {
            console.error("Auto-resize failed", err);
        }
    }, [panelRef, files, searchResults, isTrashView, t, dateFormat, onResizeMultiple, onResize]);

    return autoFit;
};
