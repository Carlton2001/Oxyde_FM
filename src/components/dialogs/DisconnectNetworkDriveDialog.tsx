import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useDraggable } from '../../hooks/useDraggable';
import { X, ServerOff } from 'lucide-react';
import { useApp } from '../../context/AppContext';
import { TFunc } from '../../i18n';

interface DisconnectNetworkDriveDialogProps {
    onClose: () => void;
    t: TFunc;
}

export const DisconnectNetworkDriveDialog: React.FC<DisconnectNetworkDriveDialogProps> = ({ onClose, t }) => {
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
        <div className="properties-overlay" onClick={onClose}>
            <div
                ref={dragRef}
                className="properties-dialog"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none',
                    width: '450px',
                    maxHeight: '400px'
                }}
            >
                <div className="prop-header-bar" onMouseDown={handleMouseDown}>
                    <div className="prop-title" style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                        <ServerOff size={14} />
                        {t('disconnect_network_drive' as any)}
                    </div>
                    <button className="btn-icon" onClick={onClose} style={{ marginLeft: 'auto' }}>
                        <X size={14} />
                    </button>
                </div>

                <div className="prop-content" style={{ padding: '1.25rem', display: 'flex', flexDirection: 'column', gap: '1rem' }}>
                    <p style={{ margin: 0, fontSize: '0.8125rem' }}>
                        {t('disconnect_network_drive_desc' as any)}
                    </p>

                    <div style={{
                        height: '200px',
                        overflowY: 'auto',
                        border: '1px solid var(--border-color)',
                        borderRadius: '0.375rem',
                        background: 'var(--surface-primary)',
                        display: 'flex',
                        flexDirection: 'column'
                    }}>
                        {loading && <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-muted)' }}>{t('loading' as any)}</div>}
                        {!loading && drives.length === 0 && (
                            <div style={{ padding: '2rem', textAlign: 'center', color: 'var(--text-muted)', fontSize: '0.8125rem' }}>
                                {t('no_network_drives' as any)}
                            </div>
                        )}
                        {!loading && drives.length > 0 && drives.map(d => (
                            <label
                                key={d.path}
                                style={{
                                    display: 'flex',
                                    alignItems: 'center',
                                    padding: '0.625rem 1rem',
                                    gap: '0.75rem',
                                    cursor: 'pointer',
                                    borderBottom: '1px solid var(--border-color)',
                                    transition: 'background 0.1s ease',
                                    background: selectedLetters.includes(d.path) ? 'var(--hover-color)' : 'transparent'
                                }}
                                className="drive-item-label"
                            >
                                <input
                                    type="checkbox"
                                    checked={selectedLetters.includes(d.path)}
                                    onChange={() => toggleSelection(d.path)}
                                    style={{ width: '14px', height: '14px' }}
                                />
                                <div style={{ display: 'flex', flexDirection: 'column', gap: '1px', overflow: 'hidden' }}>
                                    <span style={{ fontFamily: 'var(--font-mono)', fontSize: '0.8125rem', fontWeight: 600 }}>{d.path}</span>
                                    {d.label && <span style={{ fontSize: '0.6875rem', color: 'var(--text-muted)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{d.label}</span>}
                                </div>
                            </label>
                        ))}
                    </div>
                </div>

                <div className="prop-footer" style={{ padding: '1rem 1.25rem', display: 'flex', justifyContent: 'flex-end', gap: '0.75rem', background: 'var(--surface-secondary)' }}>
                    <button className="btn" onClick={onClose} disabled={disconnectLoading}>
                        {t('cancel')}
                    </button>
                    <button className="btn primary" onClick={handleConfirm} disabled={disconnectLoading || selectedLetters.length === 0}>
                        OK
                    </button>
                </div>
            </div>
            <style>{`
                .drive-item-label:hover {
                    background: var(--hover-color) !important;
                }
                .drive-item-label:last-child {
                    border-bottom: none !important;
                }
            `}</style>
        </div>
    );
};
