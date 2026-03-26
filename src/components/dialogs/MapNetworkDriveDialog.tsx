import React, { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useDraggable } from '../../hooks/useDraggable';
import { useApp } from '../../context/AppContext';
import { X, Network, Check, ChevronDown } from 'lucide-react';
import { TFunc } from '../../i18n';
import cx from 'classnames';
import '../../styles/components/Dialogs.css';

interface MapNetworkDriveDialogProps {
    onClose: () => void;
    t: TFunc;
    zIndex?: number;
    onFocus?: () => void;
}

export const MapNetworkDriveDialog: React.FC<MapNetworkDriveDialogProps> = ({ onClose, t, zIndex, onFocus }) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const { notify, drives } = useApp();

    const [letter, setLetter] = useState('Z:');
    const [path, setPath] = useState('');
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [reconnect, setReconnect] = useState(true);
    const [loading, setLoading] = useState(false);
    const [isDropdownOpen, setIsDropdownOpen] = useState(false);
    const dropdownRef = useRef<HTMLDivElement>(null);

    const availableLetters = 'ZYXWVUTSRQPONMLKJIHGFEDCBA'.split('').map(l => `${l}:`);

    useEffect(() => {
        if (drives && drives.length > 0) {
            const firstAvailable = availableLetters.find(l =>
                !drives.some(d => d.path.toUpperCase().startsWith(l.toUpperCase()))
            );
            if (firstAvailable) setLetter(firstAvailable);
        }
    }, [drives]);

    useEffect(() => {
        const handleClickOutside = (event: MouseEvent) => {
            if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
                setIsDropdownOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, []);

    const handleConfirm = async () => {
        if (!path) {
            notify(t('error_empty_path' as any), 'error');
            return;
        }

        setLoading(true);
        try {
            await invoke('map_network_drive', {
                letter,
                path,
                reconnect,
                username: username.trim() || null,
                password: password || null
            });
            notify(t('map_network_drive_success' as any), 'success');
            onClose();
        } catch (e: any) {
            notify(e.toString(), 'error');
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="properties-overlay" style={{ zIndex }}>
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
                <div className="prop-header-bar" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <div className="prop-title" style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                        <Network size={14} />
                        {t('map_network_drive' as any)}
                    </div>
                    <button className="btn-icon" onClick={onClose} style={{ marginLeft: 'auto' }}>
                        <X size={14} />
                    </button>
                </div>

                <div className="prop-content overflow-visible" style={{ padding: '1.25rem' }}>
                    <div style={{ marginBottom: '1.5rem' }}>
                        <p style={{ margin: '0 0 0.5rem 0', fontWeight: 600, fontSize: '0.85rem' }}>
                            {t('map_network_drive_desc' as any)}
                        </p>
                        <p className="form-hint" style={{ margin: 0, opacity: 0.7, fontSize: '0.75rem' }}>
                            {t('map_network_drive_subdesc' as any)}
                        </p>
                    </div>

                    <div className="prop-grid">
                        <div className="prop-label">{t('drive' as any)}</div>
                        <div className="prop-value">
                            <div className="custom-unit-selector" ref={dropdownRef}>
                                <div
                                    className="unit-selected-value"
                                    onClick={() => setIsDropdownOpen(!isDropdownOpen)}
                                >
                                    {letter}
                                    <ChevronDown size={14} className={cx("arrow", { open: isDropdownOpen })} />
                                </div>
                                {isDropdownOpen && (
                                    <div className="unit-dropdown-list dropdown-scrollable">
                                        {availableLetters.map(l => {
                                            const isUsed = drives?.some(d => d.path.toUpperCase().startsWith(l));
                                            return (
                                                <div
                                                    key={l}
                                                    className={cx("unit-option", {
                                                        active: letter === l,
                                                        disabled: isUsed
                                                    })}
                                                    style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '0.5rem' }}
                                                    onClick={() => {
                                                        if (!isUsed) {
                                                            setLetter(l);
                                                            setIsDropdownOpen(false);
                                                        }
                                                    }}
                                                >
                                                    <span style={{ fontWeight: 600 }}>{l}</span>
                                                    {isUsed && (
                                                        <span style={{ fontSize: '0.7rem', opacity: 0.7, fontStyle: 'italic' }}>
                                                            ({t('drive_in_use' as any)})
                                                        </span>
                                                    )}
                                                </div>
                                            );
                                        })}
                                    </div>
                                )}
                            </div>
                        </div>

                        <div className="prop-label">{t('folder' as any)}</div>
                        <div className="prop-value">
                            <input
                                type="text"
                                className="input-field"
                                placeholder="\\server\share"
                                value={path}
                                onChange={e => setPath(e.target.value)}
                                onInput={e => setPath((e.target as HTMLInputElement).value)}
                                onPaste={e => {
                                    // Small delay to let the paste finish before reading value if needed, 
                                    // though React onChange/onInput usually handle it.
                                    const pastedData = e.clipboardData.getData('text');
                                    if (pastedData) setPath(pastedData);
                                }}
                                autoFocus
                            />
                        </div>

                        <div className="prop-divider-row" />

                        <div className="prop-label">{t('username' as any)}</div>
                        <div className="prop-value">
                            <input
                                type="text"
                                className="input-field"
                                placeholder="SERVEUR\Utilisateur"
                                value={username}
                                onChange={e => setUsername(e.target.value)}
                            />
                        </div>

                        <div className="prop-label">{t('password' as any)}</div>
                        <div className="prop-value">
                            <input
                                type="password"
                                className="input-field"
                                value={password}
                                onChange={e => setPassword(e.target.value)}
                            />
                        </div>
                    </div>

                    <div style={{ marginTop: '1.25rem' }}>
                        <label className="prop-checkbox">
                            <input
                                type="checkbox"
                                checked={reconnect}
                                onChange={e => setReconnect(e.target.checked)}
                            />
                            <div className="checkbox-visual">
                                {reconnect && <Check size={10} strokeWidth={3} />}
                            </div>
                            <span style={{ fontSize: '0.8125rem' }}>{t('reconnect_at_signin' as any)}</span>
                        </label>
                    </div>
                </div>

                <div className="prop-footer spaced">
                    <div />
                    <div style={{ display: 'flex', gap: '0.5rem' }}>
                        <button className="btn" onClick={onClose} disabled={loading}>
                            {t('cancel')}
                        </button>
                        <button className="btn primary" onClick={handleConfirm} disabled={loading || !path.trim()}>
                            {t('finish' as any)}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
};
