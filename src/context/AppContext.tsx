import React, { createContext, useContext, useEffect, useCallback, ReactNode } from 'react';
import { Theme, LayoutMode, Language, DateFormat, CompressionQuality } from '../types';
import { getT, TFunc } from '../i18n';
import { useNotifications } from '../hooks/useNotifications';
import { useDrives } from '../hooks/useFileSystem';
import { useRustConfig } from '../hooks/useRustConfig';
import { DriveInfo, NotificationType, AppNotification } from '../types';

interface AppContextValue {
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

    // Translation
    t: TFunc;

    // Notifications
    notifications: AppNotification[];
    notify: (message: string, type?: NotificationType, duration?: number) => void;
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

    refreshDrives: () => void;
}

const AppContext = createContext<AppContextValue | null>(null);

interface AppProviderProps {
    children: ReactNode;
}

export const AppProvider: React.FC<AppProviderProps> = ({ children }) => {
    const { config, isLoading, setConfigValue } = useRustConfig();
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
        showCheckboxes: false
    };

    // Derived state (or defaults)
    const theme = (config?.theme as Theme) || (localStorage.getItem('fm_theme') as Theme) || defaults.theme;
    const layout = (config?.layout as LayoutMode) || defaults.layout;
    const language = (config?.language as Language) || defaults.language;
    const showHidden = config?.show_hidden ?? defaults.showHidden;
    const showSystem = config?.show_system ?? defaults.showSystem;
    const useSystemIcons = config?.use_system_icons ?? defaults.useSystemIcons;
    const dateFormat = (config?.date_format as DateFormat) || defaults.dateFormat;
    const showPreviews = config?.show_previews ?? defaults.showPreviews;
    const zipQuality = (config?.zip_quality as CompressionQuality) || defaults.zipQuality;
    const sevenZipQuality = (config?.seven_zip_quality as CompressionQuality) || defaults.sevenZipQuality;
    const zstdQuality = (config?.zstd_quality as CompressionQuality) || defaults.zstdQuality;
    const cachedFontSize = localStorage.getItem('fm_fontSize');
    const fontSize = config?.font_size ?? (cachedFontSize ? parseInt(cachedFontSize, 10) : defaults.fontSize);
    const searchLimit = config?.search_limit ?? defaults.searchLimit;
    const defaultTurboMode = config?.default_turbo_mode ?? defaults.defaultTurboMode;
    const showGridThumbnails = config?.show_grid_thumbnails ?? defaults.showGridThumbnails;
    const showCheckboxes = config?.show_checkboxes ?? defaults.showCheckboxes;

    // Setters (memoized to avoid new refs on every render)
    const setTheme = useCallback((v: Theme) => setConfigValue('theme', v), [setConfigValue]);
    const setLayout = useCallback((v: LayoutMode) => setConfigValue('layout', v), [setConfigValue]);
    const setLanguage = useCallback((v: Language) => setConfigValue('language', v), [setConfigValue]);
    const setShowHidden = useCallback((v: boolean) => setConfigValue('show_hidden', v), [setConfigValue]);
    const setShowSystem = useCallback((v: boolean) => setConfigValue('show_system', v), [setConfigValue]);
    const setUseSystemIcons = useCallback((v: boolean) => setConfigValue('use_system_icons', v), [setConfigValue]);
    const setDateFormat = useCallback((v: DateFormat) => setConfigValue('date_format', v), [setConfigValue]);
    const setShowPreviews = useCallback((v: boolean) => setConfigValue('show_previews', v), [setConfigValue]);
    const setZipQuality = useCallback((v: CompressionQuality) => setConfigValue('zip_quality', v), [setConfigValue]);
    const setSevenZipQuality = useCallback((v: CompressionQuality) => setConfigValue('seven_zip_quality', v), [setConfigValue]);
    const setZstdQuality = useCallback((v: CompressionQuality) => setConfigValue('zstd_quality', v), [setConfigValue]);
    const setDefaultTurboMode = useCallback((v: boolean) => setConfigValue('default_turbo_mode', v), [setConfigValue]);
    const setShowGridThumbnails = useCallback((v: boolean) => setConfigValue('show_grid_thumbnails', v), [setConfigValue]);
    const setShowCheckboxes = useCallback((v: boolean) => setConfigValue('show_checkboxes', v), [setConfigValue]);

    const setFontSize = useCallback((size: number) => {
        const newSize = Math.max(10, Math.min(24, size));
        setConfigValue('font_size', newSize);
    }, [setConfigValue]);

    const setSearchLimit = useCallback((limit: number) => {
        const newLimit = Math.max(10, Math.min(50000, limit));
        setConfigValue('search_limit', newLimit);
    }, [setConfigValue]);

    const t = getT(language);

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
        refreshDrives
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

