import React from 'react';
import cx from 'classnames';
import { ArrowRight, ArrowLeft, Sun, Moon, Eye, Globe, Info, Check, Plus, Minus, Grid2x2, Calendar, Zap, CheckSquare, Trash2, RefreshCw } from 'lucide-react';
import { Toggle } from '../ui/Toggle';
import { useApp } from '../../context/AppContext';
import { useDialogs } from '../../context/DialogContext';
import { Theme, Language, DateFormat, CompressionQuality } from '../../types';

interface SettingsMenuProps {
    isOpen: boolean;
    onClose: () => void;
    page: 'main' | 'themes' | 'languages' | 'dates' | 'compression';
    onPageChange: (page: 'main' | 'themes' | 'languages' | 'dates' | 'compression') => void;
    onShowAbout: () => void;
}

export const SettingsMenu: React.FC<SettingsMenuProps> = ({
    isOpen,
    onClose,
    page,
    onPageChange,
    onShowAbout,
}) => {
    const {
        theme, setTheme,
        language, setLanguage,
        showHidden, setShowHidden,
        showSystem, setShowSystem,
        fontSize, setFontSize,
        useSystemIcons, setUseSystemIcons,
        dateFormat, setDateFormat,
        showPreviews, setShowPreviews,
        zipQuality, setZipQuality,
        sevenZipQuality, setSevenZipQuality,
        zstdQuality, setZstdQuality,
        searchLimit, setSearchLimit,
        defaultTurboMode, setDefaultTurboMode,
        showGridThumbnails, setShowGridThumbnails,
        showCheckboxes, setShowCheckboxes,
        t, notify, resetToDefaults
    } = useApp();
    const { confirm } = useDialogs();

    if (!isOpen) return null;

    return (
        <div className="settings-menu" onClick={(e) => e.stopPropagation()}>
            {page === 'main' ? (
                <div className="settings-columns">
                    <div className="settings-column">
                        <div className="settings-label">{t('appearance' as any) || 'Appearance'}</div>
                        <div className="settings-item" onClick={(e) => { e.stopPropagation(); onPageChange('themes'); }}>
                            <div className="settings-item-content">
                                {theme.includes('light') ? <Sun size={14} /> : <Moon size={14} />}
                                {t('theme')}
                            </div>
                            <ArrowRight size={12} className="settings-item-icon-right" />
                        </div>
                        <div className="settings-item no-hover">
                            <div className="settings-item-content">
                                <div className="settings-font-icon-placeholder">A</div>
                                <span>{t('font_size' as any)}</span>
                            </div>
                            <div className="settings-font-controls">
                                <button
                                    className="font-btn"
                                    onClick={() => setFontSize(fontSize - 1)}
                                >
                                    <Minus size={12} />
                                </button>
                                <span
                                    className="settings-font-display"
                                    onClick={() => setFontSize(16)}
                                    title={t('reset_to_default' as any, { val: '16px' })}
                                >
                                    {fontSize}px
                                </span>
                                <button
                                    className="font-btn"
                                    onClick={() => setFontSize(fontSize + 1)}
                                >
                                    <Plus size={12} />
                                </button>
                            </div>
                        </div>
                        <div className="settings-item" onClick={() => setShowGridThumbnails(!showGridThumbnails)}>
                            <Grid2x2 size={14} />
                            <span className="settings-item-text">{t('show_grid_thumbnails' as any)}</span>
                            <Toggle checked={showGridThumbnails} onChange={setShowGridThumbnails} />
                        </div>
                        <div className="settings-item" onClick={() => setUseSystemIcons(!useSystemIcons)}>
                            <Grid2x2 size={14} />
                            <span className="settings-item-text">{t('use_system_icons')}</span>
                            <Toggle checked={useSystemIcons} onChange={setUseSystemIcons} />
                        </div>
                        <div className="settings-item" onClick={() => setShowCheckboxes(!showCheckboxes)}>
                            <CheckSquare size={14} />
                            <span className="settings-item-text">{t('show_checkboxes' as any)}</span>
                            <Toggle checked={showCheckboxes} onChange={setShowCheckboxes} />
                        </div>

                        <div className="settings-divider" />
                        <div className="settings-label">{t('files' as any) || 'Files'}</div>
                        <div className="settings-item" onClick={() => setShowHidden(!showHidden)}>
                            <Eye size={14} />
                            <span className="settings-item-text">{t('show_hidden')}</span>
                            <Toggle checked={showHidden} onChange={setShowHidden} />
                        </div>
                        <div className="settings-item" onClick={() => setShowSystem(!showSystem)}>
                            <Eye size={14} />
                            <span className="settings-item-text">{t('show_system' as any)}</span>
                            <Toggle checked={showSystem} onChange={setShowSystem} />
                        </div>
                        <div className="settings-item" onClick={() => setShowPreviews(!showPreviews)}>
                            <Eye size={14} />
                            <span className="settings-item-text">{t('show_previews' as any)}</span>
                            <Toggle checked={showPreviews} onChange={setShowPreviews} />
                        </div>
                    </div>

                    <div className="settings-column divider-left">
                        <div className="settings-label">{t('region_language' as any) || 'Region & Language'}</div>
                        <div className="settings-item" onClick={(e) => { e.stopPropagation(); onPageChange('languages'); }}>
                            <div className="settings-item-content">
                                <Globe size={14} />
                                {t('language')}
                            </div>
                            <ArrowRight size={12} className="settings-item-icon-right" />
                        </div>
                        <div className="settings-item" onClick={(e) => { e.stopPropagation(); onPageChange('dates'); }}>
                            <div className="settings-item-content">
                                <Calendar size={14} />
                                {t('date_format' as any)}
                            </div>
                            <ArrowRight size={12} className="settings-item-icon-right" />
                        </div>

                        <div className="settings-divider" />
                        <div className="settings-label">{t('performance' as any)}</div>
                        <div className="settings-item" onClick={() => setDefaultTurboMode(!defaultTurboMode)}>
                            <Zap size={14} />
                            <span className="settings-item-text">{t('mode_turbo' as any)}</span>
                            <Toggle checked={defaultTurboMode} onChange={setDefaultTurboMode} />
                        </div>
                        <div className="settings-item no-hover">
                            <div className="settings-item-content">
                                <Zap size={14} />
                                <span>{t('search_limit' as any)}</span>
                            </div>
                            <div className="settings-font-controls">
                                <button
                                    className="font-btn"
                                    onClick={() => setSearchLimit(searchLimit - 500)}
                                >
                                    <Minus size={12} />
                                </button>
                                <span
                                    className="settings-font-display"
                                    style={{ minWidth: '45px' }}
                                    onClick={() => setSearchLimit(3000)}
                                    title={t('reset_to_default' as any, { val: '3000' })}
                                >
                                    {searchLimit}
                                </span>
                                <button
                                    className="font-btn"
                                    onClick={() => setSearchLimit(searchLimit + 500)}
                                >
                                    <Plus size={12} />
                                </button>
                            </div>
                        </div>
                        <div className="settings-item" onClick={(e) => { e.stopPropagation(); onPageChange('compression'); }}>
                            <div className="settings-item-content">
                                <Zap size={14} />
                                {t('compression' as any)}
                            </div>
                            <ArrowRight size={12} className="settings-item-icon-right" />
                        </div>

                        <div className="settings-divider" />
                        <div className="settings-label">{t('data_management' as any) || 'Data Management'}</div>
                        <div className="settings-item" onClick={() => {
                            confirm(
                                t('clear_cache_desc' as any),
                                t('clear_cache' as any),
                                true
                            ).then(async (confirmed) => {
                                if (confirmed) {
                                    try {
                                        const { invoke } = await import('@tauri-apps/api/core');
                                        await invoke('clear_app_cache');
                                        notify(t('clear_cache_success' as any), 'success');
                                    } catch (e: any) {
                                        notify(`Error clearing cache: ${e}`, 'error');
                                    }
                                }
                            });
                            onClose();
                        }}>
                            <Trash2 size={14} className="error-text" />
                            <span className="error-text">{t('clear_cache' as any)}</span>
                        </div>
                        <div className="settings-item" onClick={() => {
                            confirm(
                                t('reset_config_desc' as any),
                                t('reset_config' as any),
                                true
                            ).then(async (confirmed) => {
                                if (confirmed) {
                                    try {
                                        await resetToDefaults();
                                        notify(t('reset_config_success' as any), 'success');
                                    } catch (e: any) {
                                        notify(`Error resetting config: ${e}`, 'error');
                                    }
                                }
                            });
                            onClose();
                        }}>
                            <RefreshCw size={14} className="warning-text" />
                            <span className="warning-text">{t('reset_config' as any)}</span>
                        </div>

                        <div className="settings-divider" style={{ marginTop: 'auto' }} />
                        <div className="settings-item" onClick={() => { onShowAbout(); onClose(); }}>
                            <Info size={14} /> {t('about')}
                        </div>
                    </div>
                </div>
            ) : page === 'themes' ? (
                <>
                    <div className="settings-item settings-item-back" onClick={(e) => { e.stopPropagation(); onPageChange('main'); }}>
                        <ArrowLeft size={14} /> {t('back')}
                    </div>

                    <div className="settings-label">{t('theme')}</div>
                    {[
                        { id: 'ayu-light', label: 'theme_ayu_light', icon: Sun },
                        { id: 'ayu-dark', label: 'theme_ayu_dark', icon: Moon },
                        { id: 'github-light', label: 'theme_github_light', icon: Sun },
                        { id: 'github-dark', label: 'theme_github_dark', icon: Moon },
                        { id: 'monokai', label: 'theme_monokai', icon: Moon },
                        { id: 'one-light', label: 'theme_one_light', icon: Sun },
                        { id: 'one-dark', label: 'theme_one_dark', icon: Moon },
                        { id: 'oxyde-light', label: 'theme_oxyde_light', icon: Sun },
                        { id: 'oxyde-dark', label: 'theme_oxyde_dark', icon: Moon },
                        { id: 'solarized-light', label: 'theme_solarized_light', icon: Sun },
                        { id: 'solarized-dark', label: 'theme_solarized_dark', icon: Moon },
                        { id: 'windows-light', label: 'theme_windows_light', icon: Sun },
                        { id: 'windows-dark', label: 'theme_windows_dark', icon: Moon },
                    ].map(item => (
                        <div
                            key={item.id}
                            className={cx("settings-item", { active: theme === item.id })}
                            onClick={() => { setTheme(item.id as Theme); }}
                        >
                            <item.icon size={14} />
                            {t(item.label as any)}
                            {theme === item.id && <Check size={12} className="settings-item-icon-right" />}
                        </div>
                    ))}
                </>
            ) : page === 'dates' ? (
                <>
                    <div className="settings-item settings-item-back" onClick={(e) => { e.stopPropagation(); onPageChange('main'); }}>
                        <ArrowLeft size={14} /> {t('back')}
                    </div>

                    <div className="settings-label">{t('date_format' as any)}</div>
                    {[
                        { id: 'US', label: 'date_us' },
                        { id: 'European', label: 'date_european' },
                        { id: 'ISO', label: 'date_iso' },
                    ].map(item => (
                        <div
                            key={item.id}
                            className={cx("settings-item", { active: dateFormat === item.id })}
                            onClick={() => { setDateFormat(item.id as DateFormat); }}
                        >
                            <Calendar size={14} />
                            {t(item.label as any)}
                            {dateFormat === item.id && <Check size={12} className="settings-item-icon-right" />}
                        </div>
                    ))}
                </>
            ) : page === 'languages' ? (
                <>
                    <div className="settings-item settings-item-back" onClick={(e) => { e.stopPropagation(); onPageChange('main'); }}>
                        <ArrowLeft size={14} /> {t('back')}
                    </div>

                    <div className="settings-label">{t('language')}</div>
                    {[
                        { id: 'en', label: 'english' },
                        { id: 'fr', label: 'french' },
                    ].map(item => (
                        <div
                            key={item.id}
                            className={cx("settings-item", { active: language === item.id })}
                            onClick={() => setLanguage(item.id as Language)}
                        >
                            <Globe size={14} />
                            {t(item.label as any)}
                            {language === item.id && <Check size={12} className="settings-item-icon-right" />}
                        </div>
                    ))}
                </>
            ) : page === 'compression' ? (
                <>
                    <div className="settings-item settings-item-back" onClick={(e) => { e.stopPropagation(); onPageChange('main'); }}>
                        <ArrowLeft size={14} /> {t('back')}
                    </div>

                    <div className="settings-label">{t('compression_quality' as any)}</div>

                    <div className="settings-sub-label">ZIP</div>
                    <div className="settings-quality-group">
                        {(['fast', 'normal', 'best'] as CompressionQuality[]).map(q => (
                            <button
                                key={q}
                                className={cx("quality-btn", { active: zipQuality === q })}
                                onClick={() => setZipQuality(q)}
                            >
                                {t(`quality_${q}` as any)}
                            </button>
                        ))}
                    </div>

                    <div className="settings-sub-label">7-Zip</div>
                    <div className="settings-quality-group">
                        {(['fast', 'normal', 'best'] as CompressionQuality[]).map(q => (
                            <button
                                key={q}
                                className={cx("quality-btn", { active: sevenZipQuality === q })}
                                onClick={() => setSevenZipQuality(q)}
                            >
                                {t(`quality_${q}` as any)}
                            </button>
                        ))}
                    </div>

                    <div className="settings-sub-label">Zstd</div>
                    <div className="settings-quality-group">
                        {(['fast', 'normal', 'best'] as CompressionQuality[]).map(q => (
                            <button
                                key={q}
                                className={cx("quality-btn", { active: zstdQuality === q })}
                                onClick={() => setZstdQuality(q)}
                            >
                                {t(`quality_${q}` as any)}
                            </button>
                        ))}
                    </div>
                </>
            ) : null}
        </div>
    );
};

