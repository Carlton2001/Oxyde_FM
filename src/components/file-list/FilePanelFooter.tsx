import React from 'react';
import { StatusBar } from '../layout/StatusBar';
import { TFunc } from '../../i18n';

interface FilePanelFooterProps {
    stats: {
        totalFiles: number;
        totalFolders: number;
        selectedFiles: number;
        selectedFolders: number;
        selectedSize: number;
    };
    viewMode: 'grid' | 'details';
    onViewModeChange: (mode: 'grid' | 'details') => void;
    onActivate: () => void;
    t: TFunc;
}

export const FilePanelFooter: React.FC<FilePanelFooterProps> = React.memo(({ stats, viewMode, onViewModeChange, onActivate, t }) => {
    return (
        <div className="panel-footer">
            <StatusBar
                fileCount={stats.totalFiles}
                folderCount={stats.totalFolders}
                selectedFileCount={stats.selectedFiles}
                selectedFolderCount={stats.selectedFolders}
                selectedSize={stats.selectedSize}
                viewMode={viewMode}
                onViewModeChange={onViewModeChange}
                onActivate={onActivate}
                t={t}
            />
        </div>
    );
});

