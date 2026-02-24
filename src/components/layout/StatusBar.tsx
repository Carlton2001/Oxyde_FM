import React from 'react';
import { formatSize } from '../../utils/format';
import { TFunc } from '../../i18n';
import { LayoutGrid, TableOfContents } from 'lucide-react';
import cx from 'classnames';
import './StatusBar.css';

interface StatusBarProps {
    fileCount: number;
    folderCount: number;
    selectedFileCount: number;
    selectedFolderCount: number;
    selectedSize: number;
    viewMode: 'grid' | 'details';
    onViewModeChange: (mode: 'grid' | 'details') => void;
    onActivate: () => void;
    t: TFunc;
}

export const StatusBar: React.FC<StatusBarProps> = ({
    fileCount,
    folderCount,
    selectedFileCount,
    selectedFolderCount,
    selectedSize,
    viewMode,
    onViewModeChange,
    onActivate,
    t
}) => {
    const getCountText = (count: number, singularKey: string, pluralKey: string) => {
        if (count === 0) return '';
        return `${count} ${t((count === 1 ? singularKey : pluralKey) as any).toLowerCase()}`;
    };

    const hasSelection = selectedFileCount > 0 || selectedFolderCount > 0;

    const totalText = [
        getCountText(folderCount, 'folder', 'folders'),
        getCountText(fileCount, 'file', 'files')
    ].filter(Boolean).join(', ');

    const selectedText = hasSelection ? [
        getCountText(selectedFolderCount, 'folder', 'folders'),
        getCountText(selectedFileCount, 'file', 'files')
    ].filter(Boolean).join(', ') : '';

    return (
        <div className="status-bar" onClick={(e) => { e.stopPropagation(); onActivate(); }}>
            <div className="status-info">
                <div className="status-section">
                    {totalText}
                </div>

                {hasSelection && (
                    <>
                        <span className="separator">•</span>
                        <div className="status-section selected">
                            {selectedText}
                            <span className="selected-label">
                                {` ${t(((selectedFileCount + selectedFolderCount) > 1 ? 'selected' : 'selected_singular') as any)}`}
                            </span>
                            {selectedFileCount > 0 && (
                                <span className="size"> ({formatSize(selectedSize, 1, t)})</span>
                            )}
                        </div>
                    </>
                )}
            </div>

            <div className="view-switcher">
                <button
                    className={cx("btn-icon", { active: viewMode === 'details' })}
                    onClick={(e) => { e.stopPropagation(); onViewModeChange('details'); }}
                    data-tooltip={t('view_details')}
                >
                    <TableOfContents size={14} />
                </button>
                <button
                    className={cx("btn-icon", { active: viewMode === 'grid' })}
                    onClick={(e) => { e.stopPropagation(); onViewModeChange('grid'); }}
                    data-tooltip={t('view_grid')}
                >
                    <LayoutGrid size={14} />
                </button>
            </div>
        </div>
    );
};

