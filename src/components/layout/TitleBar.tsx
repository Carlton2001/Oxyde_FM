import React, { useState, useEffect } from 'react';
import { Minus, Square, X, Copy, Wrench, Search, ChartBarBig, RefreshCw, Plus } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import cx from 'classnames';
import { SettingsMenu } from './SettingsMenu';
import { TFunc } from '../../i18n';
import { useApp } from '../../context/AppContext';
import './TitleBar.css';

interface TitleBarProps {
    t: TFunc;
    onAdvancedSearch: () => void;
    onDuplicateSearch: () => void;
    onShowAbout: () => void;
    onRefresh: () => void;
    onCalculateAllSizes: () => void;
}

export const TitleBar: React.FC<TitleBarProps> = ({
    t,
    onAdvancedSearch,
    onDuplicateSearch,
    onShowAbout,
    onRefresh,
    onCalculateAllSizes
}) => {
    const { firstLaunch, setFirstLaunch } = useApp();
    const [isMaximized, setIsMaximized] = useState(false);
    const [settingsOpen, setSettingsOpen] = useState(false);
    const [settingsPage, setSettingsPage] = useState<'main' | 'themes' | 'languages' | 'dates' | 'compression'>('main');
    const [hamburgerOpen, setHamburgerOpen] = useState(false);
    const lastActionRef = React.useRef<number>(0);

    useEffect(() => {
        const updateMaximized = async () => {
            try {
                const max = await getCurrentWindow().isMaximized();
                setIsMaximized(max);
            } catch (err) {
                // Ignore transient errors
            }
        };
        window.addEventListener('resize', updateMaximized);
        updateMaximized();
        return () => window.removeEventListener('resize', updateMaximized);
    }, []);

    // Close settings/hamburger when clicking outside
    useEffect(() => {
        if (!settingsOpen && !hamburgerOpen) return;
        const handleClickOutside = (e: MouseEvent) => {
            const target = e.target as HTMLElement;
            if (settingsOpen && !target.closest('.settings-container')) {
                setSettingsOpen(false);
                setSettingsPage('main');
            }
            if (hamburgerOpen && !target.closest('.hamburger-container')) {
                setHamburgerOpen(false);
            }
        };
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [settingsOpen, hamburgerOpen]);

    const toggleSettings = () => {
        if (!settingsOpen && firstLaunch) {
            setFirstLaunch(false);
        }
        setSettingsOpen(!settingsOpen);
        setSettingsPage('main');
        if (hamburgerOpen) setHamburgerOpen(false);
    };

    const toggleHamburger = () => {
        setHamburgerOpen(!hamburgerOpen);
        if (settingsOpen) setSettingsOpen(false);
    };

    return (
        <div className="title-bar" data-tauri-drag-region>
            <div className="title-bar-left">
                <div className="settings-container branding" onClick={(e) => e.stopPropagation()}>
                    <button
                        className={cx("btn-icon logo-btn", { "glow-pulse": firstLaunch && !settingsOpen })}
                        onClick={toggleSettings}
                        data-tooltip={t('settings' as any)}
                        data-tooltip-pos="bottom"
                    >
                        <img src="/logo.svg" className="app-logo-icon" alt="Oxyde" />
                    </button>
                    <SettingsMenu
                        isOpen={settingsOpen}
                        onClose={() => setSettingsOpen(false)}
                        page={settingsPage}
                        onPageChange={setSettingsPage}
                        onShowAbout={onShowAbout}
                    />
                </div>
                <span className="app-title">Oxyde File Manager</span>
            </div>

            <div className="title-bar-center" data-tauri-drag-region>
                {/* Search or Path could go here in the future if needed */}
            </div>

            <div className="title-bar-right">
                <div className="app-tools-group">
                    <button
                        className="btn-icon"
                        onClick={(e) => {
                            e.stopPropagation();
                            const now = Date.now();
                            if (now - lastActionRef.current < 500) return;
                            lastActionRef.current = now;
                            invoke('add_panel', { path: 'C:\\' }).catch(console.error);
                        }}
                        data-tooltip={t('new_pane' as any) || 'New Pane'}
                        data-tooltip-pos="bottom"
                    >
                        <Plus size={16} />
                    </button>

                    <div className="hamburger-container">
                        <button className={cx("btn-icon", { active: hamburgerOpen })} onClick={toggleHamburger} data-tooltip={t('tools' as any) || 'Tools'} data-tooltip-pos="bottom">
                            <Wrench size={16} />
                        </button>
                        {hamburgerOpen && (
                            <div className="hamburger-menu" onClick={(e) => e.stopPropagation()}>
                                <div className="hamburger-item" onClick={() => { onRefresh(); setHamburgerOpen(false); }}>
                                    <div className="hamburger-item-content">
                                        <RefreshCw size={14} />
                                        {t('refresh' as any) || 'Refresh'}
                                    </div>
                                </div>
                                <div className="hamburger-item" onClick={() => { onCalculateAllSizes(); setHamburgerOpen(false); }}>
                                    <div className="hamburger-item-content">
                                        <ChartBarBig size={14} />
                                        {t('calculate_size' as any) || 'Calculate Sizes'}
                                    </div>
                                </div>
                                <div className="hamburger-separator" />
                                <div className="hamburger-item" onClick={() => { onAdvancedSearch(); setHamburgerOpen(false); }}>
                                    <div className="hamburger-item-content">
                                        <Search size={14} />
                                        {t('advanced_search' as any) || 'Advanced Search'}
                                    </div>
                                </div>
                                <div className="hamburger-item" onClick={() => { onDuplicateSearch(); setHamburgerOpen(false); }}>
                                    <div className="hamburger-item-content">
                                        <Copy size={14} />
                                        {t('duplicates' as any) || 'Duplicate Search'}
                                    </div>
                                </div>
                            </div>
                        )}
                    </div>
                </div>

                <div className="window-controls">
                    <div className="btn-icon control-btn" onClick={() => getCurrentWindow().minimize()} data-tooltip={t('minimize' as any)} data-tooltip-pos="bottom">
                        <Minus size={14} />
                    </div>
                    <div className="btn-icon control-btn" onClick={async () => {
                        await getCurrentWindow().toggleMaximize();
                        setIsMaximized(await getCurrentWindow().isMaximized());
                    }} data-tooltip={isMaximized ? t('restore' as any) || 'Restore' : t('maximize' as any)} data-tooltip-pos="bottom">
                        {isMaximized ? <Copy size={12} style={{ transform: 'rotate(180deg) scaleY(-1)' }} /> : <Square size={12} />}
                    </div>
                    <div className="btn-icon control-btn danger" onClick={() => getCurrentWindow().close()} data-tooltip={t('close' as any)} data-tooltip-pos="bottom">
                        <X size={14} />
                    </div>
                </div>
            </div>
        </div>
    );
};

