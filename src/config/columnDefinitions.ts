/**
 * Centralized Column Definition Registry
 * 
 * Single source of truth for all column definitions used in the file list detail view.
 * Consumed by: FileHeader, DetailsRow, ResizeHandle auto-fit, and grid-template generation.
 */

import React from 'react';
import { FileEntry, SortField, DateFormat } from '../types';
import { formatSize, formatDate, getFileTypeString } from '../utils/format';
import { getParent } from '../utils/path';
import { TFunc } from '../i18n';

// ─── Types ──────────────────────────────────────────────────────────────────

/** View modes that determine which columns are visible */
export type ColumnMode = 'normal' | 'search' | 'trash';

/** Context passed to cell renderers */
export interface CellRenderContext {
    t: TFunc;
    dateFormat: DateFormat;
}

/** Context passed to the auto-fit measurement function */
export interface MeasureContext {
    t: TFunc;
    dateFormat: DateFormat;
}

/** Full column definition */
export interface ColumnDef {
    /** Unique key matching ColumnWidths keys */
    key: string;
    /** i18n key for the header label */
    labelKey: string;
    /** Alternative i18n key for specific modes (e.g. 'original_location' in trash) */
    labelKeyOverrides?: Partial<Record<ColumnMode, string>>;
    /** Sort field triggered when clicking this header */
    sortField: SortField;
    /** Sort field overrides for specific modes */
    sortFieldOverrides?: Partial<Record<ColumnMode, SortField>>;
    /** Default width in pixels */
    defaultWidth: number;
    /** Minimum width for manual resize */
    minWidth: number;
    /** If true, this column takes remaining space (flex: 1 in CSS) */
    flex?: boolean;
    /** Content alignment */
    align?: 'left' | 'right';
    /** CSS class for the cell div */
    cellClass: string;
    /** Which modes this column is visible in */
    visibleIn: ColumnMode[];
    /** Render the cell content for a given entry */
    renderCell: (entry: FileEntry, ctx: CellRenderContext) => React.ReactNode;
    /** Return the text content for auto-fit measurement (used by ResizeHandle) */
    measureContent: (entry: FileEntry, ctx: MeasureContext) => string;
}

// ─── Column Definitions ─────────────────────────────────────────────────────

export const COLUMNS: ColumnDef[] = [
    {
        key: 'name',
        labelKey: 'name',
        sortField: 'name',
        defaultWidth: 250,
        minWidth: 100,
        flex: true,
        cellClass: 'file-name-group',
        visibleIn: ['normal', 'search', 'trash'],
        // Name column has special rendering (icon + rename), handled by the component directly
        renderCell: (_entry, _ctx) => null,
        measureContent: (entry) => entry.name,
    },
    {
        key: 'location',
        labelKey: 'location',
        labelKeyOverrides: { trash: 'original_location' },
        sortField: 'location',
        defaultWidth: 200,
        minWidth: 50,
        cellClass: 'file-info col-location',
        visibleIn: ['search', 'trash'],
        renderCell: (entry, _ctx) => {
            // In search mode: parent path; in trash mode: original_path
            const text = entry.original_path || getParent(entry.path) || '';
            return React.createElement('span', { className: 'text-truncate' }, text);
        },
        measureContent: (entry) => entry.original_path || getParent(entry.path) || '',
    },
    {
        key: 'deletedDate',
        labelKey: 'deleted_date',
        sortField: 'deletedDate',
        defaultWidth: 120,
        minWidth: 50,
        cellClass: 'file-info col-date',
        visibleIn: ['trash'],
        renderCell: (entry, ctx) => {
            const text = entry.deleted_time ? formatDate(entry.deleted_time, ctx.dateFormat) : '';
            return React.createElement('span', { className: 'text-truncate' }, text);
        },
        measureContent: (entry, ctx) => entry.deleted_time ? formatDate(entry.deleted_time, ctx.dateFormat) : '',
    },
    {
        key: 'type',
        labelKey: 'type',
        sortField: 'type',
        defaultWidth: 80,
        minWidth: 20,
        cellClass: 'file-info col-type',
        visibleIn: ['normal', 'search', 'trash'],
        renderCell: (entry, ctx) => {
            return React.createElement('span', { className: 'text-truncate' }, getFileTypeString(entry, ctx.t));
        },
        measureContent: (entry, ctx) => getFileTypeString(entry, ctx.t),
    },
    {
        key: 'size',
        labelKey: 'size',
        sortField: 'size',
        defaultWidth: 80,
        minWidth: 20,
        align: 'right',
        cellClass: 'file-info col-size',
        visibleIn: ['normal', 'search', 'trash'],
        renderCell: (entry, ctx) => {
            let text = '';
            if (entry.is_dir) {
                if (entry.is_calculated) {
                    text = entry.size === 0 ? ctx.t('empty_dir') : formatSize(entry.size, 1, ctx.t);
                } else if (entry.is_calculating) {
                    text = ctx.t('calculating');
                }
            } else {
                text = formatSize(entry.size, 1, ctx.t);
            }
            return React.createElement('span', { className: 'text-truncate' }, text);
        },
        measureContent: (entry, ctx) => {
            if (entry.is_dir) return '';
            return formatSize(entry.size, 1, ctx.t);
        },
    },
    {
        key: 'date',
        labelKey: 'date',
        sortField: 'date',
        defaultWidth: 120,
        minWidth: 20,
        cellClass: 'file-info col-date',
        visibleIn: ['normal', 'search', 'trash'],
        renderCell: (entry, ctx) => {
            return React.createElement('span', { className: 'text-truncate' }, formatDate(entry.modified, ctx.dateFormat));
        },
        measureContent: (entry, ctx) => formatDate(entry.modified, ctx.dateFormat),
    },
];

// ─── Helpers ────────────────────────────────────────────────────────────────

/**
 * Returns the columns visible in a given mode, in the correct order.
 */
export function getVisibleColumns(mode: ColumnMode): ColumnDef[] {
    return COLUMNS.filter(col => col.visibleIn.includes(mode));
}

/**
 * Returns the current mode based on view state.
 */
export function getColumnMode(isTrashView: boolean, hasSearchResults: boolean): ColumnMode {
    if (isTrashView) return 'trash';
    if (hasSearchResults) return 'search';
    return 'normal';
}

/**
 * Returns the header label key for a column in a given mode.
 */
export function getColumnLabel(col: ColumnDef, mode: ColumnMode): string {
    return col.labelKeyOverrides?.[mode] || col.labelKey;
}

/**
 * Returns the sort field for a column in a given mode.
 */
export function getColumnSortField(col: ColumnDef, mode: ColumnMode): SortField {
    return col.sortFieldOverrides?.[mode] || col.sortField;
}

/**
 * Returns default column widths as an object keyed by column key.
 */
export function getDefaultColumnWidths(): Record<string, number> {
    const widths: Record<string, number> = {};
    for (const col of COLUMNS) {
        widths[col.key] = col.defaultWidth;
    }
    return widths;
}

/**
 * Generates a CSS grid-template-columns string from column widths and visible columns.
 * The first column (name) with `flex: true` uses its width but will be treated as the flexible column.
 */
export function buildGridTemplate(
    visibleColumns: ColumnDef[],
    colWidths: Record<string, number>
): string {
    return visibleColumns
        .map(col => `${colWidths[col.key] || col.defaultWidth}px`)
        .join(' ');
}
