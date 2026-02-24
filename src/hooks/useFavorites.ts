import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { QuickAccessItem } from '../types';

export function useFavorites() {
    const [favorites, setFavorites] = useState<QuickAccessItem[]>([]);

    useEffect(() => {
        let isFetching = false;
        let pendingFetch = false;

        const fetchFavorites = async () => {
            if (isFetching) {
                pendingFetch = true;
                return;
            }

            isFetching = true;
            try {
                const items: QuickAccessItem[] = await invoke('get_quick_access_items');
                setFavorites(items);
            } catch (err) {
                console.error("Failed to fetch favorites:", err);
            } finally {
                isFetching = false;
                if (pendingFetch) {
                    pendingFetch = false;
                    fetchFavorites();
                }
            }
        };

        fetchFavorites();

        const unlistenPromise = listen('quick-access-changed', () => {
            setTimeout(fetchFavorites, 200);
        });

        return () => {
            unlistenPromise.then(unlisten => unlisten());
        };
    }, []);

    return favorites;
}
