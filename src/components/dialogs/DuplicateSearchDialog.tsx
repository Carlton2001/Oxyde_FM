import React, { useState, useRef } from 'react';
import { X, Search, Copy, Loader2, ChevronRight, ChevronDown, Check, Database, Settings2 } from 'lucide-react';
import { useDraggable } from '../../hooks/useDraggable';
import { useResizable } from '../../hooks/useResizable';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { FileEntry, DriveInfo } from '../../types';
import { usePanelContext } from '../../context/PanelContext';
import { formatSize } from '../../utils/format';
import { getFileIcon } from '../../utils/fileIcons';
import { useApp } from '../../context/AppContext';
import './SearchDialog.css';
import './DuplicateSearchDialog.css';
import cx from 'classnames';

interface DuplicateSearchDialogProps {
    initialRoot: string;
    onClose: () => void;
    t: any;
    zIndex?: number;
    onFocus?: () => void;
}

export const DuplicateSearchDialog: React.FC<DuplicateSearchDialogProps> = ({
    initialRoot,
    onClose,
    t,
    zIndex,
    onFocus
}) => {
    const { useSystemIcons } = useApp();
    const [duplicates, setDuplicates] = useState<{ size: number, files: FileEntry[] }[]>([]);
    const [isSearchingDuplicates, setIsSearchingDuplicates] = useState(false);
    const [duplicatesError, setDuplicatesError] = useState<string | null>(null);
    const [displayedDuplicatesCount, setDisplayedDuplicatesCount] = useState(100);
    const [duplicatesProgress, setDuplicatesProgress] = useState<{ stage: string, current: number, total: number, message: string } | null>(null);
    const [collapsedGroups, setCollapsedGroups] = useState<Set<number>>(new Set());
    const [allDrives, setAllDrives] = useState<DriveInfo[]>([]);
    const [selectedSearchPaths, setSelectedSearchPaths] = useState<string[]>([]);
    const [searchOptions, setSearchOptions] = useState(() => {
        const saved = localStorage.getItem('duplicate_search_options');
        return saved ? JSON.parse(saved) : {
            byName: false,
            bySize: true,
            byContent: true
        };
    });

    React.useEffect(() => {
        localStorage.setItem('duplicate_search_options', JSON.stringify(searchOptions));
    }, [searchOptions]);
    const [filterQuery, setFilterQuery] = useState('');

    const toggleGroup = (groupIdx: number) => {
        setCollapsedGroups(prev => {
            const next = new Set(prev);
            if (next.has(groupIdx)) next.delete(groupIdx);
            else next.add(groupIdx);
            return next;
        });
    };

    const filteredDuplicates = React.useMemo(() => {
        if (!filterQuery.trim()) return duplicates;
        const q = filterQuery.toLowerCase();
        return duplicates.filter(group =>
            group.files.some(f => f.path.toLowerCase().includes(q) || f.name.toLowerCase().includes(q))
        );
    }, [duplicates, filterQuery]);


    const totalFileCount = React.useMemo(() => {
        return duplicates.reduce((sum, group) => sum + group.files.length, 0);
    }, [duplicates]);

    const dragRef = useRef<HTMLDivElement>(null);
    const { size, handleResizeStart } = useResizable({
        initialSize: (() => {
            const saved = localStorage.getItem('duplicate_search_dialog_size');
            return saved ? JSON.parse(saved) : { width: 832, height: 700 };
        })()
    });

    const { position, handleMouseDown } = useDraggable({ 
        initialPosition: { 
            x: (window.innerWidth - size.width) / 2, 
            y: (window.innerHeight - size.height) / 2 
        },
        dragRef 
    });

    React.useEffect(() => {
        localStorage.setItem('duplicate_search_dialog_size', JSON.stringify(size));
    }, [size]);

    React.useEffect(() => {
        // Fetch drives
        invoke<DriveInfo[]>('get_drives').then(drives => {
            setAllDrives(drives);
            // Default to searching in the current root
            if (initialRoot && !initialRoot.startsWith('oxyde://') && !initialRoot.startsWith('trash://')) {
                setSelectedSearchPaths([initialRoot]);
            } else if (drives.length > 0) {
                const cDrive = drives.find(d => d.path.startsWith('C:'));
                if (cDrive) setSelectedSearchPaths([cDrive.path]);
            }
        });
    }, [initialRoot]);

    const handleFindDuplicates = async () => {
        try {
            setDuplicatesError(null);
            setDuplicatesProgress(null);
            setIsSearchingDuplicates(true);
            setDisplayedDuplicatesCount(100);

            const unlisten = await listen<{ stage: string, current: number, total: number, message: string }>('duplicates_progress', (event) => {
                setDuplicatesProgress(event.payload);
            });

            const result = await invoke<{ size: number, files: FileEntry[] }[]>('find_duplicates', {
                paths: selectedSearchPaths,
                options: {
                    by_name: searchOptions.byName,
                    by_size: searchOptions.bySize,
                    by_content: searchOptions.byContent
                }
            });
            setDuplicates(result || []);
            unlisten();
        } catch (e: any) {
            console.error("Failed to find duplicates", e);
            setDuplicatesError(e.toString());
        } finally {
            setIsSearchingDuplicates(false);
            setDuplicatesProgress(null);
        }
    };

    const handleCancelDuplicates = async () => {
        try {
            await invoke('cancel_find_duplicates');
            setIsSearchingDuplicates(false);
        } catch (e) {
            console.error("Failed to cancel search", e);
        }
    };

    const { activePanel } = usePanelContext();

    const handleJumpToFolder = async (filePath: string) => {
        try {
            const lastSlashIndex = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'));
            const dirPath = lastSlashIndex >= 0 ? filePath.substring(0, lastSlashIndex) : filePath;
            const parent = dirPath.endsWith(':') ? dirPath + '\\' : dirPath;

            // 1. Navigate to the folder using panel state directly
            activePanel.navigate(parent);

            // 2. Select the file after a short delay (to let files load)
            // Increased delay to be more safe for deep folders or network drives
            setTimeout(() => {
                activePanel.setSelected(new Set([filePath]));
            }, 500);

            // 3. Dialogue stays open
        } catch (e) {
            console.error("Failed to jump to folder", e);
        }
    };

    return (
        <div className="dialog-overlay no-center" onMouseDown={onFocus} style={{ background: 'transparent', pointerEvents: 'none', zIndex: zIndex || 1000 }}>
            <div
                ref={dragRef}
                className="dialog-window duplicate-search-dialog"
                onClick={e => e.stopPropagation()}
                style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    width: `${size.width}px`,
                    height: `${size.height}px`,
                    transition: 'none',
                    pointerEvents: 'auto'
                }}
            >
                <div className="dialog-header" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <div className="dialog-title">
                        <Copy size={16} />
                        <span>{t('duplicates') || 'Duplicate Search'}</span>
                    </div>
                    <button className="dialog-close-btn btn-icon" onClick={onClose}>
                        <X size={16} />
                    </button>
                </div>

                <div className="dialog-content duplicate-search-content">
                    {/* Sidebar: Control Panel */}
                    <div className="duplicates-sidebar scrollbar-custom">
                            {/* Locations Section */}
                        <div className="sidebar-section">
                        <div className="dialog-section-title">
                            <Database size={14} />
                            <span>{t('search_locations' as any)}</span>
                        </div>

                            <div className="drive-table">
                                <div className="drive-table-body">
                                    {allDrives.map(drive => {
                                        const isSelected = selectedSearchPaths.includes(drive.path);
                                        return (
                                            <div
                                                key={drive.path}
                                                className={cx("drive-row", {
                                                    active: isSelected,
                                                    searching: isSearchingDuplicates
                                                })}
                                                onClick={() => {
                                                    if (isSelected) {
                                                        setSelectedSearchPaths(prev => prev.filter(p => p !== drive.path));
                                                    } else {
                                                        setSelectedSearchPaths(prev => [...prev, drive.path]);
                                                    }
                                                }}
                                            >
                                                <div className="drive-row-icon">
                                                    {getFileIcon(drive.label, true, { size: 14 }, useSystemIcons, drive.path)}
                                                </div>
                                                <div className="drive-row-content">
                                                    <span className="drive-label-text">
                                                        {(() => {
                                                            if (drive.remote_path) {
                                                                const parts = drive.remote_path.split(/[\\/]/).filter(Boolean);
                                                                return parts[parts.length - 1] || drive.remote_path;
                                                            }
                                                            return drive.label.replace(/#Disk\s\d+/g, '').replace(/#\d+/g, '').trim();
                                                        })()} ({drive.path.replace(/[\\/]$/, '')})
                                                    </span>
                                                    <div className="drive-meta-info">
                                                        {(drive.media_type || drive.drive_type === 'remote') && (
                                                            <span className={cx("drive-badge", { 
                                                                ssd: drive.media_type?.includes('SSD'),
                                                                hdd: drive.media_type?.includes('HDD'),
                                                                usb: drive.media_type?.includes('USB') || drive.drive_type === 'removable',
                                                                removable: drive.drive_type === 'removable',
                                                                remote: drive.drive_type === 'remote',
                                                                network: drive.drive_type === 'remote'
                                                            })}>
                                                                {drive.drive_type === 'remote' ? 'NETWORK' : (
                                                                    drive.media_type?.includes('SSD') ? 'SSD' : (
                                                                        drive.media_type?.includes('HDD') ? 'HDD' : drive.media_type
                                                                    )
                                                                )}
                                                            </span>
                                                        )}
                                                        {drive.physical_id !== undefined && drive.drive_type !== 'remote' && (
                                                            <span className="drive-badge">
                                                                Disk #{drive.physical_id.toString().replace(/Disk\s*/i, '')}
                                                            </span>
                                                        )}
                                                    </div>
                                                </div>
                                            </div>
                                        );
                                    })}

                                    {initialRoot && !allDrives.some(d => d.path === initialRoot) && !initialRoot.startsWith('oxyde://') && !initialRoot.startsWith('trash://') && (() => {
                                        const rootDrive = allDrives.find(d => initialRoot.startsWith(d.path));
                                        return (
                                            <div
                                                className={cx("drive-row", { active: selectedSearchPaths.includes(initialRoot), searching: isSearchingDuplicates })}
                                                onClick={() => {
                                                    if (selectedSearchPaths.includes(initialRoot)) {
                                                        setSelectedSearchPaths(prev => prev.filter(p => p !== initialRoot));
                                                    } else {
                                                        setSelectedSearchPaths(prev => [...prev, initialRoot]);
                                                    }
                                                }}
                                            >
                                                <div className="drive-row-icon">
                                                    {getFileIcon('current', true, { size: 14 }, useSystemIcons, initialRoot)}
                                                </div>
                                                <div className="drive-row-content">
                                                    <span className="drive-label-text">
                                                        {t('current_folder')}
                                                    </span>
                                                    <div className="drive-meta-info" style={{ flexWrap: 'wrap', gap: '0.2rem' }}>
                                                        {rootDrive && (rootDrive.media_type || rootDrive.drive_type === 'remote') && (
                                                            <span className={cx("drive-badge", { 
                                                                ssd: rootDrive.media_type?.includes('SSD'),
                                                                hdd: rootDrive.media_type?.includes('HDD'),
                                                                usb: rootDrive.media_type?.includes('USB') || rootDrive.drive_type === 'removable',
                                                                removable: rootDrive.drive_type === 'removable',
                                                                remote: rootDrive.drive_type === 'remote',
                                                                network: rootDrive.drive_type === 'remote'
                                                            })}>
                                                                {rootDrive.drive_type === 'remote' ? 'NETWORK' : (
                                                                    rootDrive.media_type?.includes('SSD') ? 'SSD' : (
                                                                        rootDrive.media_type?.includes('HDD') ? 'HDD' : rootDrive.media_type
                                                                    )
                                                                )}
                                                            </span>
                                                        )}
                                                        {rootDrive && rootDrive.physical_id !== undefined && rootDrive.drive_type !== 'remote' && (
                                                            <span className="drive-badge">
                                                                Disk #{rootDrive.physical_id.toString().replace(/Disk\s*/i, '')}
                                                            </span>
                                                        )}
                                                        <div style={{ width: '100%', marginTop: '0.1rem' }}>
                                                            <span className="drive-path-text" data-tooltip={initialRoot} style={{ display: 'block', opacity: 0.6, fontSize: '0.7rem' }}>
                                                                {initialRoot}
                                                            </span>
                                                        </div>
                                                    </div>
                                                </div>
                                            </div>
                                        );
                                    })()}
                                </div>
                            </div>

                            {selectedSearchPaths.length > 1 && (
                                <div className="location-warning">
                                    {t('multiple_locations_warning')}
                                </div>
                            )}
                        </div>



                        {/* Options Section */}
                        <div className="sidebar-section">
                        <div className="dialog-section-title">
                            <Settings2 size={14} />
                            <span>{t('search_settings' as any)}</span>
                        </div>
                            <div className="options-card">
                                <div className="checkbox-list">
                                    <label className="dialog-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={searchOptions.byName}
                                            onChange={() => setSearchOptions((prev: any) => ({ ...prev, byName: !prev.byName }))}
                                        />
                                        <div className="checkbox-visual">
                                            {searchOptions.byName && <Check size={10} strokeWidth={4} />}
                                        </div>
                                        <span>{t('search_by_name')}</span>
                                    </label>

                                    <label className="dialog-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={searchOptions.bySize}
                                            onChange={() => setSearchOptions((prev: any) => ({ ...prev, bySize: !prev.bySize }))}
                                        />
                                        <div className="checkbox-visual">
                                            {searchOptions.bySize && <Check size={10} strokeWidth={4} />}
                                        </div>
                                        <span>{t('search_by_size')}</span>
                                    </label>

                                    <label className="dialog-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={searchOptions.byContent}
                                            onChange={() => setSearchOptions((prev: any) => ({ ...prev, byContent: !prev.byContent }))}
                                        />
                                        <div className="checkbox-visual">
                                            {searchOptions.byContent && <Check size={10} strokeWidth={4} />}
                                        </div>
                                        <span>{t('search_by_content')}</span>
                                    </label>
                                </div>
                            </div>
                        </div>

                    </div>

                    {/* Main Area: Stats, Filter and Results */}
                    <div className="duplicates-main">
                        {/* Header Area: Group Count, File Count and Filter */}
                        <div className="main-results-header">
                            <div style={{ flex: 1 }}>
                                <div className="input-with-icon icon-left">
                                    <Search size={14} className="input-icon" />
                                    <input
                                        type="text"
                                        value={filterQuery}
                                        onChange={(e) => setFilterQuery(e.target.value)}
                                        placeholder={t('filter_duplicates') || 'Filter results...'}
                                    />
                                    {filterQuery && (
                                        <div className="input-actions-hint right">
                                            <button
                                                type="button"
                                                className="regex-badge-btn clear-filter-btn"
                                                onClick={() => setFilterQuery('')}
                                            >
                                                <X size={14} />
                                            </button>
                                        </div>
                                    )}
                                </div>
                            </div>
                            {duplicates.length > 0 && (
                                <div className="results-stats">
                                    <div className="results-stats-primary">{duplicates.length} {t('duplicate_groups')}</div>
                                    <div className="results-stats-secondary">{totalFileCount} {t('files')}</div>
                                </div>
                            )}
                        </div>

                        {/* Horizontal Separator */}
                        <div className="header-separator" />

                        {/* Progress Bar (if active) */}
                        {isSearchingDuplicates && duplicatesProgress && (
                            <div className="duplicates-progress-panel">
                                <div className="progress-header">
                                    <span className="progress-stage">{duplicatesProgress.stage}</span>
                                    {duplicatesProgress.total > 0 && (
                                        <span className="progress-counter">
                                            {duplicatesProgress.current} / {duplicatesProgress.total}
                                            <span className="progress-percentage">({Math.round((duplicatesProgress.current / duplicatesProgress.total) * 100)}%)</span>
                                        </span>
                                    )}
                                </div>
                                <div className="progress-bar-wrapper">
                                    <div 
                                        className={cx("progress-bar-fill", { pulse: duplicatesProgress.total === 0 })}
                                        style={{ width: duplicatesProgress.total > 0 ? `${(duplicatesProgress.current / duplicatesProgress.total) * 100}%` : '100%' }} 
                                    />
                                </div>
                                <div className="progress-message">
                                    {duplicatesProgress.message}
                                </div>
                            </div>
                        )}

                        {/* Results Area */}
                        <div className="duplicates-results-container">
                            {duplicatesError ? (
                                <div className="duplicate-error-state">
                                    <X size={48} style={{ opacity: 0.2, marginBottom: '1rem' }} />
                                    <div>{t('error') || 'Error'}: {duplicatesError}</div>
                                </div>
                            ) : filteredDuplicates.length > 0 ? (
                                <div className="duplicates-list">
                                    {filteredDuplicates.slice(0, displayedDuplicatesCount).map((group, gIdx) => {
                                        const isCollapsed = collapsedGroups.has(gIdx);
                                        return (
                                            <div key={gIdx} className="duplicate-group-card">
                                                <div
                                                    className={cx("duplicate-group-header", { collapsed: isCollapsed })}
                                                    onClick={() => toggleGroup(gIdx)}
                                                >
                                                    <div className="group-header-info">
                                                        {isCollapsed ? <ChevronRight size={16} /> : <ChevronDown size={16} />}
                                                        <span>{t('group')} {gIdx + 1}</span>
                                                        <span className="group-badge">
                                                            {group.files.length}
                                                        </span>
                                                    </div>
                                                    <div className="group-size">
                                                        {formatSize(group.size, 1, t)}
                                                    </div>
                                                </div>
                                                {!isCollapsed && (
                                                    <div className="dialog-pill-list">
                                                        {group.files.map((file, fIdx) => {
                                                            const lastSlashIndex = Math.max(file.path.lastIndexOf('/'), file.path.lastIndexOf('\\'));
                                                            const dirPath = lastSlashIndex >= 0 ? file.path.substring(0, lastSlashIndex + 1) : '';
                                                            const fileName = lastSlashIndex >= 0 ? file.path.substring(lastSlashIndex + 1) : file.path;

                                                            return (
                                                                <div key={fIdx}
                                                                    onClick={() => handleJumpToFolder(file.path)}
                                                                    className="dialog-pill"
                                                                    data-tooltip={t('jump_to_folder') || 'Jump to Folder'}
                                                                >
                                                                    <div className="dialog-pill-icon">
                    {getFileIcon(file.name, false, { size: 28 }, useSystemIcons, file.path)}
                </div>
                                                    <div className="dialog-pill-info">
                                                                        <div className="dialog-pill-title">
                                                                            {fileName}
                                                                        </div>
                                                                        <div className="dialog-pill-subtitle">
                                                                            {dirPath}
                                                                        </div>
                                                                    </div>
                                                                </div>
                                                            );
                                                        })}
                                                    </div>
                                                )}
                                            </div>
                                        );
                                    })}

                                    {displayedDuplicatesCount < filteredDuplicates.length && (
                                        <div className="load-more-container">
                                            <button
                                                type="button"
                                                className="btn secondary load-more-btn"
                                                onClick={() => setDisplayedDuplicatesCount(prev => prev + 100)}
                                            >
                                                {t('load_more') || 'Load More'}
                                            </button>
                                        </div>
                                    )}
                                </div>
                            ) : (
                                <div className="duplicate-empty-state">
                                    {isSearchingDuplicates
                                        ? <Loader2 className="spin" size={40} />
                                        : (
                                            <>
                                                <Search size={48} className="empty-icon" />
                                                <div className="empty-hint">
                                                    {duplicates.length > 0 && filterQuery
                                                        ? (t('no_duplicates_found') || 'No matching duplicates found with current filter')
                                                        : (t('duplicate_start_hint') || 'Configure your search and click "Find Duplicates"')}
                                                </div>
                                            </>
                                        )}
                                </div>
                            )}
                        </div>
                    </div>

                </div>

                <div className="dialog-footer">
                    {!isSearchingDuplicates ? (
                        <>
                            <button
                                type="button"
                                className="btn secondary"
                                onClick={onClose}
                            >
                                {t('cancel') || 'Annuler'}
                            </button>
                            <button
                                type="button"
                                className="btn primary"
                                onClick={handleFindDuplicates}
                                disabled={selectedSearchPaths.length === 0}
                            >
                                <Search size={14} />
                                {t('search') || 'Rechercher'}
                            </button>
                        </>
                    ) : (
                        <button
                            type="button"
                            className="btn danger"
                            onClick={handleCancelDuplicates}
                        >
                            <Loader2 size={14} className="spin" />
                            {t('cancel') || 'Annuler'}
                        </button>
                    )}
                </div>

                {/* Resize handle */}
                <div className="resize-handle" onMouseDown={handleResizeStart}>
                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                        <path d="M11 1L1 11M11 5L5 11M11 9L9 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                    </svg>
                </div>
            </div>
        </div>
    );
};
