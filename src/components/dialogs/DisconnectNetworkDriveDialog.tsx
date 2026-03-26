import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useDraggable } from '../../hooks/useDraggable';
import { X, ServerOff, Check, Loader2, HardDrive } from 'lucide-react';
import { useApp } from '../../context/AppContext';
import { TFunc } from '../../i18n';
import cx from 'classnames';
import '../../styles/components/Dialogs.css';

interface DisconnectNetworkDriveDialogProps {
    onClose: () => void;
    t: TFunc;
    zIndex?: number;
    onFocus?: () => void;
}

export const DisconnectNetworkDriveDialog: React.FC<DisconnectNetworkDriveDialogProps> = ({ onClose, t, zIndex, onFocus }) => {
    const dragRef = React.useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const { notify } = useApp();

    // Assume `get_drives` fetches all drives, including mapped ones (`isDrive` with `remote` driveType)
    // Actually `get_network_resources` might not fetch mapped drives, but `get_drives` does.
    const [drives, setDrives] = useState<any[]>([]);
    const [loading, setLoading] = useState(true);
    const [selectedLetters, setSelectedLetters] = useState<string[]>([]);
    const [disconnectLoading, setDisconnectLoading] = useState(false);

    useEffect(() => {
        const loadDrives = async () => {
            try {
                // system::get_drives
                const appDrives = await invoke<any[]>('get_drives');
                // Filter only network/remote ones. Assuming 'remote' or 'network'
                const remoteDrives = appDrives.filter(d =>
                    d.drive_type === 'remote' || d.drive_type === 'network' || d.path.startsWith('\\\\')
                );
                setDrives(remoteDrives);
            } catch (e: any) {
                notify(e.toString(), 'error');
            } finally {
                setLoading(false);
            }
        };

        loadDrives();
    }, [notify]);

    const handleConfirm = async () => {
        if (selectedLetters.length === 0) {
            onClose();
            return;
        }

        setDisconnectLoading(true);
        try {
            for (const path of selectedLetters) {
                // path is usually "Z:\" or "\\server\share". The WNet API uses letter like "Z:" or full path.
                const cleanLetter = path.replace(/[\\/]+$/, '');
                await invoke('disconnect_network_drive', { letter: cleanLetter, force: true });
            }
            notify(t('disconnect_network_drive_success' as any), 'success');
            onClose();
        } catch (e: any) {
            notify(e.toString(), 'error');
        } finally {
            setDisconnectLoading(false);
        }
    };

    const toggleSelection = (path: string) => {
        if (selectedLetters.includes(path)) {
            setSelectedLetters(selectedLetters.filter(p => p !== path));
        } else {
            setSelectedLetters([...selectedLetters, path]);
        }
    };

    return (
        <div className="dialog-overlay" style={{ zIndex }}>
            <div
                ref={dragRef}
                className="dialog-window"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none',
                    width: '480px',
                    height: '460px'
                }}
            >
                <div className="dialog-header" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <div className="dialog-title" style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                        <ServerOff size={14} />
                        {t('disconnect_network_drive' as any)}
                    </div>
                    <button className="dialog-close-btn btn-icon" onClick={onClose} style={{ marginLeft: 'auto' }}>
                        <X size={14} />
                    </button>
                </div>

                <div className="dialog-content" style={{ padding: '1.25rem', display: 'flex', flexDirection: 'column', gap: '1rem', flex: 1, minHeight: 0 }}>
                    <p style={{ margin: 0, fontSize: '0.8125rem', opacity: 0.8 }}>
                        {t('disconnect_network_drive_desc' as any)}
                    </p>

                    <div className="network-drives-container" style={{ flex: 1, minHeight: 0, border: '1px solid var(--border-color)', borderRadius: '0.5rem', background: 'var(--surface-primary)', display: 'flex', flexDirection: 'column' }}>
                        <div className="dialog-pill-list" style={{ padding: '0.75rem', overflowY: 'auto', flex: 1 }}>
                            {loading ? (
                                <div style={{ padding: '3rem', textAlign: 'center', opacity: 0.5 }}><Loader2 className="animate-spin" /></div>
                            ) : drives.length === 0 ? (
                                <div style={{ padding: '3rem', textAlign: 'center', color: 'var(--text-muted)', fontSize: '0.85rem' }}>
                                    {t('no_network_drives_found' as any)}
                                </div>
                            ) : (
                                drives.map(d => {
                                    const isSelected = selectedLetters.includes(d.path);
                                    return (
                                        <div
                                            key={d.path}
                                            className={cx("dialog-pill", { selected: isSelected })}
                                            onClick={() => toggleSelection(d.path)}
                                        >
                                            <div className="dialog-checkbox" style={{ pointerEvents: 'none' }}>
                                                <input type="checkbox" checked={isSelected} readOnly />
                                                <div className="checkbox-visual">
                                                    {isSelected && <Check size={10} strokeWidth={3} />}
                                                </div>
                                            </div>

                                            <div className="dialog-pill-icon" style={{ color: isSelected ? 'var(--accent-color)' : 'var(--text-muted)' }}>
                                                <HardDrive size={18} strokeWidth={1.5} />
                                            </div>

                                            <div className="dialog-pill-info">
                                                <div className="dialog-pill-title">
                                                    {d.path}
                                                </div>
                                                <div className="dialog-pill-subtitle" style={{ opacity: isSelected ? 0.8 : 0.6 }}>
                                                    {d.remote_path || d.label}
                                                </div>
                                            </div>
                                        </div>
                                    );
                                })
                            )}
                        </div>
                    </div>
                </div>

                <div className="dialog-footer">
                    <div style={{ display: 'flex', gap: '0.5rem' }}>
                        <button className="btn" onClick={onClose} disabled={disconnectLoading}>
                            {t('cancel')}
                        </button>
                        <button className="btn primary" onClick={handleConfirm} disabled={disconnectLoading || selectedLetters.length === 0}>
                            {t('disconnect' as any)}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
};
