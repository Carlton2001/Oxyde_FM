import React, { useRef } from 'react';
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

const InlineAboutDialog: React.FC<{ onClose: () => void, t: any, theme: string, appVersion: string }> = ({ onClose, t, theme, appVersion }) => {
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

                const confirmed = await confirm(
                    t('update_confirm_msg'),
                    t('update_confirm_title'),
                    false, // isDanger
                    t('install'), // confirmLabel
                );

                if (confirmed) {
                    await update.downloadAndInstall();
                    await relaunch();
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
        <div className="properties-overlay" onClick={onClose}>
            <div
                ref={dragRef}
                className="about-dialog"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="about-header" onMouseDown={handleMouseDown}>
                    <span className="about-title">{t('about')}</span>
                    <button className="btn-icon" onClick={onClose}><X size={16} /></button>
                </div>
                <div className="about-content">
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
                                {updateStatus === 'available' ? t('update_available') :
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
                                { name: 'React', logo: theme.includes('light') ? reactLight : reactDark, url: 'https://react.dev' },
                                { name: 'Rust', logo: rustLogo, isRust: true, url: 'https://www.rust-lang.org' },
                                { name: 'Tauri', logo: theme.includes('light') ? tauriLight : tauriDark, url: 'https://tauri.app' },
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
                <div className="about-footer">
                    <button className="btn primary" onClick={onClose}>OK</button>
                </div>
            </div>
        </div>
    );
};

export const GlobalDialogContainer: React.FC = () => {
    const { dialogs, closeDialog } = useDialogs();
    const { t, theme } = useApp();

    if (dialogs.length === 0) return null;

    return (
        <>
            {dialogs.map(dialog => {
                const { id, type, props } = dialog;
                const handleClose = (result?: any) => closeDialog(id, result);

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
                                icon="new_folder" // Default icon
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
                            />
                        );

                    case 'duplicates':
                        return (
                            <DuplicateSearchDialog
                                key={id}
                                initialRoot={props.initialRoot}
                                t={t}
                                onClose={() => handleClose(null)}
                            />
                        );

                    case 'mapNetworkDrive':
                        return (
                            <MapNetworkDriveDialog
                                key={id}
                                t={t}
                                onClose={() => handleClose()}
                            />
                        );

                    case 'disconnectNetworkDrive':
                        return (
                            <DisconnectNetworkDriveDialog
                                key={id}
                                t={t}
                                onClose={() => handleClose()}
                            />
                        );

                    default:
                        return null;
                }
            })}
        </>
    );
};

