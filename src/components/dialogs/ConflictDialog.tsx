import React, { useState, useRef } from 'react';
import { X, Search, Check, Ban, CheckCircle2, XCircle, ChevronDown, ChevronRight, Settings2 } from 'lucide-react';
import cx from 'classnames';
import { DateFormat, ConflictEntry, ConflictAction } from '../../types';
import { TFunc } from '../../i18n';
import { useDraggable } from '../../hooks/useDraggable';
import { useResizable } from '../../hooks/useResizable';
import { getParent } from '../../utils/path';
import { formatSize, formatDate } from '../../utils/format';
import { getFileIcon } from '../../utils/fileIcons';
import { useApp } from '../../context/AppContext';
import '../../styles/components/Dialogs.css';
import '../ui/SearchBox.css';
import './SearchDialog.css';

interface ConflictDetailPanelProps {
    current: ConflictEntry;
    t: TFunc;
    dateFormat: DateFormat;
    useSystemIcons: boolean;
    onAction: (action: ConflictAction) => void;
    inheritedRes?: { action: ConflictAction; parent: string } | null;
}

const ConflictDetailPanel: React.FC<ConflictDetailPanelProps> = ({ current, t, dateFormat, useSystemIcons, onAction, inheritedRes }) => {
    const isSourceNewer = current.source.modified > current.target.modified;
    const isTargetNewer = current.target.modified > current.source.modified;
    const isSourceLarger = current.source.size > current.target.size;
    const isTargetLarger = current.target.size > current.source.size;
    const isSourceNewerButSmaller = isSourceNewer && current.source.size < current.target.size;
    const isTargetNewerButSmaller = isTargetNewer && current.target.size < current.source.size;

    const getIcon = (entry: any) => {
        return getFileIcon(entry.name, entry.is_dir, { size: 40, strokeWidth: 1.5 }, useSystemIcons, entry.path);
    };

    const formatItemsCount = (entry: any) => {
        if (!entry.is_dir || (entry.folders_count === undefined && entry.files_count === undefined)) return '';
        const folders = entry.folders_count || 0;
        const files = entry.files_count || 0;
        const total = folders + files;
        if (total === 0) return '';
        return `(${total} ${t(total > 1 ? 'items' : 'item')})`;
    };

    return (
        <div className="conflict-comparison">
            <div className={cx("conflict-row source", { "is-newer": isSourceNewer })}>
                <div className="conflict-row-label">
                    <span>{t('source_file' as any)}</span>
                    {isSourceNewer && <span className={cx("delta-badge newer", { "warning": isSourceNewerButSmaller })}>{t('newer' as any)}</span>}
                    {isSourceLarger && <span className="delta-badge larger">{t('larger')}</span>}
                </div>
                <div className="conflict-row-content">
                    <div className="conflict-icon-wrapper">{getIcon(current.source)}</div>
                    <div className="conflict-info-main">
                        <div className="conflict-name-row"><span className="name">{current.name}</span></div>
                        <div className="conflict-meta-row">
                            <div className="meta-item"><span className="label">{t('size')} :</span> <span className={cx("value", { "highlight": isSourceLarger })}>{formatSize(current.source.size, 1, t)} {formatItemsCount(current.source)}</span></div>
                            <div className="meta-item"><span className="label">{t('date')} :</span> <span className={cx("value", { "highlight": isSourceNewer })}>{formatDate(current.source.modified, dateFormat)}</span></div>
                        </div>
                        <div className="conflict-path-row">{getParent(current.source.path)}</div>
                    </div>
                </div>
            </div>

            <div className="conflict-divider"><div className="divider-vs">VS</div></div>

            <div className={cx("conflict-row target", { "is-newer": isTargetNewer })}>
                <div className="conflict-row-label">
                    <span>{t('target_file' as any)}</span>
                    {isTargetNewer && <span className={cx("delta-badge newer", { "warning": isTargetNewerButSmaller })}>{t('newer' as any)}</span>}
                    {isTargetLarger && <span className="delta-badge larger">{t('larger')}</span>}
                </div>
                <div className="conflict-row-content">
                    <div className="conflict-icon-wrapper">{getIcon(current.target)}</div>
                    <div className="conflict-info-main">
                        <div className="conflict-name-row"><span className="name">{current.name}</span></div>
                        <div className="conflict-meta-row">
                            <div className="meta-item"><span className="label">{t('size')} :</span> <span className={cx("value", { "highlight": isTargetLarger })}>{formatSize(current.target.size, 1, t)} {formatItemsCount(current.target)}</span></div>
                            <div className="meta-item"><span className="label">{t('date')} :</span> <span className={cx("value", { "highlight": isTargetNewer })}>{formatDate(current.target.modified, dateFormat)}</span></div>
                        </div>
                        <div className="conflict-path-row">{getParent(current.target.path)}</div>
                    </div>
                </div>
            </div>

            {inheritedRes && (
                <div className={cx("inherited-resolution-badge", inheritedRes.action)}>
                    {inheritedRes.action === 'replace' ? <CheckCircle2 size={12} className="replace" /> : <XCircle size={12} className="skip" />}
                    <span>{t('inherited_from' as any) || 'Inherited from'} <b>{inheritedRes.parent}</b> : {t(inheritedRes.action as any)}</span>
                </div>
            )}

            <div className="detail-actions">
                <button className="btn" onClick={() => onAction('skip')}>
                    <Ban size={14} />
                    {t('skip' as any)}
                </button>
                <button className="btn primary" onClick={() => onAction('replace')}>
                    <Check size={14} />
                    {t('replace' as any)}
                </button>
            </div>
        </div>
    );
};

interface ConflictDialogProps {
    conflicts: ConflictEntry[];
    onResolve: (resolutions: Map<string, ConflictAction>) => void;
    onCancel: () => void;
    t: TFunc;
    operation: 'copy' | 'move';
    totalCount: number;
    zIndex?: number;
    onFocus?: () => void;
}

export const ConflictDialog: React.FC<ConflictDialogProps> = ({ conflicts, onResolve, onCancel, t, operation, totalCount, zIndex, onFocus }) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const [currentIndex, setCurrentIndex] = useState(0);
    const [resolutions, setResolutions] = useState<Record<string, ConflictAction>>({});
    const [viewMode, setViewMode] = useState<'simple' | 'detailed'>('simple');
    const [filterQuery, setFilterQuery] = useState('');
    const [activeFilter, setActiveFilter] = useState<'all' | 'identical' | 'newer' | 'older'>('all');
    const [collapsedPaths, setCollapsedPaths] = useState<Set<string>>(new Set());
    
    // Standard Oxyde resizing logic
    const { size, handleResizeStart } = useResizable({
        initialSize: (() => {
            const saved = localStorage.getItem('conflict_dialog_size');
            return saved ? JSON.parse(saved) : { width: 832, height: 576 };
        })(),
        minSize: { width: 760, height: 480 }
    });

    const [sidebarWidth, setSidebarWidth] = useState(400);
    const isResizingSidebar = useRef(false);

    // Use a stable ref to avoid closure issues in the delta calculation
    const sidebarWidthRef = useRef(sidebarWidth);
    sidebarWidthRef.current = sidebarWidth;
    
    const startResizingSidebar = (e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        isResizingSidebar.current = true;
        
        const startX = e.clientX;
        const initialWidth = sidebarWidthRef.current;

        const onMouseMove = (ev: MouseEvent) => {
            if (!isResizingSidebar.current) return;
            const delta = ev.clientX - startX;
            const newWidth = initialWidth + delta;
            
            // Limit between 150px and size.width - 150px
            if (newWidth > 150 && newWidth < size.width - 150) {
                setSidebarWidth(newWidth);
            }
        };

        const onMouseUp = () => {
            isResizingSidebar.current = false;
            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', onMouseUp);
            document.body.style.cursor = 'default';
        };

        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
        document.body.style.cursor = 'col-resize';
    };

    React.useEffect(() => {
        localStorage.setItem('conflict_dialog_size', JSON.stringify(size));
    }, [size]);

    const current = conflicts[currentIndex];
    
    // Filtering logic
    const filteredConflicts = React.useMemo(() => {
        return conflicts.filter(c => {
            // Tab filter
            if (activeFilter === 'newer' && !(c.source.modified > c.target.modified)) return false;
            if (activeFilter === 'older' && !(c.target.modified > c.source.modified)) return false;
            if (activeFilter === 'identical' && !(c.source.modified === c.target.modified && c.source.size === c.target.size)) return false;

            // Search filter
            if (filterQuery && !c.name.toLowerCase().includes(filterQuery.toLowerCase())) return false;
            
            // Hierarchy filter (folding)
            // If any ancestor in the conflict list is collapsed, hide this item
            let currentPath = c.source.path;
            while (currentPath.includes('\\') || currentPath.includes('/')) {
                const lastIdx = Math.max(currentPath.lastIndexOf('\\'), currentPath.lastIndexOf('/'));
                const parent = currentPath.substring(0, lastIdx);
                if (collapsedPaths.has(parent)) return false;
                currentPath = parent;
            }
            
            return true;
        });
    }, [conflicts, filterQuery, activeFilter, collapsedPaths]);

    const counts = React.useMemo(() => {
        return {
            all: conflicts.length,
            identical: conflicts.filter(c => c.source.modified === c.target.modified && c.source.size === c.target.size).length,
            newer: conflicts.filter(c => c.source.modified > c.target.modified).length,
            older: conflicts.filter(c => c.target.modified > c.source.modified).length,
        };
    }, [conflicts]);

    // Hierarchical indentation calculation
    const conflictDepths = React.useMemo(() => {
        if (conflicts.length === 0) return {};
        
        // Find common root length (shortest part count)
        const splitPaths = conflicts.map(c => c.source.path.split(/[/\\]/));
        const minPathLength = Math.min(...splitPaths.map(p => p.length));
        
        const depths: Record<string, number> = {};
        conflicts.forEach(c => {
            const parts = c.source.path.split(/[/\\]/);
            // Indent based on depth relative to the shared base
            depths[c.source.path] = Math.max(0, parts.length - minPathLength);
        });
        return depths;
    }, [conflicts]);


    // Update currentIndex if it goes out of bounds of filtered list
    React.useEffect(() => {
        if (currentIndex >= filteredConflicts.length) {
            setCurrentIndex(0);
        }
    }, [filteredConflicts.length, currentIndex]);

    const displayEntry = filteredConflicts[currentIndex] || current;

    const isMultiple = conflicts.length > 1;

    const { useSystemIcons, dateFormat } = useApp();

    const handleAction = (action: ConflictAction) => {
        // In hybrid view, displayEntry is the filtered entry being shown — not necessarily current.
        // We must resolve based on what is actually displayed, not the global index.
        const targetEntry = viewMode === 'detailed' ? displayEntry : current;
        if (!targetEntry) return;

        const newResolutions = { ...resolutions };
        newResolutions[targetEntry.source.path] = action;
        setResolutions(newResolutions);
        
        if (viewMode === 'detailed') {
            // In hybrid view: navigate to next unresolved item in filteredConflicts
            const nextIdx = filteredConflicts.findIndex((c, i) => i > currentIndex && !newResolutions[c.source.path]);
            if (nextIdx !== -1) setCurrentIndex(nextIdx);
        } else {
            // In simple view: step through conflicts sequentially
            if (currentIndex + 1 < conflicts.length) {
                setCurrentIndex(currentIndex + 1);
            } else {
                // Auto-resolve when reaching the end in simple view
                const resultMap = new Map<string, ConflictAction>();
                Object.entries(newResolutions).forEach(([k, v]) => resultMap.set(k, v));
                onResolve(resultMap);
            }
        }
    };

    const handleBulkAction = (action: ConflictAction) => {
        const newResolutions = { ...resolutions };
        // Apply to all conflicts, not just filtered ones, if called from simple view 
        // Or if the user really means "Everything"
        const listToResolve = viewMode === 'simple' ? conflicts : filteredConflicts;
        
        listToResolve.forEach(c => {
            newResolutions[c.source.path] = action;
        });
        setResolutions(newResolutions);

        if (viewMode === 'simple') {
            const resultMap = new Map<string, ConflictAction>();
            Object.entries(newResolutions).forEach(([k, v]) => resultMap.set(k, v));
            onResolve(resultMap);
        }
    };

    const getInheritedResolution = (path: string): { action: ConflictAction; parent: string } | null => {
        let currentPath = path;
        while (currentPath.includes('\\') || currentPath.includes('/')) {
            const lastIndex = Math.max(currentPath.lastIndexOf('\\'), currentPath.lastIndexOf('/'));
            const parent = currentPath.substring(0, lastIndex);
            if (resolutions[parent]) {
                return { 
                    action: resolutions[parent], 
                    parent: parent.split(/[\\/]/).pop() || parent 
                };
            }
            currentPath = parent;
        }
        return null;
    };

    const toggleCollapse = (path: string, e: React.MouseEvent) => {
        e.stopPropagation();
        const next = new Set(collapsedPaths);
        if (next.has(path)) {
            next.delete(path);
        } else {
            next.add(path);
        }
        setCollapsedPaths(next);
    };

    const handleApplyDecisions = () => {
        const resultMap = new Map<string, ConflictAction>();
        Object.entries(resolutions).forEach(([k, v]) => resultMap.set(k, v));
        onResolve(resultMap);
    };

    const resolvedCount = Object.keys(resolutions).length;
    // A conflict is considered resolved if it has an explicit resolution OR an inherited one
    const effectivelyResolvedCount = conflicts.filter(c =>
        resolutions[c.source.path] !== undefined || getInheritedResolution(c.source.path) !== null
    ).length;
    const isAllResolved = effectivelyResolvedCount >= conflicts.length;

    if (!current) return null;

    const renderDetailedView = () => {
        return (
            <div className="conflict-overlay" style={{ zIndex }}>
                <div
                    ref={dragRef}
                    className="properties-dialog conflict-dialog hybrid"
                    style={{
                        width: `${size.width}px`,
                        height: `${size.height}px`,
                        transform: `translate(${position.x}px, ${position.y}px)`,
                        transition: 'none'
                    }}
                >
                    {/* Header */}
                    <div className="prop-header-bar" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                        <div className="prop-title">
                            {t('conflict_reso' as any) || 'Conflict Resolution'}
                        </div>
                        <button className="btn-icon" onClick={onCancel}><X size={16} /></button>
                    </div>

                    <div className="search-tabs">
                        <button 
                            className={cx("search-tab", { active: activeFilter === 'all' })} 
                            onClick={() => setActiveFilter('all')}
                            disabled={counts.all === 0}
                        >
                            {t('all' as any)} {counts.all > 0 && <span className="tab-count">{counts.all}</span>}
                        </button>
                        <button 
                            className={cx("search-tab", { active: activeFilter === 'identical' })} 
                            onClick={() => setActiveFilter('identical')}
                            disabled={counts.identical === 0}
                        >
                            {t('identical')} {counts.identical > 0 && <span className="tab-count">{counts.identical}</span>}
                        </button>
                        <button 
                            className={cx("search-tab", { active: activeFilter === 'newer' })} 
                            onClick={() => setActiveFilter('newer')}
                            disabled={counts.newer === 0}
                        >
                            {t('newer')} {counts.newer > 0 && <span className="tab-count">{counts.newer}</span>}
                        </button>
                        <button 
                            className={cx("search-tab", { active: activeFilter === 'older' })} 
                            onClick={() => setActiveFilter('older')}
                            disabled={counts.older === 0}
                        >
                            {t('older')} {counts.older > 0 && <span className="tab-count">{counts.older}</span>}
                        </button>
                        <div style={{ flex: 1 }} />
                        <div className="hybrid-search-container">
                            <div className="input-with-icon icon-right hybrid-search-bar">
                                <Search size={14} className="input-icon right" />
                                <input
                                    type="text"
                                    value={filterQuery}
                                    placeholder={t('filter_results')}
                                    onChange={(e) => setFilterQuery(e.target.value)}
                                />
                            </div>
                        </div>
                    </div>

                    {/* Main Content Area */}
                    <div className="hybrid-main-grid" style={{ gridTemplateColumns: `${sidebarWidth}px 6px 1fr` }}>
                        {/* Sidebar */}
                        <div className="hybrid-sidebar" style={{ minWidth: 0, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                            <div className="hybrid-list">
                                {filteredConflicts.map((c, idx) => {
                                    const res = resolutions[c.source.path];
                                    const inherited = !res ? getInheritedResolution(c.source.path) : null;
                                    return (
                                        <div 
                                            key={idx} 
                                            className={cx("hybrid-list-item", { 
                                                active: idx === currentIndex,
                                                inherited: !!inherited && !res
                                            })}
                                            onClick={() => setCurrentIndex(idx)}
                                            style={{ paddingLeft: `${0.5 + (conflictDepths[c.source.path] || 0) * 1}rem` }}
                                        >
                                            <div className="item-collapse-toggle">
                                                {c.source.is_dir && (
                                                    <div className="btn-icon-tiny" onClick={(e) => toggleCollapse(c.source.path, e)}>
                                                        {collapsedPaths.has(c.source.path) 
                                                            ? <ChevronRight size={14} /> 
                                                            : <ChevronDown size={14} />
                                                        }
                                                    </div>
                                                )}
                                            </div>
                                            <div className="item-icon-small">
                                                {getFileIcon(c.name, c.source.is_dir, { size: 16 }, useSystemIcons, c.source.path)}
                                            </div>
                                            <div className="item-info">
                                                <span className="name">{c.name}</span>
                                                <span className="path">{getParent(c.source.path)}</span>
                                            </div>
                                            <div className="item-status-icons">
                                                {res === 'replace' && <CheckCircle2 size={12} className="status-icon-res replace" />}
                                                {res === 'skip' && <XCircle size={12} className="status-icon-res skip" />}
                                                {inherited && !res && (
                                                    inherited.action === 'replace' 
                                                        ? <CheckCircle2 size={12} className="status-icon-res replace inherited" />
                                                        : <XCircle size={12} className="status-icon-res skip inherited" />
                                                )}
                                                <div className={cx("status-dot", { 
                                                    newer: c.source.modified > c.target.modified,
                                                    older: c.target.modified > c.source.modified,
                                                    identical: c.source.modified === c.target.modified && c.source.size === c.target.size 
                                                })} />
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                            
                            <div className="hybrid-sidebar-footer">
                                <div className="legend-item">
                                    <div className="status-dot newer"></div>
                                    <span>{t('legend_newer' as any)}</span>
                                </div>
                                <div className="legend-item">
                                    <div className="status-dot older"></div>
                                    <span>{t('legend_older' as any)}</span>
                                </div>
                                <div className="legend-item">
                                    <div className="status-dot identical"></div>
                                    <span>{t('legend_identical' as any)}</span>
                                </div>
                            </div>
                        </div>

                        {/* Resize Handle (Grab) */}
                        <div 
                            className="hybrid-resizer-handle"
                            onMouseDown={startResizingSidebar}
                        />

                        {/* Detail Column */}
                        <div className="hybrid-detail" style={{ minWidth: 0, display: 'flex', flexDirection: 'column', overflowY: 'auto' }}>
                            {displayEntry ? (
                                <ConflictDetailPanel 
                                    current={displayEntry} 
                                    t={t} 
                                    dateFormat={dateFormat}
                                    useSystemIcons={useSystemIcons}
                                    onAction={handleAction}
                                    inheritedRes={!resolutions[displayEntry.source.path] ? getInheritedResolution(displayEntry.source.path) : null}
                                />
                            ) : (
                                <div className="hybrid-empty-state-large">
                                    {t('select_item_to_compare' as any) || 'Select an item to compare'}
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Footer */}
                    <div className="prop-footer">
                        <button className="btn" onClick={() => setViewMode('simple')}>{t('back' as any) || 'Back'}</button>
                        <div style={{ flex: 1 }} />
                        <button 
                            className={cx("btn", { primary: isAllResolved })} 
                            onClick={handleApplyDecisions}
                        >
                            {resolvedCount > 0 
                                ? `${t('apply_decisions' as any) || 'Apply'} (${resolvedCount})`
                                : t('apply_decisions' as any) || 'Apply'}
                        </button>
                    </div>

                    {/* Resize Corner Handle (Standard Oxyde) */}
                    <div 
                        className="conflict-resize-handle"
                        onMouseDown={handleResizeStart}
                    >
                        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                            <path d="M11 1L1 11M11 5L5 11M11 9L9 11" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                        </svg>
                    </div>
                </div>
            </div>
        );
    };

    return viewMode === 'detailed' ? renderDetailedView() : (
        <div className="conflict-overlay simple" style={{ zIndex }}>
            <div
                ref={dragRef}
                className="properties-dialog conflict-dialog simple"
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="prop-header-bar" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <div className="prop-title">
                        {currentIndex + 1} / {conflicts.length} {t('conflict' as any)}
                    </div>
                    <div style={{ fontSize: '0.65rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.05em', opacity: 0.5, marginRight: '1rem' }}>
                        {t(operation as any)} : {totalCount} {totalCount > 1 ? t('items') : t('item')}
                    </div>
                    <button className="btn-icon" onClick={onCancel}><X size={16} /></button>
                </div>

                <div className="prop-content" style={{ padding: '1.25rem' }}>
                    <div className="conflict-message">
                        {t('conflict_msg' as any).replace('{name}', current.name)}
                    </div>

                    <ConflictDetailPanel 
                        current={current} 
                        t={t} 
                        dateFormat={dateFormat}
                        useSystemIcons={useSystemIcons}
                        onAction={handleAction}
                    />
                </div>

                <div className="prop-footer">
                    <button className="btn" onClick={onCancel} style={{ marginRight: 'auto' }}>
                        {t('cancel_all' as any)}
                    </button>
                    {isMultiple && (
                        <button className="btn btn-advanced" onClick={() => setViewMode('detailed')}>
                            <Settings2 size={16} />
                            {t('compare_all')}
                        </button>
                    )}
                    {isMultiple ? (
                        <>
                            <button className="btn" onClick={() => handleBulkAction('skip')} style={{ minWidth: '8rem' }}>
                                {t('skip_all' as any) || 'Tout ignorer'}
                            </button>
                            <button className="btn primary" onClick={() => handleBulkAction('replace')} style={{ minWidth: '8rem' }}>
                                {t('replace_all' as any) || 'Tout remplacer'}
                            </button>
                        </>
                    ) : (
                        <>
                            <button className="btn" onClick={() => handleAction('skip')} style={{ minWidth: '7rem' }}>
                                {t('skip' as any)}
                            </button>
                            <button className="btn primary" onClick={() => handleAction('replace')} style={{ minWidth: '7rem' }}>
                                {t('replace' as any)}
                            </button>
                        </>
                    )}
                </div>
            </div>
        </div>
    );
};

