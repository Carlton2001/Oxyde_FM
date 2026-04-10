import React, { useEffect, useState, useRef } from 'react';
import { X, Folder, ChartBarBig, FileText, Link, Globe } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { FileProperties, FileSummary, NotificationType, FileEntry, FolderSizeResult } from '../../types';
import { formatSize, formatDate, getFileTypeString } from '../../utils/format';
import { getFileIcon } from '../../utils/fileIcons';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import { useDraggable } from '../../hooks/useDraggable';
import { DiskUsageChart } from '../ui/DiskUsageChart';
import cx from 'classnames';
import '../../styles/components/Dialogs.css';

interface PropertiesDialogProps {
    paths: string[];
    initialEntries?: FileEntry[];
    onClose: () => void;
    t: TFunc;
    notify: (message: string, type?: NotificationType) => void;
    zIndex?: number;
    onFocus?: () => void;
}

export const PropertiesDialog: React.FC<PropertiesDialogProps> = ({ paths, initialEntries, onClose, t, notify, zIndex, onFocus }) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const [activeTab, setActiveTab] = useState('general');
    const [properties, setProperties] = useState<FileProperties | null>(null);
    const [summary, setSummary] = useState<FileSummary | null>(null);
    const [loading, setLoading] = useState(true);
    const [calcLoading, setCalcLoading] = useState(false);
    const [localCalculated, setLocalCalculated] = useState<FolderSizeResult | null>(null);

    const { useSystemIcons, dateFormat, drives } = useApp();

    const isSingle = paths.length === 1;

    useEffect(() => {
        const fetchProps = async () => {
            setLoading(true);
            try {
                if (isSingle) {
                    const props = await invoke<FileProperties>('get_file_properties', { path: paths[0] });

                    // Check if we already have calculation data from initialEntries
                    const initial = initialEntries?.find(e => e.path === paths[0]);
                    if (initial?.is_calculated) {
                        props.size = initial.size;
                        props.is_calculated = true;
                    }

                    setProperties(props);
                } else {
                    const sum = await invoke<FileSummary>('get_files_summary', { paths });
                    setSummary(sum);
                }
            } catch (error) {
                console.error("Failed to get properties", error);
                notify(`${t('error' as any)}: ${error}`, 'error');
            } finally {
                setLoading(false);
            }
        };
        fetchProps();
    }, [paths.join(',')]);

    // Auto-calculate folder size if it's a directory and not already calculated
    // Skip auto-calculate for drive roots as it can be very slow
    useEffect(() => {
        const isDrive = drives?.some(d =>
            d.path.toUpperCase() === properties?.path.toUpperCase() ||
            d.path.toUpperCase() === (properties?.path && !properties.path.endsWith('\\') ? properties.path + '\\' : properties?.path)?.toUpperCase()
        );

        if (isSingle && properties?.is_dir && !properties.is_calculated && !calcLoading && !localCalculated && !isDrive) {
            handleCalculate();
        }
    }, [isSingle, properties, calcLoading, localCalculated, drives]);

    const handleCalculate = async () => {
        if (!properties) return;
        setCalcLoading(true);
        try {
            const result = await invoke<FolderSizeResult>('calculate_folder_size', { path: properties.path });
            setLocalCalculated(result);
        } catch (e) {
            console.error("Failed to calculate size", e);
            notify(`${t('error' as any)}: ${e}`, 'error');
        } finally {
            setCalcLoading(false);
        }
    };

    const getIcon = (name: string, isDir: boolean, path?: string, isSymlink?: boolean, isJunction?: boolean) => {
        return getFileIcon(name, isDir, { size: 48, strokeWidth: 1 }, useSystemIcons, path, false, false, isSymlink, isJunction);
    };

    const handleOk = async () => {
        if (isSingle && properties?.shortcut) {
            try {
                await invoke('set_shortcut_info', { path: properties.path, info: properties.shortcut });
            } catch (e) {
                console.error("Failed to save shortcut info", e);
                notify(`${t('error' as any)}: ${e}`, 'error');
                return; // Don't close if save failed
            }
        }
        onClose();
    };

    if (!isSingle && !summary) return null;
    if (isSingle && !properties && !loading) return null;

    const hasCalculatedSize = !!localCalculated || properties?.is_calculated;
    const filesCount = localCalculated?.files_count ?? properties?.files_count;
    const foldersCount = localCalculated?.folders_count ?? properties?.folders_count;
    const showCounts = isSingle && properties?.is_dir && hasCalculatedSize && (filesCount !== undefined || foldersCount !== undefined);

    const showTabs = isSingle && properties?.name.toLowerCase().endsWith('.lnk');

    const currentDrive = isSingle && properties ? drives?.find(d =>
        d.path.toUpperCase() === properties.path.toUpperCase() ||
        d.path.toUpperCase() === (properties.path.endsWith('\\') ? properties.path : properties.path + '\\').toUpperCase()
    ) : null;
    const isDriveRoot = !!currentDrive;

    return (
        <div className="dialog-overlay" onClick={onClose} style={{ zIndex }}>
            <div
                ref={dragRef}
                className="dialog-window properties-dialog"
                onClick={e => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none',
                    pointerEvents: 'auto'
                }}
            >
                <div className="dialog-header" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <div className="dialog-title">{t('properties')}</div>
                    <button className="dialog-close-btn btn-icon" onClick={onClose}><X size={16} /></button>
                </div>

                {showTabs && (
                    <div className="dialog-tabs">
                        <button
                            className={cx("dialog-tab", { active: activeTab === 'general' })}
                            onClick={() => setActiveTab('general')}
                        >
                            <FileText size={14} />
                            {t('general')}
                        </button>

                        {isSingle && properties?.name.toLowerCase().endsWith('.lnk') && (
                            <button
                                className={cx("dialog-tab", { active: activeTab === 'shortcut' })}
                                onClick={() => setActiveTab('shortcut')}
                            >
                                <Link size={14} />
                                {t('shortcut')}
                            </button>
                        )}
                    </div>
                )}

                <div className="dialog-content">
                    {loading ? (
                        <div className="dialog-loading">{t('loading' as any)}</div>
                    ) : (
                        <>
                            {activeTab === 'general' && (
                                <>
                                    <div className="dialog-grid">
                                        <div className="dialog-main-info" style={{ gridColumn: '1 / -1' }}>
                                            <div className="dialog-icon-large">
                                                {isSingle ? getIcon(properties!.name, properties!.is_dir, properties!.path, properties!.is_symlink, properties!.is_junction) : <Folder size={48} strokeWidth={1} />}
                                            </div>
                                            <div className="dialog-name-input">
                                                {isSingle ? (() => {
                                                    if (currentDrive) {
                                                        let label = currentDrive.label;
                                                        if (currentDrive.drive_type === 'remote' && currentDrive.remote_path) {
                                                            const parts = currentDrive.remote_path.split(/[\\/]/).filter(Boolean);
                                                            label = parts[parts.length - 1] || currentDrive.remote_path;
                                                        } else {
                                                            label = (!label || label === 'Local Disk') ? t('local_disk' as any) : label;
                                                        }
                                                        const letter = currentDrive.path.replace(/\\$/, '');
                                                        return `${label} (${letter})`;
                                                    }
                                                    return properties!.original_path
                                                        ? properties!.original_path.split('\\').pop() || properties!.name
                                                        : properties!.name;
                                                })() : `${summary!.count} items`}
                                            </div>
                                        </div>

                                        {isSingle ? (
                                            <>
                                                <div className="dialog-label">{t('type')}</div>
                                                <div className="dialog-value">
                                                    {properties!.is_media_device 
                                                        ? t('network_device' as any) 
                                                        : (isDriveRoot && currentDrive 
                                                            ? (() => {
                                                                switch (currentDrive.drive_type) {
                                                                    case 'fixed': return t('local_disk' as any);
                                                                    case 'removable': return t('removable_disk' as any);
                                                                    case 'remote': return t('network_drive' as any);
                                                                    case 'cdrom': return t('cd_drive' as any);
                                                                    default: return t('disk_drive' as any);
                                                                }
                                                            })()
                                                            : (getFileTypeString(properties as any, t) || '—'))}
                                                </div>

                                                {properties!.is_media_device ? (
                                                    <>
                                                        <div className="dialog-label">{t('manufacturer' as any)}</div>
                                                        <div className="dialog-value">{properties!.manufacturer || '—'}</div>

                                                        <div className="dialog-label">{t('model' as any)}</div>
                                                        <div className="dialog-value">{properties!.model_name || '—'}</div>

                                                        <div className="dialog-label">{t('model_number' as any)}</div>
                                                        <div className="dialog-value">{properties!.model_number || '—'}</div>

                                                        <div className="dialog-divider-row" />

                                                        <div className="dialog-label">{t('serial_number' as any)}</div>
                                                        <div className="dialog-value">{properties!.serial_number || '—'}</div>

                                                        <div className="dialog-label">{t('mac_address' as any)}</div>
                                                        <div className="dialog-value">{properties!.mac_address || '—'}</div>

                                                        <div className="dialog-label">{t('unique_id' as any)}</div>
                                                        <div className="dialog-value" style={{ wordBreak: 'break-all', fontSize: '0.65rem' }}>{properties!.unique_id || '—'}</div>

                                                        <div className="dialog-label">{t('ip_address' as any)}</div>
                                                        <div className="dialog-value">{properties!.ip_address || '—'}</div>

                                                        {properties!.debug_props && properties!.debug_props.length > 0 && (
                                                            <>
                                                                <div className="dialog-divider-row"
 />
                                                                <div className="dialog-label">Debug Data</div>
                                                                <div className="dialog-value" style={{ gridColumn: '1 / -1' }}>
                                                                    <textarea
                                                                        readOnly
                                                                        rows={5}
                                                                        style={{ width: '100%', fontSize: '0.65rem', overflow: 'auto', backgroundColor: 'transparent', border: '1px solid var(--border-color)', color: 'var(--text-color)' }}
                                                                        value={properties!.debug_props.join('\n')}
                                                                    />
                                                                </div>
                                                            </>
                                                        )}
                                                    </>
                                                ) : (!isDriveRoot && (
                                                    <>
                                                        <div className="dialog-label">{t('location')}</div>
                                                        <div className="dialog-value" data-tooltip={properties!.parent}>{properties!.parent}</div>
                                                    </>
                                                ))}

                                                {properties!.original_path && (
                                                    <div className="dialog-label">{t('original_location')}</div>
                                                )}
                                                {properties!.original_path && (() => {
                                                    const parts = properties!.original_path.split('\\');
                                                    parts.pop(); // Remove filename
                                                    const dirPath = parts.join('\\');
                                                    return (
                                                        <div className="dialog-value" data-tooltip={dirPath}>
                                                            {dirPath}
                                                        </div>
                                                    );
                                                })()}

                                                {properties!.deleted_time && (
                                                    <div className="dialog-label">{t('date_deleted')}</div>
                                                )}
                                                {properties!.deleted_time && (
                                                    <div className="dialog-value">
                                                        {formatDate(properties!.deleted_time, dateFormat, '—')}
                                                    </div>
                                                )}

                                                {/* Section spécifique aux disques */}
                                                {isDriveRoot && currentDrive && currentDrive.total_bytes !== undefined && (
                                                    <>
                                                        {currentDrive.drive_type === 'remote' && currentDrive.remote_path && (
                                                            <>
                                                                <div className="dialog-label">{t('network_path' as any)}</div>
                                                                <div className="dialog-value">
                                                                    <input
                                                                        type="text"
                                                                        className="dialog-name-input"
                                                                        readOnly
                                                                        value={currentDrive.remote_path}
                                                                        style={{ height: '24px', fontSize: '0.8rem', padding: '0 8px' }}
                                                                        onClick={(e) => (e.target as HTMLInputElement).select()}
                                                                    />
                                                                </div>
                                                            </>
                                                        )}
                                                        <div className="dialog-label">{t('used_space')}</div>
                                                        <div className="dialog-value">
                                                            {formatSize(currentDrive.total_bytes! - (currentDrive.free_bytes || 0), 1, t)}
                                                        </div>

                                                        <div className="dialog-label">{t('free_space')}</div>
                                                        <div className="dialog-value">
                                                            {formatSize(currentDrive.free_bytes || 0, 1, t)}
                                                        </div>

                                                        <div className="dialog-label">{t('capacity')}</div>
                                                        <div className="dialog-value">
                                                            {formatSize(currentDrive.total_bytes!, 1, t)}
                                                        </div>

                                                        <div className="dialog-divider-row" style={{ gridColumn: '1 / -1', margin: '0.5rem 0' }} />
                                                        <div className="usage-chart-row" style={{ gridColumn: '1 / -1' }}>
                                                            <DiskUsageChart
                                                                total={currentDrive.total_bytes!}
                                                                free={currentDrive.free_bytes || 0}
                                                                inline={true}
                                                                showText={false}
                                                                t={t}
                                                            />
                                                        </div>
                                                    </>
                                                )}

                                                {/* Section spécifique aux fichiers/dossiers (non disques) */}
                                                {!isDriveRoot && !properties!.is_media_device && (
                                                    <>
                                                        <div className="dialog-label">{t('size')}</div>
                                                        <div className="dialog-value">
                                                            {properties!.is_dir ? (
                                                                (localCalculated || properties!.is_calculated) ? (
                                                                    (localCalculated?.size ?? properties!.size) === 0
                                                                        ? t('empty_dir' as any)
                                                                        : formatSize(localCalculated?.size ?? properties!.size, 1, t)
                                                                ) : (
                                                                    calcLoading ? (
                                                                        <span className="calc-status">{t('calculating' as any)}</span>
                                                                    ) : (
                                                                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                                                                            <span className="dimmed-placeholder">—</span>
                                                                            <button className="dialog-btn xsmall" onClick={handleCalculate} title={t('calculate_size' as any)}>
                                                                                <ChartBarBig size={12} />
                                                                            </button>
                                                                        </div>
                                                                    )
                                                                )
                                                            ) : (
                                                                formatSize(properties!.size, 1, t)
                                                            )}
                                                        </div>

                                                        {properties!.is_dir && (
                                                            <>
                                                                <div className="dialog-label">{t('contains')}</div>
                                                                <div className="dialog-value">
                                                                    {showCounts ? (
                                                                        <>{filesCount} {t(filesCount === 1 ? 'file' as any : 'files' as any)}, {foldersCount} {t(foldersCount === 1 ? 'folder' as any : 'folders' as any)}</>
                                                                    ) : (
                                                                        <span className="dialog-text-muted">—</span>
                                                                    )}
                                                                </div>
                                                            </>
                                                        )}
                                                    </>
                                                )}

                                                {/* Dates (toujours affichées pour un élément unique, disque ou fichier) */}
                                                {!properties!.is_media_device && (
                                                    <>
                                                        <div className="dialog-divider-row" style={{ gridColumn: '1 / -1', margin: '0.5rem 0' }} />

                                                        <div className="dialog-label">{t('created')}</div>
                                                        <div className="dialog-value">{formatDate(properties!.created, dateFormat, '—')}</div>

                                                        <div className="dialog-label">{t('modified')}</div>
                                                        <div className="dialog-value">{formatDate(properties!.modified, dateFormat, '—')}</div>

                                                        <div className="dialog-label">{t('accessed')}</div>
                                                        <div className="dialog-value">{formatDate(properties!.accessed, dateFormat, '—')}</div>
                                                    </>
                                                )}
                                            </>
                                        ) : (
                                            <>
                                                <div className="dialog-label">{t('location')}</div>
                                                <div className="dialog-value">
                                                    {summary!.parent_path ? summary!.parent_path : t('multiple_locations' as any)}
                                                </div>

                                                <div className="dialog-label">{t('contains')}</div>
                                                <div className="dialog-value">
                                                    {summary!.files_count} {t(summary!.files_count === 1 ? 'file' as any : 'files' as any)}, {summary!.folders_count} {t(summary!.folders_count === 1 ? 'folder' as any : 'folders' as any)}
                                                </div>

                                                <div className="dialog-label">{t('total_size')}</div>
                                                <div className="dialog-value">{formatSize(summary!.total_size, 1, t)}</div>
                                            </>
                                        )}
                                    </div>
                                </>
                            )}


                            {activeTab === 'shortcut' && properties?.shortcut && (
                                <div className="dialog-grid">
                                    <div className="dialog-label">{t('target' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.target}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, target: e.target.value }
                                        })}
                                    />

                                    <div className="dialog-label">{t('arguments' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.arguments}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, arguments: e.target.value }
                                        })}
                                    />

                                    <div className="dialog-label">{t('working_dir' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.working_dir}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, working_dir: e.target.value }
                                        })}
                                    />

                                    <div className="dialog-label">{t('description' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.description}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, description: e.target.value }
                                        })}
                                    />

                                    <div className="dialog-divider-row" />

                                    <div className="dialog-label">{t('run_window' as any)}</div>
                                    <select
                                        className="select-field"
                                        value={properties.shortcut.run_window}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, run_window: parseInt(e.target.value) }
                                        })}
                                    >
                                        <option value={1}>{t('normal_window' as any)}</option>
                                        <option value={3}>{t('maximized_window' as any)}</option>
                                        <option value={7}>{t('minimized_window' as any)}</option>
                                    </select>
                                </div>
                            )}

                            {activeTab === 'shortcut' && !properties?.shortcut && (
                                <div className="dialog-placeholder">
                                    {t('shortcut_edit_hint' as any)}
                                </div>
                            )}
                        </>
                    )}
                </div>

                <div className="dialog-footer">
                    <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                        <button className="btn" onClick={() => invoke('show_system_properties', { path: paths[0] })}>
                            {t('system_properties')}
                        </button>
                        {properties?.is_media_device && (properties?.has_web_page || initialEntries?.[0]?.has_web_page) && (
                            <button className="btn" onClick={() => invoke('open_item', { path: paths[0] })}>
                                <Globe size={14} />
                                {t('view_device_webpage' as any)}
                            </button>
                        )}
                    </div>
                    <div style={{ display: 'flex', gap: '0.5rem' }}>
                        <button className="btn" onClick={onClose}>{t('cancel')}</button>
                        <button className="btn primary" onClick={handleOk}>OK</button>
                    </div>
                </div>
            </div>
        </div>
    );
};
