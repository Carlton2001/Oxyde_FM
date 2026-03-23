import React, { useState, useRef } from 'react';
import { X } from 'lucide-react';
import cx from 'classnames';
import { ConflictEntry, ConflictAction } from '../../types';
import { Toggle } from '../ui/Toggle';
import { TFunc } from '../../i18n';
import { useDraggable } from '../../hooks/useDraggable';
import { getParent } from '../../utils/path';
import { formatSize, formatDate } from '../../utils/format';
import { getFileIcon } from '../../utils/fileIcons';
import { useApp } from '../../context/AppContext';
import '../../styles/components/Dialogs.css';

interface ConflictDialogProps {
    conflicts: ConflictEntry[];
    onResolve: (resolutions: Map<string, ConflictAction>) => void;
    onCancel: () => void;
    t: TFunc;
    operation: 'copy' | 'move';
    totalCount: number;
}

export const ConflictDialog: React.FC<ConflictDialogProps> = ({ conflicts, onResolve, onCancel, t, operation, totalCount }) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const [currentIndex, setCurrentIndex] = useState(0);
    const [applyToAll, setApplyToAll] = useState(false);
    const [resolutions] = useState(new Map<string, ConflictAction>());

    const current = conflicts[currentIndex];
    const isMultiple = conflicts.length > 1;

    const { useSystemIcons, dateFormat } = useApp();

    const isSourceNewer = current.source.modified > current.target.modified;
    const isTargetNewer = current.target.modified > current.source.modified;
    const isSourceLarger = current.source.size > current.target.size;
    const isTargetLarger = current.target.size > current.source.size;

    const isSourceNewerButSmaller = isSourceNewer && (current.source.size < current.target.size);
    const isTargetNewerButSmaller = isTargetNewer && (current.target.size < current.source.size);

    const getIcon = (entry: any) => {
        return getFileIcon(
            entry.name || entry.path.split('\\').pop(),
            entry.is_dir,
            { size: 40, strokeWidth: 1.5 },
            useSystemIcons,
            entry.path
        );
    };

    const formatItemsCount = (entry: any) => {
        if (!entry.is_dir) return null;
        if (entry.files_count === undefined && entry.folders_count === undefined) return null;
        const total = (entry.files_count || 0) + (entry.folders_count || 0);
        return `(${total} ${total > 1 ? t('items') : t('item')})`;
    };

    const handleAction = (action: ConflictAction) => {
        if (applyToAll) {
            const newResolutions = new Map(resolutions);
            for (let i = currentIndex; i < conflicts.length; i++) {
                newResolutions.set(conflicts[i].source.path, action);
            }
            onResolve(newResolutions);
        } else {
            resolutions.set(current.source.path, action);
            if (currentIndex + 1 < conflicts.length) {
                setCurrentIndex(currentIndex + 1);
            } else {
                onResolve(resolutions);
            }
        }
    };

    if (!current) return null;

    return (
        <div className="properties-overlay">
            <div
                ref={dragRef}
                className="properties-dialog conflict-dialog v2"
                style={{
                    width: '38rem',
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="prop-header-bar" onMouseDown={handleMouseDown}>
                    <div className="prop-title">
                        {currentIndex + 1} / {conflicts.length} {t('conflict' as any)}
                    </div>
                    <div style={{ fontSize: '0.65rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.05em', opacity: 0.5, marginRight: '1rem' }}>
                        {t(operation as any)} : {totalCount} {totalCount > 1 ? t('items') : t('item')}
                    </div>
                    <button className="btn-icon" onClick={onCancel}><X size={16} /></button>
                </div>

                <div className="prop-content" style={{ padding: '1.25rem' }}>
                    <div className="conflict-message-v2">
                        {t('conflict_msg' as any).replace('{name}', current.name)}
                    </div>

                    <div className="conflict-comparison-v2">
                        {/* Source File (Incoming) */}
                        <div className={cx("conflict-row source", { "is-newer": isSourceNewer })}>
                            <div className="conflict-row-label">
                                <span>{t('source_file' as any)}</span>
                                {isSourceNewer && <span className={cx("delta-badge newer", { "warning": isSourceNewerButSmaller })}>{t('newer' as any) || 'Newer'}</span>}
                                {isSourceLarger && <span className="delta-badge larger">{t('larger') || 'Larger'}</span>}
                            </div>
                            <div className="conflict-row-content">
                                <div className="conflict-icon-wrapper">
                                    {getIcon(current.source)}
                                </div>
                                <div className="conflict-info-main">
                                    <div className="conflict-name-row">
                                        <span className="name">{current.name}</span>
                                    </div>
                                    <div className="conflict-meta-row">
                                        <div className="meta-item">
                                            <span className="label">{t('size')} :</span>
                                            <span className={cx("value", { "highlight": isSourceLarger })}>
                                                {formatSize(current.source.size, 1, t)} {formatItemsCount(current.source)}
                                            </span>
                                        </div>
                                        <div className="meta-item">
                                            <span className="label">{t('date')} :</span>
                                            <span className={cx("value", { "highlight": isSourceNewer })}>
                                                {formatDate(current.source.modified, dateFormat)}
                                            </span>
                                        </div>
                                    </div>
                                    <div className="conflict-path-row">{getParent(current.source.path)}</div>
                                </div>
                            </div>
                        </div>

                        <div className="conflict-divider">
                            <div className="divider-line" />
                            <div className="divider-vs">VS</div>
                            <div className="divider-line" />
                        </div>

                        {/* Target File (Existing) */}
                        <div className={cx("conflict-row target", { "is-newer": isTargetNewer })}>
                            <div className="conflict-row-label">
                                <span>{t('target_file' as any)}</span>
                                {isTargetNewer && <span className={cx("delta-badge newer", { "warning": isTargetNewerButSmaller })}>{t('newer' as any) || 'Newer'}</span>}
                                {isTargetLarger && <span className="delta-badge larger">{t('larger') || 'Larger'}</span>}
                            </div>
                            <div className="conflict-row-content">
                                <div className="conflict-icon-wrapper">
                                    {getIcon(current.target)}
                                </div>
                                <div className="conflict-info-main">
                                    <div className="conflict-name-row">
                                        <span className="name">{current.name}</span>
                                    </div>
                                    <div className="conflict-meta-row">
                                        <div className="meta-item">
                                            <span className="label">{t('size')} :</span>
                                            <span className={cx("value", { "highlight": isTargetLarger })}>
                                                {formatSize(current.target.size, 1, t)} {formatItemsCount(current.target)}
                                            </span>
                                        </div>
                                        <div className="meta-item">
                                            <span className="label">{t('date')} :</span>
                                            <span className={cx("value", { "highlight": isTargetNewer })}>
                                                {formatDate(current.target.modified, dateFormat)}
                                            </span>
                                        </div>
                                    </div>
                                    <div className="conflict-path-row">{getParent(current.target.path)}</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <div className="prop-footer" style={{ gap: '0.75rem' }}>
                    <button className="btn" onClick={onCancel} style={{ marginRight: 'auto' }}>
                        {t('cancel_all' as any)}
                    </button>
                    {isMultiple && (
                        <Toggle
                            checked={applyToAll}
                            onChange={(val) => setApplyToAll(val)}
                            label={t('apply_all' as any)}
                        />
                    )}
                    <button className="btn" onClick={() => handleAction('skip')} style={{ minWidth: '7rem' }}>
                        {t('skip' as any)}
                    </button>
                    <button className="btn primary" onClick={() => handleAction('replace')} style={{ minWidth: '7rem' }}>
                        {t('replace' as any)}
                    </button>
                </div>
            </div>
        </div>
    );
};

