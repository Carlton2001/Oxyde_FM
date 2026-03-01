import React from 'react';
import cx from 'classnames';
import { ChevronUp, ChevronDown, Filter } from 'lucide-react';
import { FileEntry, ViewMode, ColumnWidths, SortConfig, SortField } from '../../types';
import { ResizeHandle } from './ResizeHandle';
import { TFunc } from '../../i18n';
import { getColumnMode, getVisibleColumns, getColumnLabel, getColumnSortField } from '../../config/columnDefinitions';

interface FileHeaderProps {
    viewMode: ViewMode;
    searchResults: FileEntry[] | null;
    isTrashView: boolean | undefined;
    isNetworkView?: boolean;
    finalFiles: FileEntry[];
    sortConfig: SortConfig;
    colWidths: ColumnWidths;
    onSort: (field: SortField) => void;
    onResize: (field: keyof ColumnWidths, delta: number) => void;
    onResizeMultiple?: (updates: Partial<ColumnWidths>) => void;
    onHeaderContextMenu?: (field: keyof ColumnWidths, e: React.MouseEvent) => void;
    onClearSearch: () => void;
    onSelectAll: (e: React.MouseEvent) => void;
    isTypeFiltered?: boolean;
    isSizeFiltered?: boolean;
    isNameFiltered?: boolean;
    isDateFiltered?: boolean;
    isLocationFiltered?: boolean;
    isDeletedDateFiltered?: boolean;
    t: TFunc;
    panelRef: React.RefObject<HTMLDivElement | null>;
}

export const FileHeader: React.FC<FileHeaderProps> = React.memo(({
    viewMode,
    searchResults,
    isTrashView,
    isNetworkView,
    finalFiles,
    sortConfig,
    colWidths,
    onSort,
    onResize,
    onResizeMultiple,
    onHeaderContextMenu,
    isTypeFiltered,
    isSizeFiltered,
    isNameFiltered,
    isDateFiltered,
    isLocationFiltered,
    isDeletedDateFiltered,
    t,
    panelRef,
    onSelectAll
}) => {
    const SortIcon = ({ field }: { field: SortField }) => {
        if (sortConfig.field !== field) return null;
        return sortConfig.direction === 'asc'
            ? <ChevronUp size={12} className="sort-arrow" />
            : <ChevronDown size={12} className="sort-arrow" />;
    };

    const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);
    const visibleCols = getVisibleColumns(mode);

    return (
        <>
            <div
                className="header-selection-gutter"
                onClick={onSelectAll}
                data-tooltip={t('select_all')}
                data-tooltip-pos="right"
            />

            {viewMode === 'details' && (
                <div className="file-header" onClick={onSelectAll}>
                    {visibleCols.map(col => {
                        const sField = getColumnSortField(col, mode);
                        const label = t(getColumnLabel(col, mode) as any);

                        const isName = col.key === 'name';

                        const isFiltered =
                            (col.key === 'type' && isTypeFiltered) ||
                            (col.key === 'size' && isSizeFiltered) ||
                            (col.key === 'name' && isNameFiltered) ||
                            (col.key === 'date' && isDateFiltered) ||
                            (col.key === 'location' && isLocationFiltered) ||
                            (col.key === 'deletedDate' && isDeletedDateFiltered);

                        return (
                            <div
                                key={col.key}
                                className={cx("col", `col-${col.key}`, { "col-name": isName })}
                                onClick={() => {
                                    if (isName) return;
                                    onSort(sField);
                                }}
                                onContextMenu={(e) => onHeaderContextMenu?.(col.key as any, e)}
                            >
                                <span className="header-label">{label}</span>
                                {isFiltered && <Filter size={10} style={{ marginLeft: '4px', opacity: 0.8, color: 'var(--accent-color)' }} />}
                                <SortIcon field={sField} />
                                <ResizeHandle
                                    field={col.key as any}
                                    panelRef={panelRef}
                                    onResize={onResize}
                                    colWidths={colWidths}
                                    files={finalFiles}
                                    searchResults={!!searchResults}
                                    isTrashView={isTrashView}
                                    onResizeMultiple={onResizeMultiple}
                                    t={t}
                                />
                            </div>
                        );
                    })}
                </div>
            )}
        </>
    );
});
