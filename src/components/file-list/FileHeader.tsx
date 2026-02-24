import React from 'react';
import cx from 'classnames';
import { ChevronUp, ChevronDown, X, Filter } from 'lucide-react';
import { FileEntry, ViewMode, SortConfig, ColumnWidths, SortField } from '../../types';
import { ResizeHandle } from './ResizeHandle';
import { TFunc } from '../../i18n';

interface FileHeaderProps {
    viewMode: ViewMode;
    searchResults: FileEntry[] | null;
    isTrashView: boolean | undefined;
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
    t: TFunc;
    panelRef: React.RefObject<HTMLDivElement | null>;
}

export const FileHeader: React.FC<FileHeaderProps> = React.memo(({
    viewMode,
    searchResults,
    isTrashView,
    finalFiles,
    sortConfig,
    colWidths,
    onSort,
    onResize,
    onResizeMultiple,
    onHeaderContextMenu,
    onClearSearch,
    onSelectAll,
    isTypeFiltered,
    isSizeFiltered,
    isNameFiltered,
    isDateFiltered,
    t,
    panelRef
}) => {
    const SortIcon = ({ field }: { field: SortField }) => {
        if (sortConfig.field !== field) return null;
        return sortConfig.direction === 'asc'
            ? <ChevronUp size={12} className="sort-arrow" />
            : <ChevronDown size={12} className="sort-arrow" />;
    };

    return (
        <>
            <div
                className="header-selection-gutter"
                onClick={onSelectAll}
                data-tooltip={t('select_all')}
                data-tooltip-pos="right"
            />

            {viewMode === 'details' && (
                <div
                    className={cx("file-header", { "search-mode": !!searchResults, "trash-mode": isTrashView })}
                    style={{ cursor: 'default' }}
                    onContextMenu={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                    }}
                >
                    <div className="col col-name" onClick={() => onSort('name')} onContextMenu={(e) => onHeaderContextMenu?.('name', e)}>
                        {searchResults ? (
                            <div className="search-status-header">
                                <span className="search-results-count">{t('results')} ({finalFiles.length})</span>
                                <SortIcon field="name" />
                                <button className="close-search-header-btn" onClick={(e) => { e.stopPropagation(); onClearSearch(); }} data-tooltip={t('clear') || 'Clear'}>
                                    <X size={14} /> <span>{t('clear')}</span>
                                </button>
                            </div>
                        ) : (
                            <>
                                <span className="header-label">{t('name')}</span>
                                {isNameFiltered && <Filter size={10} style={{ marginLeft: '4px', opacity: 0.8, color: 'var(--accent-color)' }} />}
                                <SortIcon field="name" />
                            </>
                        )}
                        <ResizeHandle
                            field="name"
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
                    {(searchResults && !isTrashView) && (
                        <div className="col col-location" onClick={() => onSort('location')}>
                            <span className="header-label">{t('location')}</span> <SortIcon field="location" />
                            <ResizeHandle
                                field="location"
                                panelRef={panelRef}
                                onResize={onResize}
                                colWidths={colWidths}
                                files={finalFiles}
                                searchResults={!!searchResults}
                                onResizeMultiple={onResizeMultiple}
                                t={t}
                            />
                        </div>
                    )}
                    {isTrashView && (
                        <>
                            <div className="col col-location" onClick={() => onSort('location')}>
                                <span className="header-label">{t('original_location' as any)}</span> <SortIcon field="location" />
                                <ResizeHandle
                                    field="location"
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
                            <div className="col col-date" onClick={() => onSort('deletedDate')}>
                                <span className="header-label">{t('deleted_date' as any)}</span> <SortIcon field="deletedDate" />
                                <ResizeHandle
                                    field="deletedDate"
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
                        </>
                    )}
                    <div className="col col-type" onClick={() => onSort('type')} onContextMenu={(e) => onHeaderContextMenu?.('type', e)}>
                        <span className="header-label">{t('type')}</span>
                        {isTypeFiltered && <Filter size={10} style={{ marginLeft: '4px', opacity: 0.8, color: 'var(--accent-color)' }} />}
                        <SortIcon field="type" />
                        <ResizeHandle
                            field="type"
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
                    <div className="col col-size" onClick={() => onSort('size')} onContextMenu={(e) => onHeaderContextMenu?.('size', e)}>
                        <span className="header-label">{t('size')}</span>
                        {isSizeFiltered && <Filter size={10} style={{ marginLeft: '4px', opacity: 0.8, color: 'var(--accent-color)' }} />}
                        <SortIcon field="size" />
                        <ResizeHandle
                            field="size"
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
                    <div className="col col-date" onClick={() => onSort('date')} onContextMenu={(e) => onHeaderContextMenu?.('date', e)}>
                        <span className="header-label">{t('date')}</span>
                        {isDateFiltered && <Filter size={10} style={{ marginLeft: '4px', opacity: 0.8, color: 'var(--accent-color)' }} />}
                        <SortIcon field="date" />
                        <ResizeHandle
                            field="date"
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
                </div>
            )}
        </>
    );
});

