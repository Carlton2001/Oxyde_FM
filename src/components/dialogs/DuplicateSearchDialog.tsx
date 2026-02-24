import React, { useState, useRef } from 'react';
import { X, Search, Folder, Copy, Loader2, File as FileIcon, HardDrive, Usb, Disc, ChevronRight, ChevronDown, Check } from 'lucide-react';
import { useDraggable } from '../../hooks/useDraggable';
import { useResizable } from '../../hooks/useResizable';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { FileEntry, DriveInfo } from '../../types';
import { usePanelContext } from '../../context/PanelContext';
import './SearchDialog.css';

interface DuplicateSearchDialogProps {
    initialRoot: string;
    onClose: () => void;
    t: any;
}

export const DuplicateSearchDialog: React.FC<DuplicateSearchDialogProps> = ({
    initialRoot,
    onClose,
    t
}) => {
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
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const { size, handleResizeStart } = useResizable({
        initialSize: (() => {
            const saved = localStorage.getItem('duplicate_search_dialog_size');
            return saved ? JSON.parse(saved) : { width: 832, height: 700 };
        })()
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

    const formatSize = (bytes: number) => {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
        return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
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
        <div style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            pointerEvents: 'none',
            zIndex: 1000
        }}>
            <div
                ref={dragRef}
                className="properties-dialog"
                onClick={e => e.stopPropagation()}
                style={{
                    position: 'absolute',
                    left: '50%',
                    top: '50%',
                    marginLeft: '-416px', // Half of default width (832)
                    marginTop: '-350px',  // Half of default height (700)
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    width: `${size.width}px`,
                    height: `${size.height}px`,
                    transition: 'none',
                    pointerEvents: 'auto',
                    display: 'flex',
                    flexDirection: 'column',
                    border: '1px solid var(--border-color)',
                    maxWidth: '95vw',
                    maxHeight: '92vh',
                    boxShadow: '0 10px 40px rgba(0,0,0,0.3)'
                }}
            >
                <div className="modal-header" onMouseDown={handleMouseDown}>
                    <div className="modal-title">
                        <Copy size={16} />
                        <span>{t('duplicates') || 'Duplicate Search'}</span>
                    </div>
                    <button className="btn-icon" onClick={onClose}>
                        <X size={16} />
                    </button>
                </div>

                <div className="modal-content" style={{ padding: 0, flex: 1, minHeight: 0, display: 'flex', flexDirection: 'row', overflow: 'hidden' }}>
                    {/* Sidebar: Control Panel */}
                    <div className="duplicates-sidebar">
                        {/* Locations Section */}
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
                            <div style={{ fontSize: '0.75rem', fontWeight: 700, color: 'var(--text-secondary)', display: 'flex', alignItems: 'center', gap: '0.5rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                                <Folder size={14} />
                                {t('search_locations') || 'Search Locations'}
                            </div>

                            <div className="vertical-list-group">
                                {allDrives.map(drive => {
                                    const isSelected = selectedSearchPaths.includes(drive.path);
                                    return (
                                        <button
                                            key={drive.path}
                                            type="button"
                                            onClick={() => {
                                                if (isSelected) {
                                                    setSelectedSearchPaths(prev => prev.filter(p => p !== drive.path));
                                                } else {
                                                    setSelectedSearchPaths(prev => [...prev, drive.path]);
                                                }
                                            }}
                                            style={{
                                                padding: '0.6rem 0.8rem',
                                                borderRadius: '8px',
                                                fontSize: '0.8rem',
                                                display: 'flex',
                                                flexDirection: 'column',
                                                alignItems: 'flex-start',
                                                gap: '0.1rem',
                                                border: '1px solid var(--border-color)',
                                                background: isSelected ? 'rgba(var(--accent-color-rgb), 0.12)' : 'var(--bg-color)',
                                                borderColor: isSelected ? 'var(--accent-color)' : 'var(--border-color)',
                                                color: 'var(--text-color)',
                                                cursor: 'pointer',
                                                transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
                                                opacity: isSearchingDuplicates ? 0.6 : 1,
                                                pointerEvents: isSearchingDuplicates ? 'none' : 'auto',
                                                boxShadow: isSelected ? '0 2px 8px rgba(var(--accent-color-rgb), 0.15)' : 'none',
                                                width: '100%'
                                            }}
                                        >
                                            <div style={{ display: 'flex', alignItems: 'center', gap: '0.6rem', width: '100%' }}>
                                                <div style={{
                                                    display: 'flex',
                                                    alignItems: 'center',
                                                    justifyContent: 'center',
                                                    color: isSelected ? 'var(--accent-color)' : 'var(--text-muted)',
                                                    opacity: isSelected ? 1 : 0.7
                                                }}>
                                                    {drive.drive_type === 'removable' ? <Usb size={16} /> :
                                                        drive.drive_type === 'cdrom' ? <Disc size={16} /> :
                                                            <HardDrive size={16} />}
                                                </div>
                                                <span style={{ fontWeight: 600, whiteSpace: 'nowrap', color: 'var(--text-color)', textAlign: 'left', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                                                    {drive.label.replace(/#Disk\s\d+/g, '').replace(/#\d+/g, '').trim()} ({drive.path.replace(/[\\/]$/, '')})
                                                </span>
                                            </div>
                                            <div style={{ display: 'flex', gap: '0.4rem', width: '100%', justifyContent: 'flex-start', marginLeft: '1.6rem', alignItems: 'center', marginTop: '0.1rem' }}>
                                                {drive.media_type && (
                                                    <span style={{
                                                        fontSize: '0.6rem',
                                                        padding: '0.1rem 0.45rem',
                                                        borderRadius: '4px',
                                                        background: drive.media_type.includes('SSD') ? '#10b981' : 'var(--surface-secondary, rgba(0,0,0,0.06))',
                                                        color: drive.media_type.includes('SSD') ? '#ffffff' : 'var(--text-muted)',
                                                        fontWeight: 800,
                                                        textTransform: 'uppercase'
                                                    }}>
                                                        {drive.media_type}
                                                    </span>
                                                )}
                                                {drive.physical_id !== undefined && (
                                                    <span style={{
                                                        fontSize: '0.6rem',
                                                        padding: '0.1rem 0.45rem',
                                                        borderRadius: '4px',
                                                        background: 'var(--surface-secondary, rgba(0,0,0,0.06))',
                                                        color: 'var(--text-muted)',
                                                        fontWeight: 800,
                                                        textTransform: 'uppercase'
                                                    }}>
                                                        Disk #{drive.physical_id.toString().replace(/Disk\s*/i, '')}
                                                    </span>
                                                )}
                                            </div>
                                        </button>
                                    );
                                })}

                                {initialRoot && !allDrives.some(d => d.path === initialRoot) && !initialRoot.startsWith('oxyde://') && !initialRoot.startsWith('trash://') && (
                                    <button
                                        type="button"
                                        onClick={() => {
                                            if (selectedSearchPaths.includes(initialRoot)) {
                                                setSelectedSearchPaths(prev => prev.filter(p => p !== initialRoot));
                                            } else {
                                                setSelectedSearchPaths(prev => [...prev, initialRoot]);
                                            }
                                        }}
                                        style={{
                                            padding: '0.6rem 0.8rem',
                                            borderRadius: '8px',
                                            fontSize: '0.8rem',
                                            display: 'flex',
                                            flexDirection: 'column',
                                            alignItems: 'flex-start',
                                            gap: '0.1rem',
                                            border: '1px solid var(--border-color)',
                                            background: selectedSearchPaths.includes(initialRoot) ? 'rgba(var(--accent-color-rgb), 0.12)' : 'var(--bg-color)',
                                            borderColor: selectedSearchPaths.includes(initialRoot) ? 'var(--accent-color)' : 'var(--border-color)',
                                            color: 'var(--text-color)',
                                            cursor: 'pointer',
                                            transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
                                            opacity: isSearchingDuplicates ? 0.6 : 1,
                                            pointerEvents: isSearchingDuplicates ? 'none' : 'auto',
                                            boxShadow: selectedSearchPaths.includes(initialRoot) ? '0 2px 8px rgba(var(--accent-color-rgb), 0.15)' : 'none',
                                            width: '100%'
                                        }}
                                    >
                                        <div style={{ display: 'flex', alignItems: 'center', gap: '0.6rem', width: '100%' }}>
                                            <div style={{
                                                display: 'flex',
                                                alignItems: 'center',
                                                justifyContent: 'center',
                                                color: selectedSearchPaths.includes(initialRoot) ? 'var(--accent-color)' : 'var(--text-muted)',
                                                opacity: selectedSearchPaths.includes(initialRoot) ? 1 : 0.7
                                            }}>
                                                <Folder size={16} />
                                            </div>
                                            <span style={{ fontWeight: 600, color: 'var(--text-color)', textAlign: 'left' }}>
                                                {t('current_folder')}
                                            </span>
                                        </div>
                                        <div style={{ display: 'flex', gap: '0.4rem', width: '100%', justifyContent: 'flex-start', marginLeft: '1.6rem', alignItems: 'center', marginTop: '0.1rem' }}>
                                            <span style={{ fontSize: '0.65rem', color: 'var(--text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: '12rem', whiteSpace: 'nowrap' }}>
                                                {initialRoot}
                                            </span>
                                        </div>
                                    </button>
                                )}
                            </div>

                            {selectedSearchPaths.length > 1 && (
                                <div style={{
                                    fontSize: '0.75rem',
                                    color: '#d97706',
                                    background: '#fffbeb',
                                    padding: '0.75rem',
                                    borderRadius: '8px',
                                    border: '1px solid #fef3c7',
                                    marginTop: '0.5rem',
                                    lineHeight: '1.4'
                                }}>
                                    {t('multiple_locations_warning')}
                                </div>
                            )}
                        </div>

                        <div style={{ height: '1px', background: 'var(--border-color)', margin: '0.5rem 0' }} />

                        {/* Options Section */}
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
                            <div style={{ fontSize: '0.75rem', fontWeight: 700, color: 'var(--text-secondary)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>
                                {t('scan_options')}
                            </div>
                            <div className="checkbox-list">
                                <label className="prop-checkbox">
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

                                <label className="prop-checkbox">
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

                                <label className="prop-checkbox">
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

                        <div style={{ flex: 1 }} />

                        {/* Action Buttons at bottom of sidebar */}
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', marginTop: '1rem' }}>
                            {!isSearchingDuplicates ? (
                                <button
                                    type="button"
                                    className="btn primary"
                                    onClick={handleFindDuplicates}
                                    disabled={selectedSearchPaths.length === 0}
                                    style={{ width: '100%' }}
                                >
                                    <Search size={14} style={{ marginRight: '0.5rem' }} />
                                    {t('find_duplicates') || 'Find Duplicates'}
                                </button>
                            ) : (
                                <button
                                    type="button"
                                    className="btn danger"
                                    onClick={handleCancelDuplicates}
                                    style={{ width: '100%' }}
                                >
                                    <Loader2 size={14} className="spin" style={{ marginRight: '0.5rem' }} />
                                    {t('cancel') || 'Cancel'}
                                </button>
                            )}
                        </div>
                    </div>

                    {/* Main Area: Stats, Filter and Results */}
                    <div className="duplicates-main">
                        {/* Header Area: Group Count, File Count and Filter */}
                        <div style={{ padding: '0.6rem 1rem', display: 'flex', gap: '1rem', alignItems: 'center' }}>
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
                                                className="regex-badge-btn"
                                                onClick={() => setFilterQuery('')}
                                                style={{ borderLeft: 'none', background: 'transparent' }}
                                            >
                                                <X size={14} />
                                            </button>
                                        </div>
                                    )}
                                </div>
                            </div>
                            {duplicates.length > 0 && (
                                <div style={{
                                    fontSize: '0.825rem',
                                    color: 'var(--text-secondary)',
                                    whiteSpace: 'nowrap',
                                    paddingLeft: '1rem',
                                    display: 'flex',
                                    flexDirection: 'column',
                                    alignItems: 'flex-end',
                                    gap: '0.1rem'
                                }}>
                                    <div style={{ fontWeight: 700, color: 'var(--text-primary)' }}>{duplicates.length} {t('duplicate_groups')}</div>
                                    <div style={{ fontSize: '0.75rem', opacity: 0.8 }}>{totalFileCount} {t('files')}</div>
                                </div>
                            )}
                        </div>

                        {/* Horizontal Separator */}
                        <div style={{ height: '1px', background: 'var(--border-color)', width: '100%' }} />

                        {/* Progress Bar (if active) */}
                        {isSearchingDuplicates && duplicatesProgress && (
                            <div style={{ padding: '0.75rem 1rem', background: 'rgba(var(--accent-color-rgb), 0.05)', borderBottom: '1px solid var(--border-color)' }}>
                                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.8rem', marginBottom: '0.4rem' }}>
                                    <span style={{ fontWeight: 700, color: 'var(--accent-color)' }}>{duplicatesProgress.stage}</span>
                                    {duplicatesProgress.total > 0 && (
                                        <span style={{ fontWeight: 600 }}>
                                            {duplicatesProgress.current} / {duplicatesProgress.total}
                                            <span style={{ marginLeft: '0.5rem', opacity: 0.7 }}>({Math.round((duplicatesProgress.current / duplicatesProgress.total) * 100)}%)</span>
                                        </span>
                                    )}
                                </div>
                                <div style={{ height: '8px', background: 'var(--surface-secondary)', borderRadius: '4px', overflow: 'hidden', marginBottom: '0.4rem' }}>
                                    <div style={{
                                        height: '100%',
                                        background: 'var(--accent-color)',
                                        width: duplicatesProgress.total > 0 ? `${(duplicatesProgress.current / duplicatesProgress.total) * 100}%` : '100%',
                                        transition: 'width 0.3s cubic-bezier(0.4, 0, 0.2, 1)',
                                        animation: duplicatesProgress.total === 0 ? 'pulse 1.5s infinite alternate' : 'none'
                                    }} />
                                </div>
                                <div style={{ fontSize: '0.75rem', color: 'var(--text-secondary)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                                    {duplicatesProgress.message}
                                </div>
                            </div>
                        )}

                        {/* Results Area */}
                        <div className="duplicates-results-container">
                            {duplicatesError ? (
                                <div className="duplicates-empty-state" style={{ color: 'var(--danger-color)', padding: '2rem' }}>
                                    <X size={48} style={{ opacity: 0.2, marginBottom: '1rem' }} />
                                    <div>{t('error') || 'Error'}: {duplicatesError}</div>
                                </div>
                            ) : filteredDuplicates.length > 0 ? (
                                <div className="duplicates-list" style={{
                                    display: 'block',
                                    width: '100%',
                                    paddingBottom: '2rem'
                                }}>
                                    {filteredDuplicates.slice(0, displayedDuplicatesCount).map((group, gIdx) => {
                                        const isCollapsed = collapsedGroups.has(gIdx);
                                        return (
                                            <div key={gIdx} className="duplicate-group-card">
                                                <div
                                                    className={`duplicate-group-header ${isCollapsed ? 'collapsed' : ''}`}
                                                    onClick={() => toggleGroup(gIdx)}
                                                >
                                                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px', fontSize: '14px', fontWeight: 'bold' }}>
                                                        {isCollapsed ? <ChevronRight size={16} /> : <ChevronDown size={16} />}
                                                        <span>{t('group')} {gIdx + 1}</span>
                                                        <span style={{
                                                            fontSize: '11px',
                                                            padding: '2px 8px',
                                                            borderRadius: '10px',
                                                            background: 'var(--accent-color)',
                                                            color: '#ffffff'
                                                        }}>
                                                            {group.files.length}
                                                        </span>
                                                    </div>
                                                    <div style={{ fontSize: '12px', fontWeight: 'bold' }}>
                                                        {formatSize(group.size)}
                                                    </div>
                                                </div>
                                                {!isCollapsed && (
                                                    <div style={{ display: 'block' }}>
                                                        {group.files.map((file, fIdx) => {
                                                            const lastSlashIndex = Math.max(file.path.lastIndexOf('/'), file.path.lastIndexOf('\\'));
                                                            const dirPath = lastSlashIndex >= 0 ? file.path.substring(0, lastSlashIndex + 1) : '';
                                                            const fileName = lastSlashIndex >= 0 ? file.path.substring(lastSlashIndex + 1) : file.path;

                                                            return (
                                                                <div key={fIdx}
                                                                    onClick={() => handleJumpToFolder(file.path)}
                                                                    className="duplicate-file-item"
                                                                    title={t('jump_to_folder') || 'Jump to Folder'}
                                                                >
                                                                    <FileIcon size={16} style={{ color: 'var(--accent-color)', flexShrink: 0 }} />
                                                                    <div style={{ flex: 1, minWidth: 0, display: 'block' }}>
                                                                        <div style={{ fontSize: '13px', fontWeight: '600', color: 'var(--text-color)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block' }}>
                                                                            {fileName}
                                                                        </div>
                                                                        <div style={{ fontSize: '11px', color: 'var(--text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block' }}>
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
                                        <div style={{ display: 'flex', justifyContent: 'center', padding: '20px' }}>
                                            <button
                                                type="button"
                                                className="btn secondary"
                                                onClick={() => setDisplayedDuplicatesCount(prev => prev + 100)}
                                                style={{ padding: '8px 25px', borderRadius: '20px' }}
                                            >
                                                {t('load_more') || 'Load More'}
                                            </button>
                                        </div>
                                    )}
                                </div>
                            ) : (
                                <div className="duplicates-empty-state" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', opacity: 0.5 }}>
                                    {isSearchingDuplicates
                                        ? <Loader2 className="spin" size={40} />
                                        : (
                                            <>
                                                <Search size={48} style={{ marginBottom: '1rem' }} />
                                                <div style={{ fontSize: '1rem' }}>
                                                    {duplicates.length > 0 && filterQuery
                                                        ? (t('no_results') || 'No matching duplicates found with current filter')
                                                        : (t('duplicate_start_hint') || 'Configure your search and click "Find Duplicates"')}
                                                </div>
                                            </>
                                        )}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Resize handle */}
                    <div
                        onMouseDown={handleResizeStart}
                        style={{
                            position: 'absolute',
                            right: 0,
                            bottom: 0,
                            width: '20px',
                            height: '20px',
                            cursor: 'nwse-resize',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            zIndex: 100,
                            color: 'var(--text-muted)',
                            opacity: 0.5
                        }}
                    >
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                            <path d="M11 1L1 11M11 5L5 11M11 9L9 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                        </svg>
                    </div>
                </div>
            </div>
        </div>
    );
};
