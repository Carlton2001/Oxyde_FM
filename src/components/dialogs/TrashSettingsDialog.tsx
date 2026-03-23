import React, { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Trash, HardDrive, Settings2, Database, X } from 'lucide-react';
import cx from 'classnames';
import { DriveInfo } from '../../types';
import { TFunc } from '../../i18n';
import { formatSize } from '../../utils/format';
import { Toggle } from '../ui/Toggle';
import { useDraggable } from '../../hooks/useDraggable';
import './TrashSettingsDialog.css';

interface TrashSystemConfig {
    drive_path: string;
    max_size_mb: number;
    nuke_on_delete: boolean;
}

interface TrashSettingsDialogProps {
    isOpen: boolean;
    onClose: () => void;
    t: TFunc;
    drives: DriveInfo[];
    confirmDeleteGlobal: boolean;
    onUpdateGlobalConfirm: (value: boolean) => void;
    isTrashEmpty: boolean;
    refreshConfigs?: () => void;
}

export const TrashSettingsDialog: React.FC<TrashSettingsDialogProps> = ({
    isOpen,
    onClose,
    t,
    drives,
    confirmDeleteGlobal,
    onUpdateGlobalConfirm,
    isTrashEmpty,
    refreshConfigs
}) => {
    const [selectedDrivePath, setSelectedDrivePath] = useState<string | null>(drives[0]?.path || null);
    const [configs, setConfigs] = useState<Record<string, TrashSystemConfig>>({});
    const [initialConfigs, setInitialConfigs] = useState<Record<string, TrashSystemConfig>>({});
    const [units, setUnits] = useState<Record<string, 'mb' | 'percent'>>({});
    const [isLoading, setIsLoading] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [totalUsage, setTotalUsage] = useState<{ size: number, items: number } | null>(null);

    useEffect(() => {
        if (isOpen && drives.length > 0) {
            fetchAllConfigs();
        }
    }, [isOpen, drives]);

    const fetchAllConfigs = async () => {
        setIsLoading(true);
        const newConfigs: Record<string, TrashSystemConfig> = {};
        const newUnits: Record<string, 'mb' | 'percent'> = {};
        try {
            // Fetch total usage
            const [usageSize, usageItems] = await invoke<[number, number]>('get_total_recycle_bin_usage');
            setTotalUsage({ size: usageSize, items: usageItems });

            const results = await Promise.all(
                drives.map(async (drive) => {
                    try {
                        const config: TrashSystemConfig = await invoke('get_recycle_bin_config', { drivePath: drive.path });
                        return { path: drive.path, config };
                    } catch (err) {
                        console.error(`Failed to fetch config for ${drive.path}:`, err);
                        return null;
                    }
                })
            );

            results.forEach(res => {
                if (res) {
                    newConfigs[res.path] = res.config;
                    newUnits[res.path] = 'mb';
                }
            });

            setConfigs(newConfigs);
            setInitialConfigs(JSON.parse(JSON.stringify(newConfigs)));
            setUnits(newUnits);
        } catch (err) {
            console.error("Failed to fetch all trash configs:", err);
        }
        if (!selectedDrivePath && drives.length > 0) {
            setSelectedDrivePath(drives[0].path);
        }
        setIsLoading(false);
    };

    const handleSave = async () => {
        const changedPaths = Object.keys(configs).filter(path => {
            const current = configs[path];
            const initial = initialConfigs[path];
            return !initial || 
                   current.max_size_mb !== initial.max_size_mb || 
                   current.nuke_on_delete !== initial.nuke_on_delete;
        });

        if (changedPaths.length === 0) {
            onClose();
            return;
        }

        setIsSaving(true);
        try {
            for (const drivePath of changedPaths) {
                await invoke('set_recycle_bin_config', { config: configs[drivePath] });
            }
            if (refreshConfigs) refreshConfigs();
            onClose();
        } catch (err) {
            console.error("Failed to save trash configs:", err);
        } finally {
            setIsSaving(false);
        }
    };

    const updateCurrentConfig = (updates: Partial<TrashSystemConfig>) => {
        if (!selectedDrivePath) return;
        setConfigs(prev => ({
            ...prev,
            [selectedDrivePath]: {
                ...prev[selectedDrivePath],
                ...updates
            }
        }));
    };

    const selectedDriveInfo = useMemo(() => 
        drives.find(d => d.path === selectedDrivePath), 
    [drives, selectedDrivePath]);

    const currentConfig = selectedDrivePath ? configs[selectedDrivePath] : null;
    const currentUnit = selectedDrivePath ? units[selectedDrivePath] : 'mb';

    const getDisplayValue = () => {
        if (!currentConfig || !selectedDriveInfo) return 0;
        if (currentUnit === 'mb') return currentConfig.max_size_mb;
        
        const totalMb = (selectedDriveInfo.total_bytes || 0) / (1024 * 1024);
        if (totalMb <= 0) return 0;
        return Math.round((currentConfig.max_size_mb / totalMb) * 100);
    };

    const handleValueChange = (val: number) => {
        if (!selectedDrivePath || !selectedDriveInfo) return;
        
        if (currentUnit === 'mb') {
            updateCurrentConfig({ max_size_mb: val });
        } else {
            const totalMb = (selectedDriveInfo.total_bytes || 0) / (1024 * 1024);
            const newMb = Math.round((val / 100) * totalMb);
            updateCurrentConfig({ max_size_mb: newMb });
        }
    };

    const dragRef = React.useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });

    if (!isOpen) return null;

    return (
        <div className="modal-overlay" style={{ background: 'transparent', pointerEvents: 'none' }}>
            <div 
                ref={dragRef}
                className="properties-dialog trash-settings-modal" 
                onClick={e => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none',
                    pointerEvents: 'auto'
                }}
            >
                <div className="prop-header-bar" onMouseDown={handleMouseDown}>
                    <div className="prop-title">{t('trash_properties' as any)}</div>
                    <button className="btn-icon" onClick={onClose}><X size={16} /></button>
                </div>

                <div className="prop-content">
                    {totalUsage && (
                        <div className="ts-total-usage">
                            <div className="ts-usage-info">
                                <Trash className="icon-sm" style={{ color: 'var(--accent-color)' }} />
                                <div className="ts-usage-text">
                                    <span className="ts-usage-size">{formatSize(totalUsage.size, 1, t)}</span>
                                    <span className="ts-usage-count">({totalUsage.items} {totalUsage.items > 1 ? t('items' as any) : t('item' as any)})</span>
                                </div>
                            </div>
                            <button 
                                className="btn secondary small"
                                disabled={isTrashEmpty}
                                onClick={async () => {
                                    await invoke('empty_trash');
                                    fetchAllConfigs();
                                }}
                            >
                                {t('empty_recycle_bin' as any)}
                            </button>
                        </div>
                    )}

                    <div className="ts-section">
                        <div className="ts-section-header">
                            <Database className="icon-xs" />
                            <span>{t('recycle_bin_location' as any)}</span>
                        </div>
                        
                        <div className="ts-drive-table">
                            <div className="ts-table-header">
                                <div className="col-name">{t('location' as any)}</div>
                                <div className="col-space">{t('available_space' as any)}</div>
                            </div>
                            <div className="ts-table-body scrollbar-custom">
                                {drives.map(drive => (
                                    <div 
                                        key={drive.path} 
                                        className={cx("ts-drive-row", { active: selectedDrivePath === drive.path })}
                                        onClick={() => setSelectedDrivePath(drive.path)}
                                    >
                                        <div className="col-name">
                                            <HardDrive className="icon-xs" />
                                            <span>{drive.label} ({drive.path.replace(/\\$/, '')})</span>
                                        </div>
                                        <div className="col-space">
                                            {formatSize(drive.free_bytes || 0, 1, t)}
                                        </div>
                                    </div>
                                ))}
                            </div>
                        </div>
                    </div>

                    <div className="ts-section">
                        <div className="ts-section-header">
                            <Settings2 className="icon-xs" />
                            <span>{t('settings_for_selected_location' as any)}</span>
                        </div>
                        
                        <div className="ts-settings-card">
                            <div className="ts-control-row">
                                <div className="ts-control-label">
                                    <span className="ts-label-title">{t('custom_size' as any)}</span>
                                    <span className="ts-label-desc">{t('max_capacity' as any)}</span>
                                </div>
                                <div className="ts-input-group">
                                    <input 
                                        type="number" 
                                        className="ts-number-input"
                                        value={getDisplayValue()}
                                        disabled={currentConfig?.nuke_on_delete}
                                        onChange={e => handleValueChange(parseInt(e.target.value) || 0)}
                                    />
                                    <div className="ts-unit-toggle">
                                        <button 
                                            className={cx({ active: currentUnit === 'mb' })}
                                            onClick={() => selectedDrivePath && setUnits(prev => ({ ...prev, [selectedDrivePath]: 'mb' }))}
                                        >Mo</button>
                                        <button 
                                            className={cx({ active: currentUnit === 'percent' })}
                                            onClick={() => selectedDrivePath && setUnits(prev => ({ ...prev, [selectedDrivePath]: 'percent' }))}
                                        >%</button>
                                    </div>
                                </div>
                            </div>

                            <div className="ts-divider" />

                            <div className="ts-control-row">
                                <div className="ts-control-label">
                                    <span className="ts-label-title">{t('dont_move_to_trash' as any)}</span>
                                    <span className="ts-label-desc">{t('delete_immediately_desc' as any)}</span>
                                </div>
                                <Toggle 
                                    checked={currentConfig?.nuke_on_delete || false}
                                    onChange={(val) => updateCurrentConfig({ nuke_on_delete: val })}
                                />
                            </div>
                        </div>
                    </div>

                    <div className="ts-section global-options">
                        <div className="ts-control-row">
                            <div className="ts-control-label">
                                <span className="ts-label-title">{t('display_delete_confirmation' as any)}</span>
                            </div>
                            <Toggle 
                                checked={confirmDeleteGlobal}
                                onChange={onUpdateGlobalConfirm}
                            />
                        </div>
                    </div>
                </div>

                <div className="prop-footer">
                    <div style={{ display: 'flex', gap: '0.75rem', justifyContent: 'flex-end', width: '100%' }}>
                        <button className="btn" onClick={onClose}>{t('cancel')}</button>
                        <button className="btn primary" onClick={handleSave} disabled={isSaving || isLoading}>
                            {t('ok' as any)}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
};
