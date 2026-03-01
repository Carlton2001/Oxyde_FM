import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface AppConfig {
    theme: string;
    language: string;
    layout: string;
    show_hidden: boolean;
    show_system: boolean;
    use_system_icons: boolean;
    date_format: string;
    show_previews: boolean;
    zip_quality: string;
    seven_zip_quality: string;
    zstd_quality: string;
    font_size: number;
    search_limit: number;
    default_turbo_mode: boolean;
    show_grid_thumbnails: boolean;
    show_checkboxes: boolean;
    show_network: boolean;
}

export const useRustConfig = () => {
    const [config, setConfig] = useState<AppConfig | null>(null);
    const [isLoading, setIsLoading] = useState(true);

    const refreshConfig = useCallback(async () => {
        try {
            const current = await invoke<AppConfig>('get_config');
            setConfig(current);
        } catch (e) {
            console.error("Failed to fetch config:", e);
        } finally {
            setIsLoading(false);
        }
    }, []);

    useEffect(() => {
        refreshConfig();
    }, [refreshConfig]);

    const setConfigValue = useCallback(async (key: keyof AppConfig, value: any) => {
        try {
            // Frontend keeps the correct type (boolean/number) for immediate UI update
            setConfig(prev => prev ? { ...prev, [key]: value } : null);

            // Backend expects a string for the generic set_config_value command
            const stringValue = value.toString();
            await invoke('set_config_value', { key, value: stringValue });
        } catch (e) {
            console.error(`Failed to set config ${key}:`, e);
            // Revert on failure by refreshing
            refreshConfig();
        }
    }, [refreshConfig]);

    return {
        config,
        isLoading,
        setConfigValue,
        refreshConfig
    };
};
