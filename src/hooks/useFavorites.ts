import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { QuickAccessItem } from '../types';

export function useFavorites() {
    const [favorites, setFavorites] = useState<QuickAccessItem[]>([]);

    const fetchFavorites = async () => {
        try {
            const items: QuickAccessItem[] = await invoke('get_quick_access_items');
            setFavorites(items);
        } catch (err) {
            console.error("Failed to fetch favorites:", err);
        }
    };

    const handleRemoveFavorite = async (path: string) => {
        // Optimistic update
        setFavorites((prev) => prev.filter((f) => f.path !== path));
        try {
            await invoke('remove_from_quick_access', { path });
            // Double check after a longer delay because Windows can be slow to update quick access cache
            setTimeout(fetchFavorites, 1000);
            setTimeout(fetchFavorites, 3000);
        } catch (e) {
            console.error(e);
            fetchFavorites(); // Rollback on error
        }
    };

    useEffect(() => {
        let isFetching = false;
        let pendingFetch = false;

        const debouncedFetchFavorites = async () => {
            if (isFetching) {
                pendingFetch = true;
                return;
            }

            isFetching = true;
            try {
                await fetchFavorites();
            } finally {
                isFetching = false;
                if (pendingFetch) {
                    pendingFetch = false;
                    fetchFavorites();
                }
            }
        };

        debouncedFetchFavorites();

        const unlistenPromise = listen('quick-access-changed', () => {
            setTimeout(debouncedFetchFavorites, 200);
        });

        return () => {
            unlistenPromise.then(unlisten => unlisten());
        };
    }, []);

    return { favorites, fetchFavorites, setFavorites, handleRemoveFavorite };
}
