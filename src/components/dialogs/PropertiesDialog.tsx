import React, { useEffect, useState, useRef } from 'react';
import { X, Folder, ChartBarBig, FileText, Link } from 'lucide-react';
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
}

export const PropertiesDialog: React.FC<PropertiesDialogProps> = ({ paths, initialEntries, onClose, t, notify }) => {
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




    const getIcon = (name: string, isDir: boolean, path?: string) => {
        return getFileIcon(name, isDir, { size: 48, strokeWidth: 1 }, useSystemIcons, path);
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

    // Handled by useDraggable hook

    if (!isSingle && !summary) return null; // Or a loader
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
        <div className="modal-overlay" style={{ background: 'transparent', pointerEvents: 'none' }}>
            <div
                ref={dragRef}
                className="properties-dialog"
                onClick={e => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none',
                    pointerEvents: 'auto'
                }}
            >
                <div className="prop-header-bar" onMouseDown={handleMouseDown}>
                    <div className="prop-title">{t('properties')}</div>
                    <button className="btn-icon" onClick={onClose}><X size={16} /></button>
                </div>

                {showTabs && (
                    <div className="prop-tabs">
                        <button
                            className={cx("prop-tab", { active: activeTab === 'general' })}
                            onClick={() => setActiveTab('general')}
                        >
                            <FileText size={14} />
                            {t('general')}
                        </button>

                        {isSingle && properties?.name.toLowerCase().endsWith('.lnk') && (
                            <button
                                className={cx("prop-tab", { active: activeTab === 'shortcut' })}
                                onClick={() => setActiveTab('shortcut')}
                            >
                                <Link size={14} />
                                {t('shortcut')}
                            </button>
                        )}
                    </div>
                )}

                <div className="prop-content">
                    {loading ? (
                        <div className="prop-loading">{t('loading' as any)}</div>
                    ) : (
                        <>
                            {activeTab === 'general' && (
                                <>
                                    <div className="prop-main-info">
                                        <div className="prop-icon-large">
                                            {isSingle ? getIcon(properties!.name, properties!.is_dir, properties!.path) : <Folder size={48} strokeWidth={1} />}
                                        </div>
                                        <div className="prop-name-input">
                                            {isSingle ? (() => {
                                                if (currentDrive) {
                                                    const label = currentDrive.label || t('local_disk' as any);
                                                    const letter = currentDrive.path.replace(/\\$/, '');
                                                    return `${label} (${letter})`;
                                                }
                                                return properties!.original_path
                                                    ? properties!.original_path.split('\\').pop() || properties!.name
                                                    : properties!.name;
                                            })() : `${summary!.count} items`}
                                        </div>
                                    </div>

                                    <div className="prop-divider" />

                                    {isSingle ? (
                                        <div className="prop-grid">
                                            <div className="prop-label">{t('type')}</div>
                                            <div className="prop-value">
                                                {isDriveRoot ? t('disk_drive' as any) : getFileTypeString(properties as any, t)}
                                            </div>

                                            {!isDriveRoot && (
                                                <>
                                                    <div className="prop-label">{t('location')}</div>
                                                    <div className="prop-value" data-tooltip={properties!.parent}>{properties!.parent}</div>
                                                </>
                                            )}

                                            {properties!.original_path && (
                                                <div className="prop-label">{t('original_location')}</div>
                                            )}
                                            {properties!.original_path && (() => {
                                                const parts = properties!.original_path.split('\\');
                                                parts.pop(); // Remove filename
                                                const dirPath = parts.join('\\');
                                                return (
                                                    <div className="prop-value" data-tooltip={dirPath}>
                                                        {dirPath}
                                                    </div>
                                                );
                                            })()}

                                            {properties!.deleted_time && (
                                                <div className="prop-label">{t('date_deleted')}</div>
                                            )}
                                            {properties!.deleted_time && (
                                                <div className="prop-value">
                                                    {formatDate(properties!.deleted_time, dateFormat, '-')}
                                                </div>
                                            )}

                                            {isDriveRoot && currentDrive && currentDrive.total_bytes !== undefined ? (
                                                <>
                                                    <div className="prop-label">{t('used_space')}</div>
                                                    <div className="prop-value">
                                                        {formatSize(currentDrive.total_bytes! - (currentDrive.free_bytes || 0), 1, t)} ({(currentDrive.total_bytes! - (currentDrive.free_bytes || 0)).toLocaleString()} {t('unit_bytes' as any)})
                                                    </div>

                                                    <div className="prop-label">{t('free_space')}</div>
                                                    <div className="prop-value">
                                                        {formatSize(currentDrive.free_bytes || 0, 1, t)} ({(currentDrive.free_bytes || 0).toLocaleString()} {t('unit_bytes' as any)})
                                                    </div>

                                                    <div className="prop-divider-row" />

                                                    <div className="prop-label">{t('capacity')}</div>
                                                    <div className="prop-value">
                                                        {formatSize(currentDrive.total_bytes!, 1, t)} ({currentDrive.total_bytes!.toLocaleString()} {t('unit_bytes' as any)})
                                                    </div>

                                                    <div style={{ gridColumn: '1 / -1', display: 'flex', justifyContent: 'center', padding: '0' }}>
                                                        <DiskUsageChart
                                                            total={currentDrive.total_bytes!}
                                                            free={currentDrive.free_bytes || 0}
                                                            size={120}
                                                            showText={false}
                                                        />
                                                    </div>
                                                </>
                                            ) : (
                                                <>
                                                    <div className="prop-label">{t('size')}</div>
                                                    <div className="prop-value">
                                                        {properties!.is_dir ? (
                                                            (localCalculated || properties!.is_calculated) ? (
                                                                (localCalculated?.size ?? properties!.size) === 0
                                                                    ? t('empty_dir' as any)
                                                                    : `${formatSize(localCalculated?.size ?? properties!.size, 1, t)} (${(localCalculated?.size ?? properties!.size).toLocaleString()} ${t('unit_bytes' as any)})`
                                                            ) : (
                                                                calcLoading ? (
                                                                    <span className="calc-status">{t('calculating' as any)}</span>
                                                                ) : (
                                                                    <button className="prop-btn xsmall" onClick={handleCalculate}>
                                                                        <ChartBarBig size={12} className="prop-btn-icon" /> {t('calculate_size' as any)}
                                                                    </button>
                                                                )
                                                            )
                                                        ) : (
                                                            `${formatSize(properties!.size, 1, t)} (${properties!.size.toLocaleString()} ${t('unit_bytes' as any)})`
                                                        )}
                                                    </div>

                                                    {showCounts && (
                                                        <>
                                                            <div className="prop-label">{t('contains')}</div>
                                                            <div className="prop-value">
                                                                {filesCount} {t(filesCount === 1 ? 'file' as any : 'files' as any)}, {foldersCount} {t(foldersCount === 1 ? 'folder' as any : 'folders' as any)}
                                                            </div>
                                                        </>
                                                    )}
                                                </>
                                            )}

                                            <div className="prop-divider-row" />

                                            <div className="prop-label">{t('created')}</div>
                                            <div className="prop-value">{formatDate(properties!.created, dateFormat, '-')}</div>

                                            <div className="prop-label">{t('modified')}</div>
                                            <div className="prop-value">{formatDate(properties!.modified, dateFormat, '-')}</div>

                                            <div className="prop-label">{t('accessed')}</div>
                                            <div className="prop-value">{formatDate(properties!.accessed, dateFormat, '-')}</div>

                                            <div className="prop-divider-row" />

                                            <div className="prop-label">{t('attributes')}</div>
                                            <div className="prop-attrs-static">
                                                {properties!.readonly && <span className="prop-badge">{t('readonly')}</span>}
                                                {properties!.is_hidden && <span className="prop-badge">{t('hidden')}</span>}
                                                {!properties!.readonly && !properties!.is_hidden && <span className="prop-text-muted">{t('none' as any)}</span>}
                                            </div>
                                        </div>
                                    ) : (
                                        <div className="prop-grid">
                                            <div className="prop-label">{t('location')}</div>
                                            <div className="prop-value">
                                                {summary!.parent_path ? summary!.parent_path : t('multiple_locations' as any)}
                                            </div>

                                            <div className="prop-label">{t('contains')}</div>
                                            <div className="prop-value">
                                                {summary!.files_count} {t(summary!.files_count === 1 ? 'file' as any : 'files' as any)}, {summary!.folders_count} {t(summary!.folders_count === 1 ? 'folder' as any : 'folders' as any)}
                                            </div>

                                            <div className="prop-label">{t('total_size')}</div>
                                            <div className="prop-value">{formatSize(summary!.total_size, 1, t)}</div>

                                            <div className="prop-divider-row" />

                                            <div className="prop-label">{t('attributes')}</div>
                                            <div className="prop-attrs-static">
                                                {summary!.all_readonly ? <span className="prop-badge">{t('readonly')}</span> : (summary!.any_readonly && <span className="prop-badge partial">{t('readonly')} (partial)</span>)}
                                                {summary!.all_hidden ? <span className="prop-badge">{t('hidden')}</span> : (summary!.any_hidden && <span className="prop-badge partial">{t('hidden')} (partial)</span>)}
                                                {!summary!.any_readonly && !summary!.any_hidden && <span className="prop-text-muted">{t('none' as any)}</span>}
                                            </div>
                                        </div>
                                    )}
                                </>
                            )}

                            {activeTab === 'shortcut' && properties?.shortcut && (
                                <div className="prop-grid">
                                    <div className="prop-label">{t('target' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.target}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, target: e.target.value }
                                        })}
                                    />

                                    <div className="prop-label">{t('arguments' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.arguments}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, arguments: e.target.value }
                                        })}
                                    />

                                    <div className="prop-label">{t('working_dir' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.working_dir}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, working_dir: e.target.value }
                                        })}
                                    />

                                    <div className="prop-label">{t('description' as any)}</div>
                                    <input
                                        type="text"
                                        className="input-field"
                                        value={properties.shortcut.description}
                                        onChange={(e) => setProperties({
                                            ...properties,
                                            shortcut: { ...properties.shortcut!, description: e.target.value }
                                        })}
                                    />

                                    <div className="prop-divider-row" />

                                    <div className="prop-label">{t('run_window' as any)}</div>
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
                                <div className="prop-placeholder">
                                    {t('shortcut_edit_hint' as any)}
                                </div>
                            )}
                        </>
                    )}
                </div>

                <div className="prop-footer spaced">
                    <button className="btn" onClick={() => invoke('show_system_properties', { path: paths[0] })}>
                        {t('system_properties')}
                    </button>
                    <div style={{ display: 'flex', gap: '0.5rem' }}>
                        <button className="btn" onClick={onClose}>{t('cancel')}</button>
                        <button className="btn primary" onClick={handleOk}>OK</button>
                    </div>
                </div>
            </div>
        </div>
    );
};


