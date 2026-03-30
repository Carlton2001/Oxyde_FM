import { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigation } from './useNavigation';
import { useFiles, getSortedFiles } from './useFileSystem';
import { useSelection } from './useSelection';
import { usePanelSearch } from './usePanelSearch';

import { useApp } from '../context/AppContext';
import { ViewMode, SortConfig, ColumnWidths, MultiModeColumnWidths, ColumnMode, SizeCategoryKey } from '../types';
import { getColumnMode } from '../config/columnDefinitions';

export const usePanel = (initialPath: string, panelId?: string, activeTabId?: string) => {
    const { showHidden, showSystem, searchLimit } = useApp();

    const normalizedPanelId = useMemo(() => {
        if (!panelId) return 'left' as 'left' | 'right';
        return (panelId.startsWith('panel-') ? panelId.replace('panel-', '') : panelId) as 'left' | 'right';
    }, [panelId]);

    // Navigation
    const { path, history, historyIndex, currentEntry, navigate, goToIndex, goBack, goForward, goUp, updateCurrentSelection, updateCurrentScroll, setNavigationState, version } = useNavigation(initialPath);

    useEffect(() => {
        if (panelId) {
            invoke('active_tab_navigate', { panelId: normalizedPanelId, path, version }).catch(err => {
                console.error("Failed to sync navigation:", err);
            });
        }
    }, [path, normalizedPanelId, version]);

    // Persistence identifiers
    const groupByDateKey = panelId ? `groupByDate_${panelId}` : null;

    // View State with localStorage persistence
    const [viewMode, setViewModeState] = useState<ViewMode>(() => {
        if (panelId) {
            const saved = localStorage.getItem(`viewMode_${panelId}`);
            if (saved && ['grid', 'details'].includes(saved)) {
                return saved as ViewMode;
            }
        }
        return 'grid';
    });

    const [groupByDate, setGroupByDateState] = useState<boolean>(() => {
        if (groupByDateKey) {
            const saved = localStorage.getItem(groupByDateKey);
            return saved === 'true';
        }
        return false;
    });

    // Persist viewMode when it changes
    const setViewMode = useCallback((mode: ViewMode) => {
        setViewModeState(mode);
        if (panelId) {
            localStorage.setItem(`viewMode_${panelId}`, mode);
        }
    }, [panelId]);

    const setGroupByDate = useCallback((val: boolean) => {
        setGroupByDateState(val);
        if (groupByDateKey) {
            localStorage.setItem(groupByDateKey, String(val));
        }
    }, [groupByDateKey]);

    // Filters
    const [extensionFilter, setExtensionFilter] = useState<Set<string> | null>(null);
    const [sizeFilter, setSizeFilter] = useState<Set<SizeCategoryKey> | null>(null);
    const [dateFilter, setDateFilter] = useState<Set<string> | null>(null);
    const [nameFilter, setNameFilter] = useState<string | null>(null);
    const [locationFilter, setLocationFilter] = useState<string | null>(null);
    const [deletedDateFilter, setDeletedDateFilter] = useState<Set<string> | null>(null);

    const clearAllFilters = useCallback(() => {
        setExtensionFilter(null);
        setSizeFilter(null);
        setDateFilter(null);
        setNameFilter(null);
        setLocationFilter(null);
        setDeletedDateFilter(null);
    }, []);

    const [sortConfig, setSortConfig] = useState<SortConfig>({ field: 'name', direction: 'asc' });

    // Sync sortConfig with backend session
    useEffect(() => {
        if (panelId) {
            invoke('update_sort_config', { panelId: normalizedPanelId, sortConfig }).catch(err => {
                console.error("Failed to sync sort config:", err);
            });
        }
    }, [sortConfig, normalizedPanelId]);

    // Multi-mode Column Widths management
    const [allColWidths, setAllColWidths] = useState<MultiModeColumnWidths>(() => {
        const defaults: ColumnWidths = { name: 250, location: 200, type: 80, size: 80, date: 160, deletedDate: 160 };
        const modes: ColumnMode[] = ['normal', 'search', 'trash', 'network'];

        try {
            const result: any = {};
            modes.forEach(m => {
                const saved = localStorage.getItem(`colWidths_${m}`);
                result[m] = saved ? JSON.parse(saved) : { ...defaults };
            });
            return result as MultiModeColumnWidths;
        } catch (e) {
            console.error("Failed to load column widths from localStorage", e);
            return {
                normal: { ...defaults },
                search: { ...defaults },
                trash: { ...defaults },
                network: { ...defaults }
            };
        }
    });

    // Search (extracted to usePanelSearch)
    const {
        searchQuery, searchResults, isSearching, searchLimitReached,
        currentSearchRoot, setSearchQuery, setSearchResults,
        setIsSearching, setSearchLimitReached, cancelSearch
    } = usePanelSearch({ path, panelId, activeTabId, searchLimit, initialPath });

    // Derive current mode (moved down to have access to searchResults)
    const isTrashView = !!path && /^(trash)(:\/\/|:\\{1,2})/i.test(path);
    const isNetworkView = path === '__network_vincinity__' || (!!path && path.startsWith('\\\\') && path.slice(2).split('\\').filter(Boolean).length === 1);
    const mode = getColumnMode(!!isTrashView, !!searchResults, isNetworkView);

    // Active widths for current mode
    const colWidths = useMemo(() => allColWidths[mode], [allColWidths, mode]);

    const setColWidths = useCallback((val: ColumnWidths | ((prev: ColumnWidths) => ColumnWidths)) => {
        setAllColWidths(prev => {
            const currentModeWidths = prev[mode];
            const nextModeWidths = typeof val === 'function' ? val(currentModeWidths) : val;

            // Persist to localStorage for this mode
            localStorage.setItem(`colWidths_${mode}`, JSON.stringify(nextModeWidths));

            return {
                ...prev,
                [mode]: nextModeWidths
            };
        });
    }, [mode]);

    // File System
    const { files, sortedFiles, summary, isComplete, loading, error, isProtected, refresh, setFiles, setSummary, setIsComplete, updateFileSize, setFileCalculating } = useFiles(normalizedPanelId, path, sortConfig, showHidden, showSystem, version);

    // Effective Files (Normal vs Search)
    const displayFiles = useMemo(() => {
        if (searchResults) {
            return getSortedFiles(searchResults, sortConfig);
        }
        return sortedFiles;
    }, [searchResults, sortedFiles, sortConfig]);

    // Selection
    const { selected, setSelected, lastSelectedPath, handleSelect, selectMultiple, clearSelection } = useSelection(displayFiles);

    // Sync selection with current history entry when path changes (navigation)
    const lastPathRef = useRef(path);
    useEffect(() => {
        if (path !== lastPathRef.current) {
            lastPathRef.current = path;
            const newSelected = new Set(currentEntry?.selected || []);
            setSelected(newSet => {
                if (newSet.size === newSelected.size && Array.from(newSet).every(p => newSelected.has(p))) {
                    return newSet;
                }
                return newSelected;
            });
        }
    }, [path, currentEntry, setSelected]);

    // Update selection in history when it changes locally
    useEffect(() => {
        updateCurrentSelection(Array.from(selected));
    }, [selected, updateCurrentSelection]);

    // State Export/Import for Tabs
    const getPanelState = useCallback(() => ({
        path,
        history,
        historyIndex,
        viewMode,
        sortConfig,
        searchQuery,
        searchResults,
        isSearching,
        searchLimitReached,
        files,
        summary,
        isComplete,
        version,
        selected: Array.from(selected),
        allColWidths,
        groupByDate,
        filters: {
            extensionFilter: extensionFilter ? Array.from(extensionFilter) : null,
            sizeFilter: sizeFilter ? Array.from(sizeFilter) : null,
            dateFilter: dateFilter ? Array.from(dateFilter) : null,
            nameFilter,
            locationFilter,
            deletedDateFilter: deletedDateFilter ? Array.from(deletedDateFilter) : null
        }
    }), [path, history, historyIndex, viewMode, sortConfig, searchQuery, searchResults, isSearching, searchLimitReached, files, summary, isComplete, version, selected, allColWidths, groupByDate, extensionFilter, sizeFilter, dateFilter, nameFilter, locationFilter, deletedDateFilter]);

    const setPanelState = useCallback((state: any) => {
        if (!state) return;
        setNavigationState({
            path: state.path,
            history: state.history,
            historyIndex: state.historyIndex,
            version: state.version || 0
        });
        setViewMode(state.viewMode);
        setSortConfig(state.sortConfig);
        if (state.searchQuery !== undefined) setSearchQuery(state.searchQuery);
        if (state.selected) setSelected(new Set(state.selected));
        if (state.allColWidths) setAllColWidths(state.allColWidths);
        if (state.groupByDate !== undefined) setGroupByDate(state.groupByDate);
        
        // Restore files and search results
        if (state.files) setFiles(state.files);
        if (state.summary) setSummary(state.summary);
        if (state.isComplete !== undefined) setIsComplete(state.isComplete);
        if (state.searchResults !== undefined) setSearchResults(state.searchResults);
        if (state.isSearching !== undefined) setIsSearching(state.isSearching);
        if (state.searchLimitReached !== undefined) setSearchLimitReached(state.searchLimitReached);

        // Restore filters
        if (state.filters) {
            setExtensionFilter(state.filters.extensionFilter ? new Set(state.filters.extensionFilter) : null);
            setSizeFilter(state.filters.sizeFilter ? new Set(state.filters.sizeFilter) : null);
            setDateFilter(state.filters.dateFilter ? new Set(state.filters.dateFilter) : null);
            setNameFilter(state.filters.nameFilter || null);
            setLocationFilter(state.filters.locationFilter || null);
            setDeletedDateFilter(state.filters.deletedDateFilter ? new Set(state.filters.deletedDateFilter) : null);
        } else {
            clearAllFilters();
        }
    }, [setNavigationState, setViewMode, setSortConfig, setSearchQuery, setSelected, setGroupByDate, clearAllFilters, setFiles, setSummary, setIsComplete, setSearchResults, setIsSearching, setSearchLimitReached]);

    // Navigation helpers
    const handleNavigate = useCallback((newPath: string, selection?: string[], forceVersion?: number) => {
        navigate(newPath, selection || Array.from(selected), forceVersion);
    }, [navigate, selected]);
    const handleGoBack = useCallback(() => goBack(Array.from(selected)), [goBack, selected]);
    const handleGoForward = useCallback(() => goForward(Array.from(selected)), [goForward, selected]);
    const handleGoUp = useCallback(() => goUp(Array.from(selected)), [goUp, selected]);
    const handleGoToIndex = useCallback((idx: number) => goToIndex(idx, Array.from(selected)), [goToIndex, selected]);

    return useMemo(() => ({
        // State
        path,
        version,
        files: displayFiles,
        loading,
        error,
        isProtected,
        selected,
        viewMode,
        sortConfig,
        history,
        historyIndex,
        searchQuery,
        searchResults,
        isSearching,
        searchLimitReached,
        summary,
        isComplete,
        colWidths,
        mode,
        isTrashView,
        isNetworkView,
        lastSelectedPath,
        currentSearchRoot,
        currentEntry,
        groupByDate,
        extensionFilter,
        sizeFilter,
        dateFilter,
        nameFilter,
        locationFilter,
        deletedDateFilter,

        // Actions
        navigate: handleNavigate,
        goBack: handleGoBack,
        goForward: handleGoForward,
        goUp: handleGoUp,
        goToIndex: handleGoToIndex,
        refresh,

        setViewMode,
        setGroupByDate,
        setSortConfig,
        setColWidths,

        setExtensionFilter,
        setSizeFilter,
        setDateFilter,
        setNameFilter,
        setLocationFilter,
        setDeletedDateFilter,
        clearAllFilters,

        setSearchQuery,
        setSearchResults,
        setIsSearching,
        setSearchLimitReached,
        cancelSearch,

        handleSelect,
        selectMultiple,
        clearSelection,
        setSelected,
        updateFileSize,
        setFileCalculating,
        updateCurrentScroll,

        // Tab Support
        getPanelState,
        setPanelState,
        setNavigationState,
    }), [
        path, displayFiles, loading, error, isProtected, selected, viewMode, sortConfig,
        history, historyIndex, searchQuery, searchResults, isSearching, searchLimitReached,
        summary, isComplete, currentSearchRoot, currentEntry,
        colWidths, mode, isTrashView, isNetworkView, lastSelectedPath, groupByDate,
        extensionFilter, sizeFilter, dateFilter, nameFilter, locationFilter, deletedDateFilter,
        navigate, goBack, goForward, goUp, goToIndex, refresh,
        setViewMode, setGroupByDate, setSortConfig, setColWidths,
        setExtensionFilter, setSizeFilter, setDateFilter, setNameFilter, setLocationFilter, setDeletedDateFilter, clearAllFilters,
        setSearchQuery, setSearchResults, setIsSearching, setSearchLimitReached,
        handleSelect, selectMultiple, clearSelection, setSelected, updateFileSize, setFileCalculating, updateCurrentScroll,
        getPanelState, setPanelState, setNavigationState
    ]);
};

