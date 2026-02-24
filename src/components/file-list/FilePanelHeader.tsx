import React from 'react';
import cx from 'classnames';
import { HardDrive, Usb, Disc, Trash } from 'lucide-react';
import { SearchBox } from '../ui/SearchBox';
import { DriveInfo } from '../../types';
import { TFunc } from '../../i18n';
import { FavoritesMenu } from '../ui/FavoritesMenu';

interface FilePanelHeaderProps {
    currentPath: string;
    drives: DriveInfo[];
    showDrives: boolean;
    onNavigate: (path: string) => void;
    onContextMenu: (e: React.MouseEvent, entry?: any) => void;
    showSearch: boolean;
    searchQuery: string;
    isSearching: boolean;
    onQueryChange: (q: string) => void;
    onSearch: () => void;
    onClearSearch: () => void;
    t: TFunc;
}

export const FilePanelHeader: React.FC<FilePanelHeaderProps> = React.memo(({
    currentPath,
    drives,
    showDrives,
    onNavigate,
    onContextMenu,
    showSearch,
    searchQuery,
    isSearching,
    onQueryChange,
    onSearch,
    onClearSearch,
    t
}) => {
    return (
        <div className="panel-header">


            {(showDrives || currentPath === '') && (
                <div className="drive-chips">
                    <FavoritesMenu onNavigate={onNavigate} currentPath={currentPath} />
                    {drives.map(drive => (
                        <div
                            key={drive.path}
                            className={cx("drive-chip", { active: currentPath?.startsWith(drive.path) })}
                            onClick={(e) => { e.stopPropagation(); onNavigate(drive.path); }}
                            onContextMenu={(e) => {
                                e.preventDefault();
                                e.stopPropagation();
                                onContextMenu(e, {
                                    path: drive.path,
                                    is_dir: true,
                                    isDrive: true,
                                    driveType: drive.drive_type
                                } as any);
                            }}
                            data-tooltip={`${drive.label} (${drive.path})`}
                            data-tooltip-total={drive.total_bytes}
                            data-tooltip-free={drive.free_bytes}
                            data-tooltip-multiline="true"
                        >
                            {drive.drive_type === 'removable' ? <Usb size="0.875rem" /> :
                                drive.drive_type === 'cdrom' ? <Disc size="0.875rem" /> :
                                    <HardDrive size="0.875rem" />} {drive.path.replace(/[:\\]+$/, '')}
                        </div>
                    ))}
                    <div
                        className={cx("drive-chip", { active: currentPath?.startsWith('trash://') })}
                        onClick={(e) => { e.stopPropagation(); onNavigate('trash://'); }}
                        data-tooltip={t('recycle_bin' as any)}
                    >
                        <Trash size="0.875rem" />
                    </div>
                </div>
            )}

            {showSearch && (
                <div className="search-module">
                    <SearchBox
                        query={searchQuery}
                        placeholder={t('search') + "..."}
                        isSearching={isSearching}
                        onChange={onQueryChange}
                        onSubmit={onSearch}
                        onClear={onClearSearch}
                        clearTitle={t('clear') || 'Clear'}
                        searchTitle={t('search')}
                    />
                </div>
            )}
        </div>
    );
});

