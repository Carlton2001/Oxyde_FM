import React, { createContext, useContext, useEffect, useCallback, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Theme, LayoutMode, Language, DateFormat, CompressionQuality, DriveInfo, NotificationType, AppNotification } from '../types';
import { getT, TFunc } from '../i18n';
import { useNotifications } from '../hooks/useNotifications';
import { useDrives } from '../hooks/useFileSystem';
import { useRustConfig } from '../hooks/useRustConfig';

export interface AppContextValue {
    // Settings
    theme: Theme;
    layout: LayoutMode;
    language: Language;
    showHidden: boolean;
    showSystem: boolean;
    useSystemIcons: boolean;
    dateFormat: DateFormat;
    showPreviews: boolean;
    zipQuality: CompressionQuality;
    sevenZipQuality: CompressionQuality;
    zstdQuality: CompressionQuality;
    defaultTurboMode: boolean;
    showGridThumbnails: boolean;
    showCheckboxes: boolean;
    showNetwork: boolean;
    confirmDelete: boolean;
    updateAvailable: boolean;
    firstLaunch: boolean;
    peekStatus: {
        installed: boolean;
        enabled: boolean;
        space_enabled: boolean;
        activation_shortcut: string | null;
    } | null;

    // Setters
    setTheme: (theme: Theme) => void;
    setLayout: (layout: LayoutMode) => void;
    setLanguage: (language: Language) => void;
    setShowHidden: (show: boolean) => void;
    setShowSystem: (show: boolean) => void;
    setUseSystemIcons: (use: boolean) => void;
    setDateFormat: (format: DateFormat) => void;
    setShowPreviews: (show: boolean) => void;
    setZipQuality: (quality: CompressionQuality) => void;
    setSevenZipQuality: (quality: CompressionQuality) => void;
    setZstdQuality: (quality: CompressionQuality) => void;
    setDefaultTurboMode: (enabled: boolean) => void;
    setShowGridThumbnails: (show: boolean) => void;
    setShowCheckboxes: (show: boolean) => void;
    setShowNetwork: (show: boolean) => void;
    setConfirmDelete: (show: boolean) => void;
    setUpdateAvailable: (available: boolean) => void;
    setFirstLaunch: (val: boolean) => void;

    // Translation
    t: TFunc;

    // Notifications
    notifications: AppNotification[];
    notify: (message: string, type?: NotificationType, duration?: number) => string | undefined;
    dismissNotification: (id: string) => void;

    // Drives
    drives: DriveInfo[];
    mountedImages: string[];

    // Font Size
    fontSize: number;
    setFontSize: (size: number) => void;

    // Search Limit
    searchLimit: number;
    setSearchLimit: (limit: number) => void;
    isTrashEmpty: boolean;
    refreshTrashStatus: () => Promise<void>;

    refreshDrives: () => void;
    resetToDefaults: () => Promise<void>;
    driveTrashConfigs: Record<string, { nukeOnDelete: boolean }>;
    refreshDriveTrashConfigs: () => Promise<void>;
}

const AppContext = createContext<AppContextValue | null>(null);

interface AppProviderProps {
    children: ReactNode;
}

export const AppProvider: React.FC<AppProviderProps> = ({ children }) => {
    const { config, isLoading, setConfigValue, refreshConfig } = useRustConfig();
    const { notifications, notify, dismissNotification } = useNotifications();
    const { drives, mountedImages, refreshDrives } = useDrives();

    // Defaults (used while loading or if config missing)
    const defaults = {
        theme: 'oxyde-dark' as Theme,
        layout: 'standard' as LayoutMode,
        language: 'en' as Language,
        showHidden: false,
        showSystem: false,
        useSystemIcons: false,
        dateFormat: 'European' as DateFormat,
        showPreviews: true,
        zipQuality: 'fast' as CompressionQuality,
        sevenZipQuality: 'fast' as CompressionQuality,
        zstdQuality: 'fast' as CompressionQuality,
        fontSize: 16,
        searchLimit: 3000,
        defaultTurboMode: true,
        showGridThumbnails: false,
        showCheckboxes: false,
        showNetwork: true,
        confirmDelete: true
    };

    // Derived state (or defaults)
    const theme = (config?.theme as Theme) || (localStorage.getItem('fm_theme') as Theme) || defaults.theme;
    const layout = (config?.layout as LayoutMode) || (localStorage.getItem('fm_layout') as LayoutMode) || defaults.layout;
    const language = (config?.language as Language) || (localStorage.getItem('fm_language') as Language) || defaults.language;
    const showHidden = config?.show_hidden ?? (localStorage.getItem('fm_showHidden') === 'true' || (localStorage.getItem('fm_showHidden') === null && defaults.showHidden));
    const showSystem = config?.show_system ?? (localStorage.getItem('fm_showSystem') === 'true' || (localStorage.getItem('fm_showSystem') === null && defaults.showSystem));
    const useSystemIcons = config?.use_system_icons ?? (localStorage.getItem('fm_useSystemIcons') === 'true' || (localStorage.getItem('fm_useSystemIcons') === null && defaults.useSystemIcons));
    const dateFormat = (config?.date_format as DateFormat) || (localStorage.getItem('fm_dateFormat') as DateFormat) || defaults.dateFormat;
    const showPreviews = config?.show_previews ?? (localStorage.getItem('fm_showPreviews') === 'true' || (localStorage.getItem('fm_showPreviews') === null && defaults.showPreviews));
    const zipQuality = (config?.zip_quality as CompressionQuality) || (localStorage.getItem('fm_zipQuality') as CompressionQuality) || defaults.zipQuality;
    const sevenZipQuality = (config?.seven_zip_quality as CompressionQuality) || (localStorage.getItem('fm_sevenZipQuality') as CompressionQuality) || defaults.sevenZipQuality;
    const zstdQuality = (config?.zstd_quality as CompressionQuality) || (localStorage.getItem('fm_zstdQuality') as CompressionQuality) || defaults.zstdQuality;
    const cachedFontSize = localStorage.getItem('fm_fontSize');
    const fontSize = config?.font_size ?? (cachedFontSize ? parseInt(cachedFontSize, 10) : defaults.fontSize);
    const searchLimit = config?.search_limit ?? (localStorage.getItem('fm_searchLimit') ? parseInt(localStorage.getItem('fm_searchLimit')!, 10) : defaults.searchLimit);
    const defaultTurboMode = config?.default_turbo_mode ?? (localStorage.getItem('fm_defaultTurboMode') === 'true' || (localStorage.getItem('fm_defaultTurboMode') === null && defaults.defaultTurboMode));
    const showGridThumbnails = config?.show_grid_thumbnails ?? (localStorage.getItem('fm_showGridThumbnails') === 'true' || (localStorage.getItem('fm_showGridThumbnails') === null && defaults.showGridThumbnails));
    const showCheckboxes = config?.show_checkboxes ?? (localStorage.getItem('fm_showCheckboxes') === 'true' || (localStorage.getItem('fm_showCheckboxes') === null && defaults.showCheckboxes));
    const showNetwork = config?.show_network ?? (localStorage.getItem('fm_showNetwork') === 'true' || (localStorage.getItem('fm_showNetwork') === null && defaults.showNetwork));
    const confirmDelete = config?.confirm_delete ?? (localStorage.getItem('fm_confirmDelete') === 'true' || (localStorage.getItem('fm_confirmDelete') === null && defaults.confirmDelete));
    const firstLaunch = config?.first_launch ?? (localStorage.getItem('fm_first_launch') !== 'false');
    const [updateAvailable, setUpdateAvailable] = React.useState(false);
    const [isTrashEmpty, setIsTrashEmpty] = React.useState(true);
    const [peekStatus, setPeekStatus] = React.useState<AppContextValue['peekStatus']>(null);
    const [driveTrashConfigs, setDriveTrashConfigs] = React.useState<Record<string, { nukeOnDelete: boolean }>>({});

    useEffect(() => {
        invoke<AppContextValue['peekStatus']>('get_peek_status')
            .then(setPeekStatus)
            .catch((e) => {
                console.error("Failed to get Peek status", e);
                setPeekStatus({ installed: false, enabled: false, space_enabled: false, activation_shortcut: null });
            });
    }, []);

    const refreshDriveTrashConfigs = useCallback(async () => {
        const newConfigs: Record<string, { nukeOnDelete: boolean }> = {};
        await Promise.all(
            drives.map(async (drive) => {
                try {
                    const drivePath = drive.path.endsWith('\\') || drive.path.endsWith('/') ? drive.path : drive.path + '\\';
                    const config: { nuke_on_delete: boolean } = await invoke('get_recycle_bin_config', { drivePath: drivePath });
                    const normKey = drive.path.toLowerCase().match(/^([a-z]:)/)?.[1] || drive.path.toLowerCase();
                    newConfigs[normKey] = { nukeOnDelete: config.nuke_on_delete };
                } catch (err) {
                    // Ignore errors for individual drives
                }
            })
        );
        setDriveTrashConfigs(newConfigs);
    }, [drives]);

    const refreshTrashStatus = useCallback(async () => {
        try {
            const [usageSize, usageItems] = await invoke<[number, number]>('get_total_recycle_bin_usage');
            setIsTrashEmpty(usageItems === 0);
            refreshDriveTrashConfigs();
        } catch (e) {
            console.error("Failed to refresh trash status", e);
        }
    }, [refreshDriveTrashConfigs]);

    useEffect(() => {
        refreshTrashStatus();
        const interval = setInterval(refreshTrashStatus, 30000);
        return () => clearInterval(interval);
    }, [refreshTrashStatus, drives.length]);

    // Setters (memoized to avoid new refs on every render)
    const setTheme = useCallback((v: Theme) => {
        localStorage.setItem('fm_theme', v);
        setConfigValue('theme', v);
    }, [setConfigValue]);
    const setLayout = useCallback((v: LayoutMode) => {
        localStorage.setItem('fm_layout', v);
        setConfigValue('layout', v);
    }, [setConfigValue]);
    const setLanguage = useCallback((v: Language) => {
        localStorage.setItem('fm_language', v);
        setConfigValue('language', v);
    }, [setConfigValue]);
    const setShowHidden = useCallback((v: boolean) => {
        localStorage.setItem('fm_showHidden', v.toString());
        setConfigValue('show_hidden', v);
    }, [setConfigValue]);
    const setShowSystem = useCallback((v: boolean) => {
        localStorage.setItem('fm_showSystem', v.toString());
        setConfigValue('show_system', v);
    }, [setConfigValue]);
    const setUseSystemIcons = useCallback((v: boolean) => {
        localStorage.setItem('fm_useSystemIcons', v.toString());
        setConfigValue('use_system_icons', v);
    }, [setConfigValue]);
    const setDateFormat = useCallback((v: DateFormat) => {
        localStorage.setItem('fm_dateFormat', v);
        setConfigValue('date_format', v);
    }, [setConfigValue]);
    const setShowPreviews = useCallback((v: boolean) => {
        localStorage.setItem('fm_showPreviews', v.toString());
        setConfigValue('show_previews', v);
    }, [setConfigValue]);
    const setZipQuality = useCallback((v: CompressionQuality) => {
        localStorage.setItem('fm_zipQuality', v);
        setConfigValue('zip_quality', v);
    }, [setConfigValue]);
    const setSevenZipQuality = useCallback((v: CompressionQuality) => {
        localStorage.setItem('fm_sevenZipQuality', v);
        setConfigValue('seven_zip_quality', v);
    }, [setConfigValue]);
    const setZstdQuality = useCallback((v: CompressionQuality) => {
        localStorage.setItem('fm_zstdQuality', v);
        setConfigValue('zstd_quality', v);
    }, [setConfigValue]);
    const setDefaultTurboMode = useCallback((v: boolean) => {
        localStorage.setItem('fm_defaultTurboMode', v.toString());
        setConfigValue('default_turbo_mode', v);
    }, [setConfigValue]);
    const setShowGridThumbnails = useCallback((v: boolean) => {
        localStorage.setItem('fm_showGridThumbnails', v.toString());
        setConfigValue('show_grid_thumbnails', v);
    }, [setConfigValue]);
    const setShowCheckboxes = useCallback((v: boolean) => {
        localStorage.setItem('fm_showCheckboxes', v.toString());
        setConfigValue('show_checkboxes', v);
    }, [setConfigValue]);
    const setShowNetwork = useCallback((v: boolean) => {
        localStorage.setItem('fm_showNetwork', v.toString());
        setConfigValue('show_network', v);
    }, [setConfigValue]);

    const setConfirmDelete = useCallback((v: boolean) => {
        localStorage.setItem('fm_confirmDelete', v.toString());
        setConfigValue('confirm_delete', v);
    }, [setConfigValue]);


    const setFontSize = useCallback((size: number) => {
        const newSize = Math.max(10, Math.min(24, size));
        localStorage.setItem('fm_fontSize', newSize.toString());
        setConfigValue('font_size', newSize);
    }, [setConfigValue]);

    const setFirstLaunch = useCallback((val: boolean) => {
        localStorage.setItem('fm_first_launch', val.toString());
        setConfigValue('first_launch', val);
    }, [setConfigValue]);

    const setSearchLimit = useCallback((limit: number) => {
        const newLimit = Math.max(10, Math.min(50000, limit));
        localStorage.setItem('fm_searchLimit', newLimit.toString());
        setConfigValue('search_limit', newLimit);
    }, [setConfigValue]);

    const t = getT(language);

    const resetToDefaults = useCallback(async () => {
        try {
            await invoke('reset_config_to_default');
            const keysToRemove = [
                'fm_theme', 'fm_layout', 'fm_language', 'fm_showHidden', 'fm_showSystem',
                'fm_useSystemIcons', 'fm_dateFormat', 'fm_showPreviews', 'fm_zipQuality',
                'fm_sevenZipQuality', 'fm_zstdQuality', 'fm_fontSize', 'fm_searchLimit',
                'fm_defaultTurboMode', 'fm_showGridThumbnails', 'fm_showCheckboxes'
            ];
            keysToRemove.forEach(k => localStorage.removeItem(k));

            // Clear panel-specific settings
            for (let i = 0; i < localStorage.length; i++) {
                const key = localStorage.key(i);
                if (key && (key.startsWith('viewMode_') || key.startsWith('groupByDate_') || key.startsWith('colWidths_'))) {
                    localStorage.removeItem(key);
                    i--; // Adjust index after removal
                }
            }
            await setConfigValue('first_launch', true);
            await refreshConfig();
        } catch (e) {
            console.error('Failed to reset defaults', e);
        }
    }, [refreshConfig]);

    // Apply theme and font size
    useEffect(() => {
        document.documentElement.setAttribute('data-theme', theme);
        localStorage.setItem('fm_theme', theme);
        document.documentElement.style.fontSize = `${fontSize}px`;
        localStorage.setItem('fm_fontSize', fontSize.toString());
    }, [theme, fontSize]);

    // Global Wheel listener
    useEffect(() => {
        const handleWheel = (e: WheelEvent) => {
            if (e.ctrlKey || e.metaKey) {
                e.preventDefault();
                const delta = e.deltaY > 0 ? -1 : 1;
                setFontSize(fontSize + delta);
            }
        };
        window.addEventListener('wheel', handleWheel, { passive: false });
        return () => window.removeEventListener('wheel', handleWheel);
    }, [fontSize]); // dependent on current fontSize to calc next

    const value: AppContextValue = {
        theme,
        layout,
        language,
        showHidden,
        showSystem,
        fontSize,
        setTheme,
        setLayout,
        setLanguage,
        setShowHidden,
        setShowSystem,
        setUseSystemIcons,
        setFontSize,
        t,
        notifications,
        notify,
        dismissNotification,
        drives,
        mountedImages,
        useSystemIcons,
        dateFormat,
        setDateFormat,
        showPreviews,
        setShowPreviews,
        zipQuality,
        setZipQuality,
        sevenZipQuality,
        setSevenZipQuality,
        zstdQuality,
        setZstdQuality,
        searchLimit,
        setSearchLimit,
        defaultTurboMode,
        setDefaultTurboMode,
        showGridThumbnails,
        setShowGridThumbnails,
        showCheckboxes,
        setShowCheckboxes,
        showNetwork,
        setShowNetwork,
        confirmDelete,
        setConfirmDelete,
        updateAvailable,
        setUpdateAvailable,
        firstLaunch,
        setFirstLaunch,
        peekStatus,
        isTrashEmpty,
        refreshTrashStatus,
        refreshDrives,
        resetToDefaults,
        driveTrashConfigs,
        refreshDriveTrashConfigs
    };

    if (isLoading) {
        // Option: Render loading spinner or just return null/children with defaults
        // Returning children with defaults prevents flicker if loading is fast
        // But might cause jump if saved config differs.
        // Let's return children, as we have defaults.
    }

    return (
        <AppContext.Provider value={value}>
            {children}
        </AppContext.Provider>
    );
};

export const useApp = (): AppContextValue => {
    const context = useContext(AppContext);
    if (!context) {
        throw new Error('useApp must be used within an AppProvider');
    }
    return context;
};

