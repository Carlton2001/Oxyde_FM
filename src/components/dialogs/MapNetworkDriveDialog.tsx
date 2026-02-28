import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useDraggable } from '../../hooks/useDraggable';
import { useApp } from '../../context/AppContext';
import { X, Network } from 'lucide-react';
import { TFunc } from '../../i18n';

interface MapNetworkDriveDialogProps {
    onClose: () => void;
    t: TFunc;
}

export const MapNetworkDriveDialog: React.FC<MapNetworkDriveDialogProps> = ({ onClose, t }) => {
    const dragRef = React.useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const { notify } = useApp();

    // Default to Z:
    const [letter, setLetter] = useState('Z:');
    const [path, setPath] = useState('');
    const [reconnect, setReconnect] = useState(true);
    const [loading, setLoading] = useState(false);

    const availableLetters = 'ZYXWVUTSRQPONMLKJIHGFEDCBA'.split('').map(l => `${l}:`);

    const handleConfirm = async () => {
        if (!path) {
            notify(t('error_empty_path' as any), 'error');
            return;
        }

        setLoading(true);
        try {
            await invoke('map_network_drive', { letter, path, reconnect });
            notify(t('map_network_drive_success' as any), 'success');
            onClose();
        } catch (e: any) {
            notify(e.toString(), 'error');
        } finally {
            setLoading(false);
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
                    width: '460px'
                }}
            >
                <div className="prop-header-bar" onMouseDown={handleMouseDown}>
                    <div className="prop-title" style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                        <Network size={14} />
                        {t('map_network_drive' as any)}
                    </div>
                    <button className="btn-icon" onClick={onClose} style={{ marginLeft: 'auto' }}>
                        <X size={14} />
                    </button>
                </div>

                <div className="prop-content" style={{ padding: '1.25rem' }}>
                    <div style={{ marginBottom: '1.25rem' }}>
                        <p style={{ margin: '0 0 0.5rem 0', fontWeight: 500 }}>
                            {t('map_network_drive_desc' as any)}
                        </p>
                        <p className="form-hint" style={{ margin: 0 }}>
                            {t('map_network_drive_subdesc' as any)}
                        </p>
                    </div>

                    <div className="form-group" style={{ display: 'grid', gridTemplateColumns: '80px 1fr', gap: '0.75rem', alignItems: 'center' }}>
                        <label>{t('drive' as any)}</label>
                        <select
                            className="select-field"
                            style={{ width: '100px' }}
                            value={letter}
                            onChange={e => setLetter(e.target.value)}
                        >
                            {availableLetters.map(l => (
                                <option key={l} value={l}>{l}</option>
                            ))}
                        </select>

                        <label>{t('folder' as any)}</label>
                        <input
                            type="text"
                            className="input-field"
                            placeholder="\\server\share"
                            value={path}
                            onChange={e => setPath(e.target.value)}
                            autoFocus
                        />
                    </div>

                    <div style={{ marginTop: '1.25rem' }}>
                        <label className="checkbox-label" style={{ display: 'flex', alignItems: 'center', gap: '0.625rem', cursor: 'pointer', fontSize: '0.8125rem' }}>
                            <input
                                type="checkbox"
                                checked={reconnect}
                                onChange={e => setReconnect(e.target.checked)}
                                style={{ width: '14px', height: '14px' }}
                            />
                            {t('reconnect_at_signin' as any)}
                        </label>
                    </div>
                </div>

                <div className="prop-footer" style={{ padding: '1rem 1.25rem', display: 'flex', justifyContent: 'flex-end', gap: '0.75rem', background: 'var(--surface-secondary)' }}>
                    <button className="btn" onClick={onClose} disabled={loading}>
                        {t('cancel')}
                    </button>
                    <button className="btn primary" onClick={handleConfirm} disabled={loading || !path}>
                        {t('finish' as any)}
                    </button>
                </div>
            </div>
        </div>
    );
};
