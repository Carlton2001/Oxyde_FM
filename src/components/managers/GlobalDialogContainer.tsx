import React, { useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../../styles/components/AboutDialog.css';
import { useDialogs } from '../../context/DialogContext';
import { useApp } from '../../context/AppContext';
import { useDraggable } from '../../hooks/useDraggable';
import { PropertiesDialog } from '../dialogs/PropertiesDialog';
import { ConflictDialog } from '../dialogs/ConflictDialog';
import { InputDialog } from '../dialogs/InputDialog';
import { ConfirmDialog } from '../dialogs/ConfirmDialog';
import { SearchDialog } from '../dialogs/SearchDialog';
import { DuplicateSearchDialog } from '../dialogs/DuplicateSearchDialog';
import { MapNetworkDriveDialog } from '../dialogs/MapNetworkDriveDialog';
import { DisconnectNetworkDriveDialog } from '../dialogs/DisconnectNetworkDriveDialog';
import { TrashSettingsDialog } from '../dialogs/TrashSettingsDialog';
import { X } from 'lucide-react';
import { openUrl } from '@tauri-apps/plugin-opener';

// Assets for About Dialog (reusing imports from DialogManager essentially)
import reactLight from '../../assets/react-light.svg';
import reactDark from '../../assets/react-dark.svg';
import rustLogo from '../../assets/rust.svg';
import tauriLight from '../../assets/tauri-light.svg';
import tauriDark from '../../assets/tauri-dark.svg';
import lucideLight from '../../assets/lucide-light.svg';
import lucideDark from '../../assets/lucide-dark.svg';

// Inline AboutDialog component until extraction (Step 2.3 cleanup usually handles extraction, but good to have it clean here)
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { Github, RefreshCcw, ExternalLink } from 'lucide-react';

const InlineAboutDialog: React.FC<{ onClose: () => void, t: any, theme: string, appVersion: string, zIndex?: number, onFocus?: () => void }> = ({ onClose, t, theme, appVersion, zIndex, onFocus }) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const { updateAvailable, setUpdateAvailable } = useApp();
    const [updating, setUpdating] = React.useState(false);
    const [updateStatus, setUpdateStatus] = React.useState<'idle' | 'available' | 'none' | 'error'>(updateAvailable ? 'available' : 'idle');

    const { confirm } = useDialogs();

    const handleCheckUpdate = async () => {
        setUpdating(true);
        try {
            const update = await check();
            if (update?.available) {
                setUpdateAvailable(true);
                setUpdateStatus('available');

                const isPortable = await invoke<boolean>('is_portable');

                if (isPortable) {
                    const confirmed = await confirm(
                        t('update_portable_msg'),
                        t('update_confirm_title'),
                        false,
                        t('download_zip'),
                    );

                    if (confirmed) {
                        openUrl('https://github.com/Carlton2001/Oxyde_FM/releases');
                    }
                } else {
                    const confirmed = await confirm(
                        t('update_confirm_msg'),
                        t('update_confirm_title'),
                        false,
                        t('install'),
                    );

                    if (confirmed) {
                        await update.downloadAndInstall();
                        await relaunch();
                    }
                }
            } else {
                setUpdateAvailable(false);
                setUpdateStatus('none');
            }
        } catch (e) {
            console.error(e);
            setUpdateStatus('error');
        } finally {
            setUpdating(false);
        }
    };

    return (
        <div className="dialog-overlay properties-overlay" onClick={onClose} onMouseDown={onFocus} style={{ zIndex }}>
            <div
                ref={dragRef}
                className="dialog-window about-dialog"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="dialog-header about-header" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <span className="dialog-title about-title">{t('about')}</span>
                    <button className="dialog-close-btn btn-icon" onClick={onClose}><X size={16} /></button>
                </div>
                <div className="dialog-content about-content">
                    <div className="about-logo">
                        <img src="/logo.svg" alt="Oxyde Logo" className="about-logo-img" />
                    </div>
                    <div className="about-text-group">
                        <div className="about-app-name">Oxyde</div>
                        <div className="about-tagline">Vibe coded with love</div>
                        <div className="about-version">
                            Version {appVersion}
                        </div>
                    </div>

                    <div className="about-actions" style={{ display: 'flex', gap: '0.5rem', marginTop: '0.5rem', flexWrap: 'wrap' }}>
                        <button
                            className={`btn secondary small ${updating ? 'loading' : ''}`}
                            onClick={handleCheckUpdate}
                            disabled={updating}
                            style={{ flex: '1 1 auto', minWidth: 'fit-content', justifyContent: 'center' }}
                        >
                            <RefreshCcw size={14} className={updating ? 'spin' : ''} />
                            <span style={{ whiteSpace: 'nowrap' }}>
                                {updating ? t('checking_updates') :
                                    updateStatus === 'available' ? t('update_available') :
                                        updateStatus === 'none' ? t('up_to_date') :
                                            t('check_updates')}
                            </span>
                        </button>

                        <button
                            className="btn secondary small"
                            style={{ flex: '1 1 auto', minWidth: 'fit-content', justifyContent: 'center' }}
                            onClick={() => openUrl('https://github.com/Carlton2001/Oxyde_FM')}
                        >
                            <Github size={14} />
                            <span style={{ whiteSpace: 'nowrap' }}>{t('github_repo')}</span>
                            <ExternalLink size={10} style={{ marginLeft: '4px', opacity: 0.6 }} />
                        </button>
                    </div>

                    <div className="about-credits">
                        <div className="credits-label">{t('main_tools' as any)}</div>
                        <div className="credits-grid">
                            {[
                                { name: 'Rust', logo: rustLogo, isRust: true, url: 'https://www.rust-lang.org' },
                                { name: 'Tauri', logo: theme.includes('light') ? tauriLight : tauriDark, url: 'https://tauri.app' },
                                { name: 'React', logo: theme.includes('light') ? reactLight : reactDark, url: 'https://react.dev' },
                                { name: 'Lucide', logo: theme.includes('light') ? lucideLight : lucideDark, url: 'https://lucide.dev' },
                            ].map((tool: any) => (
                                <div key={tool.name} className="credit-card" onClick={() => openUrl(tool.url)}>
                                    {tool.isRust ? (
                                        <div className="rust-logo-group">
                                            <img
                                                src={tool.logo}
                                                alt={tool.name}
                                                className={`credit-card-logo ${!theme.includes('light') ? 'invert-icon' : ''}`}
                                            />
                                            <span className="rust-brand-text">Rust</span>
                                        </div>
                                    ) : tool.name === 'Lucide' ? (
                                        <div className="lucide-logo-group">
                                            <img src={tool.logo} alt={tool.name} className="credit-card-logo" />
                                            <span className="lucide-brand-text">Lucide</span>
                                        </div>
                                    ) : (
                                        <img src={tool.logo} alt={tool.name} className="credit-card-logo wide" />
                                    )}
                                </div>
                            ))}
                        </div>
                    </div>
                </div>
                <div className="dialog-footer about-footer">
                    <button className="btn primary" onClick={onClose}>OK</button>
                </div>
            </div>
        </div>
    );
};

export const GlobalDialogContainer: React.FC = () => {
    const { dialogs, closeDialog, focusDialog } = useDialogs();
    const { t, theme, drives, confirmDelete, setConfirmDelete, isTrashEmpty, refreshDriveTrashConfigs } = useApp();

    React.useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key === 'Escape' && dialogs.length > 0) {
                // Find dialog with highest zIndex
                const topDialog = [...dialogs].sort((a, b) => b.zIndex - a.zIndex)[0];
                if (topDialog) {
                    // Close with appropriate default result for the type
                    let result: any = undefined;
                    if (topDialog.type === 'confirm') result = false;
                    if (topDialog.type === 'prompt') result = null;
                    if (topDialog.type === 'search') result = null;
                    if (topDialog.type === 'conflict') result = null;

                    closeDialog(topDialog.id, result);
                }
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [dialogs, closeDialog]);

    if (dialogs.length === 0) return null;

    return (
        <>
            {dialogs.map(dialog => {
                const { id, type, props, zIndex } = dialog;
                const handleClose = (result?: any) => closeDialog(id, result);
                const handleFocus = () => focusDialog(id);

                switch (type) {
                    case 'alert':
                        return (
                            <ConfirmDialog
                                key={id}
                                isOpen={true}
                                title={props.title || t('info' as any)}
                                message={props.message}
                                onConfirm={() => handleClose()}
                                onClose={() => handleClose()}
                                confirmLabel="OK"
                                t={t}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'confirm':
                        return (
                            <ConfirmDialog
                                key={id}
                                isOpen={true}
                                title={props.title || t('confirmation' as any)}
                                message={props.message}
                                subMessage={props.subMessage}
                                onConfirm={() => handleClose(true)}
                                onClose={() => handleClose(false)}
                                confirmLabel={t('yes' as any)}
                                isDanger={props.isDanger}
                                sources={props.sources}
                                destination={props.destination}
                                t={t}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'prompt':
                        // We reuse InputDialog for 'prompt'
                        return (
                            <InputDialog
                                key={id}
                                isOpen={true}
                                title={props.title || t('input' as any)}
                                label={props.message}
                                initialValue={props.defaultValue || ''}
                                onSubmit={(val) => handleClose(val)}
                                onClose={() => handleClose(null)}
                                confirmLabel="OK"
                                t={t}
                                icon={props.icon || 'new_folder'}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'properties':
                        return (
                            <PropertiesDialog
                                key={id}
                                onClose={() => handleClose()}
                                paths={props.paths}
                                t={t}
                                notify={() => { }} // Notifications handled by App context listeners
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'conflict':
                        return (
                            <ConflictDialog
                                key={id}
                                conflicts={props.conflicts}
                                onResolve={(resolutions) => handleClose(resolutions)}
                                onCancel={() => handleClose(null)} // Cancel returns null or similar
                                t={t}
                                operation={props.operation}
                                totalCount={props.totalCount}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'about':
                        return (
                            <InlineAboutDialog
                                key={id}
                                onClose={() => handleClose()}
                                t={t}
                                theme={theme}
                                appVersion={import.meta.env.PACKAGE_VERSION}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'search':
                        return (
                            <SearchDialog
                                key={id}
                                initialRoot={props.initialRoot}
                                initialOptions={props.initialOptions}
                                t={t}
                                onSearch={(options) => handleClose(options)}
                                onClose={() => handleClose(null)}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'duplicates':
                        return (
                            <DuplicateSearchDialog
                                key={id}
                                initialRoot={props.initialRoot}
                                t={t}
                                onClose={() => handleClose(null)}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'mapNetworkDrive':
                        return (
                            <MapNetworkDriveDialog
                                key={id}
                                t={t}
                                onClose={() => handleClose()}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    case 'disconnectNetworkDrive':
                        return (
                            <DisconnectNetworkDriveDialog
                                key={id}
                                t={t}
                                onClose={() => handleClose()}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );
                        
                    case 'trashSettings':
                        return (
                            <TrashSettingsDialog
                                key={id}
                                isOpen={true}
                                onClose={() => handleClose()}
                                t={t as any}
                                drives={drives}
                                confirmDeleteGlobal={confirmDelete}
                                onUpdateGlobalConfirm={setConfirmDelete}
                                isTrashEmpty={isTrashEmpty}
                                refreshConfigs={refreshDriveTrashConfigs}
                                zIndex={zIndex}
                                onFocus={handleFocus}
                            />
                        );

                    default:
                        return null;
                }
            })}
        </>
    );
};

