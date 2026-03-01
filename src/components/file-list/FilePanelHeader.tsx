import React from 'react';
import cx from 'classnames';
import { HardDrive, Usb, Disc, Trash, Globe } from 'lucide-react';
import { SearchBox } from '../ui/SearchBox';
import { DriveInfo } from '../../types';
import { TFunc } from '../../i18n';
import { FavoritesMenu } from '../ui/FavoritesMenu';
import { getDriveLetter, getDriveNameOnly, getDriveTooltip, shouldShowDriveCapacity } from '../../utils/drive';

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
    const headerRef = React.useRef<HTMLDivElement>(null);
    const [isCompact, setIsCompact] = React.useState(false);
    const [isNarrow, setIsNarrow] = React.useState(false);

    React.useEffect(() => {
        if (!headerRef.current) return;
        const observer = new ResizeObserver((entries) => {
            for (const entry of entries) {
                const width = entry.contentRect.width;
                // Calculate threshold based on drive count + favorites + network/trash + search
                // roughly (120px per drive w/ name) vs (40px per drive letter)
                const neededForNames = drives.length * 120 + 250;
                setIsCompact(width < neededForNames);
                setIsNarrow(width < 380); // Trigger narrow search bar earlier
            }
        });
        observer.observe(headerRef.current);
        return () => observer.disconnect();
    }, [drives.length]);

    const renderDriveChip = (drive: DriveInfo) => {
        const letter = getDriveLetter(drive);
        const name = getDriveNameOnly(drive, t);
        const isActive = currentPath?.startsWith(drive.path);

        return (
            <div
                key={drive.path}
                className={cx("drive-chip", { active: isActive, compact: isCompact })}
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
                data-tooltip={getDriveTooltip(drive, t)}
                data-tooltip-total={shouldShowDriveCapacity(drive) ? drive.total_bytes : undefined}
                data-tooltip-free={shouldShowDriveCapacity(drive) ? drive.free_bytes : undefined}
                data-tooltip-multiline={shouldShowDriveCapacity(drive) ? "true" : undefined}
            >
                {drive.drive_type === 'removable' ? <Usb size="0.875rem" /> :
                    drive.drive_type === 'cdrom' ? <Disc size="0.875rem" /> :
                        <HardDrive size="0.875rem" />}
                {!isCompact && (
                    <span className="drive-chip-label">
                        <span className="drive-chip-name">{name}</span>
                        {letter && <span className="drive-chip-letter">{` (${letter})`}</span>}
                    </span>
                )}
                {isCompact && letter && (
                    <span className="drive-chip-letter">{letter.replace(':', '')}</span>
                )}
            </div>
        );
    };

    const [isSearchFocused, setIsSearchFocused] = React.useState(false);

    return (
        <div className="panel-header" ref={headerRef}>
            {(showDrives || currentPath === '') && (
                <div className="drive-chips">
                    <FavoritesMenu onNavigate={onNavigate} currentPath={currentPath} compact={isCompact} />
                    {drives.map(drive => renderDriveChip(drive))}
                    <div
                        className={cx("drive-chip", { active: currentPath?.startsWith('__network_vincinity__'), compact: isCompact })}
                        onClick={(e) => { e.stopPropagation(); onNavigate('__network_vincinity__'); }}
                        onContextMenu={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            onContextMenu(e, {
                                path: '__network_vincinity__',
                                is_dir: true,
                                isNetworkVicinity: true
                            } as any);
                        }}
                        data-tooltip={t('network_vincinity' as any)}
                    >
                        <Globe size="0.875rem" />
                    </div>
                    <div
                        className={cx("drive-chip", { active: currentPath?.startsWith('trash://'), compact: isCompact })}
                        onClick={(e) => { e.stopPropagation(); onNavigate('trash://'); }}
                        onContextMenu={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            onContextMenu(e, {
                                path: 'trash://',
                                is_dir: true,
                                isTrash: true
                            } as any);
                        }}
                        data-tooltip={t('recycle_bin' as any)}
                    >
                        <Trash size="0.875rem" />
                    </div>
                </div>
            )}

            {showSearch && (
                <div
                    className={cx("search-module", { narrow: isNarrow, focused: isSearchFocused })}
                    onFocus={() => setIsSearchFocused(true)}
                    onBlur={() => setIsSearchFocused(false)}
                >
                    <SearchBox
                        query={searchQuery}
                        placeholder={isNarrow && !isSearchFocused ? "" : (t('search') + "...")}
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

