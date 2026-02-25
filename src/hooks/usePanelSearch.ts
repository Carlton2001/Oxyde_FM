/**
 * usePanelSearch — Extracted search state and streaming logic from usePanel.
 * 
 * Handles search lifecycle: query state, tab-scoped caching, Tauri event streaming,
 * buffered result updates, limit enforcement, and cleanup.
 */

import { useState, useEffect, useRef, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { FileEntry, SearchEventPayload } from '../types';
import { purgeIconCache } from '../components/ui/AsyncFileIcon';
import { isVirtualPath } from '../utils/path';

interface UsePanelSearchOptions {
    path: string;
    panelId?: string;
    activeTabId?: string;
    searchLimit: number;
    initialPath: string;
}

export interface PanelSearchState {
    searchQuery: string;
    searchResults: FileEntry[] | null;
    isSearching: boolean;
    searchLimitReached: boolean;
    currentSearchRoot: string;
    setSearchQuery: (q: string) => void;
    setSearchResults: React.Dispatch<React.SetStateAction<FileEntry[] | null>>;
    setIsSearching: (v: boolean) => void;
    setSearchLimitReached: (v: boolean) => void;
}

export const usePanelSearch = ({
    path, panelId, activeTabId, searchLimit, initialPath
}: UsePanelSearchOptions): PanelSearchState => {
    const [searchQuery, setSearchQuery] = useState('');
    const [searchResults, setSearchResults] = useState<FileEntry[] | null>(null);
    const [isSearching, setIsSearching] = useState(false);
    const [searchLimitReached, setSearchLimitReached] = useState(false);

    const pid = useMemo(() => panelId?.replace('panel-', '') || 'left', [panelId]);

    // Track last physical path for search root (per tab)
    const prevPathRef = useRef<string>(initialPath);
    const tabSearchRootCache = useRef<Map<string, string>>(new Map());
    const tabSearchQueryCache = useRef<Map<string, string>>(new Map());
    const lastActiveTabIdRef = useRef<string | undefined>(activeTabId);

    // Track which tab/path the current searchResults state originated from
    const resultsContextRef = useRef<{ id: string, path: string } | null>(null);

    // More complete search cache: { query, root, results, isComplete, limitReached }
    const tabSearchFullCache = useRef<Map<string, {
        path: string,
        query: string,
        root: string,
        results: FileEntry[],
        isComplete: boolean,
        limitReached: boolean
    }>>(new Map());

    // Search results buffering
    const searchBufferRef = useRef<FileEntry[]>([]);

    // Tab switch detection & query cache
    useEffect(() => {
        // DETECT TAB SWITCH
        if (activeTabId !== lastActiveTabIdRef.current) {
            lastActiveTabIdRef.current = activeTabId;

            if (activeTabId) {
                // Restore search query for the newly activated tab
                const cachedQuery = tabSearchQueryCache.current.get(activeTabId) || '';
                setSearchQuery(cachedQuery);
            }
            return;
        }

        // SAVE STATE (Only if results match current context to prevent poisoning)
        const isCorrectContext = resultsContextRef.current?.id === activeTabId && resultsContextRef.current?.path === path;

        if (activeTabId) {
            tabSearchQueryCache.current.set(activeTabId, searchQuery);

            if (path.startsWith('search://') && searchResults !== null && isCorrectContext) {
                const queryPart = path.replace('search://', '').split('?')[0];
                const params = new URLSearchParams(path.split('?')[1] || '');
                const uriRoot = params.get('root') || tabSearchRootCache.current.get(activeTabId) || '';

                tabSearchFullCache.current.set(activeTabId, {
                    path,
                    query: decodeURIComponent(queryPart),
                    root: uriRoot,
                    results: searchResults,
                    isComplete: !isSearching,
                    limitReached: searchLimitReached
                });
            }
        }
    }, [searchQuery, searchResults, isSearching, searchLimitReached, activeTabId, path]);

    // Search Lifecycle
    useEffect(() => {
        const isSearch = path.startsWith('search://') || path.startsWith('search:\\\\');

        if (!isSearch) {
            // ONLY update cache if the tab ID matches our current "stable" knowledge
            if (activeTabId && activeTabId === lastActiveTabIdRef.current) {
                if (!isVirtualPath(path)) {
                    prevPathRef.current = path;
                    tabSearchRootCache.current.set(activeTabId, path);
                }
            }
            // Reset UI state when leaving search
            if (searchResults !== null) {
                setSearchResults(null);
                setIsSearching(false);
                setSearchLimitReached(false);
                searchBufferRef.current = [];
                purgeIconCache();
            }
            return;
        }

        const searchPart = path.startsWith('search://')
            ? path.replace('search://', '')
            : path.replace('search:\\\\', '');

        const querySepIndex = searchPart.indexOf('?');
        let query = '';
        let uriRoot: string | null = null;

        const params = new URLSearchParams(querySepIndex !== -1 ? searchPart.substring(querySepIndex + 1) : '');

        if (querySepIndex !== -1) {
            query = decodeURIComponent(searchPart.substring(0, querySepIndex));
            uriRoot = params.get('root');
        } else {
            query = decodeURIComponent(searchPart);
        }

        // Check full cache for existing results for this tab
        const fullCache = activeTabId ? tabSearchFullCache.current.get(activeTabId) : null;
        const searchRoot = uriRoot || (activeTabId ? tabSearchRootCache.current.get(activeTabId) : null) || prevPathRef.current;

        // If we have cached results for the EXACT same query and root, restore them ONLY if complete
        if (fullCache && fullCache.path === path && fullCache.isComplete) {
            if (searchResults !== fullCache.results) {
                setSearchResults(fullCache.results);
                setSearchLimitReached(fullCache.limitReached);
                if (activeTabId) {
                    resultsContextRef.current = { id: activeTabId, path };
                }
            }
            setIsSearching(false);
            return; // SKIP BACKEND START
        } else {
            // New search or no cache - reset state
            setSearchResults([]);
            setSearchLimitReached(false);
            resultsContextRef.current = null;
        }

        setIsSearching(true);
        searchBufferRef.current = [];

        // Track total received to know when to cancel
        let totalReceived = 0;
        const MAX_RESULTS = searchLimit;
        let searchCancelled = false;

        // Track pending idle callbacks for cleanup
        let pendingIdleCallback: number | null = null;
        let isCleanedUp = false;

        // Listen for events - Buffer the results (with limit)
        const unlistenPromise = listen<SearchEventPayload>('search_event', (event) => {
            if (isCleanedUp || searchCancelled) return;

            const payload = event.payload;
            if (payload.panel_id === pid) {
                if (payload.completed) {
                    setIsSearching(false);
                }
                if (payload.results && payload.results.length > 0) {
                    // Update context on first results
                    if (searchBufferRef.current.length === 0 && activeTabId) {
                        resultsContextRef.current = { id: activeTabId, path };
                    }

                    // Only buffer what we need
                    const canAccept = MAX_RESULTS - totalReceived;
                    if (canAccept > 0) {
                        const toBuffer = payload.results.slice(0, canAccept);
                        searchBufferRef.current.push(...toBuffer);
                        totalReceived += toBuffer.length;

                        // Cancel search once we have enough
                        if (totalReceived >= MAX_RESULTS && !searchCancelled) {
                            searchCancelled = true;
                            invoke('cancel_search', { panelId: pid }).catch(console.error);
                            setIsSearching(false);
                            setSearchLimitReached(true);
                        }
                    }
                }
            }
        });

        // Throttled UI Update: Flush buffer every 600ms
        const scheduleFlush = (callback: () => void) => {
            if (isCleanedUp) return;

            if ('requestIdleCallback' in window) {
                pendingIdleCallback = (window as any).requestIdleCallback(callback, { timeout: 100 });
            } else {
                setTimeout(callback, 0);
            }
        };

        const flushInterval = setInterval(() => {
            if (isCleanedUp) {
                clearInterval(flushInterval);
                return;
            }

            if (searchBufferRef.current.length > 0) {
                const batch = [...searchBufferRef.current];
                searchBufferRef.current = [];

                scheduleFlush(() => {
                    if (isCleanedUp) return;

                    setSearchResults(prev => {
                        const MAX_DISPLAY = searchLimit;
                        if (!prev || prev.length === 0) return batch.slice(0, MAX_DISPLAY);
                        if (prev.length >= MAX_DISPLAY) return prev;
                        const remaining = MAX_DISPLAY - prev.length;
                        return prev.concat(batch.slice(0, remaining));
                    });
                });
            }
        }, 600);

        // Determine final search root for the backend
        const searchOptions = {
            query,
            searchRoot,
            regex: params.get('regex') === 'true',
            caseSensitive: params.get('case_sensitive') === 'true',
            recursive: params.get('recursive') !== 'false',
            minSize: params.get('min_size') ? parseInt(params.get('min_size')!) : undefined,
            maxSize: params.get('max_size') ? parseInt(params.get('max_size')!) : undefined,
            minDate: params.get('min_date') ? parseInt(params.get('min_date')!) : undefined,
            maxDate: params.get('max_date') ? parseInt(params.get('max_date')!) : undefined,
            contentQuery: params.get('content') || undefined,
            contentRegex: params.get('content_regex') === 'true',
            ignoreAccents: params.get('ignore_accents') === 'true',
            searchInArchives: params.get('search_in_archives') === 'true'
        };

        invoke('start_search', { panelId: pid, ...searchOptions })
            .catch(err => {
                console.error("Failed to start search:", err);
                setIsSearching(false);
            });

        return () => {
            isCleanedUp = true;

            unlistenPromise.then(u => u());
            clearInterval(flushInterval);

            // Cancel pending idle callback
            if (pendingIdleCallback && 'cancelIdleCallback' in window) {
                (window as any).cancelIdleCallback(pendingIdleCallback);
            }

            // CRITICAL: Clear buffer to free memory immediately
            searchBufferRef.current = [];

            // Cancel search when navigating away or unmounting
            invoke('cancel_search', { panelId: pid }).catch(console.error);
        };
    }, [path, panelId, activeTabId, searchLimit]);

    // Compute the current search root
    const currentSearchRoot = useMemo(() => {
        if (path && !path.startsWith('search://') && !path.startsWith('trash://')) {
            return path;
        }
        return (activeTabId ? tabSearchRootCache.current.get(activeTabId) : null) || prevPathRef.current;
    }, [path, activeTabId]);

    return {
        searchQuery,
        searchResults,
        isSearching,
        searchLimitReached,
        currentSearchRoot,
        setSearchQuery,
        setSearchResults,
        setIsSearching,
        setSearchLimitReached
    };
};
