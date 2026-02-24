import React, { useState, useRef } from 'react';
import { X, ArrowRight } from 'lucide-react';
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

    const getIcon = (entry: any) => {
        return getFileIcon(
            entry.name || entry.path.split('\\').pop(),
            entry.is_dir,
            { size: 32, strokeWidth: 1.5 },
            useSystemIcons,
            entry.path
        );
    };

    const handleAction = (action: ConflictAction) => {
        if (applyToAll) {
            // Apply this action to all remaining conflicts
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
        <div className="properties-overlay" style={{ zIndex: 11000 }}>
            <div
                ref={dragRef}
                className="properties-dialog conflict-dialog"
                style={{
                    width: '62.5rem',
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

                <div className="prop-content" style={{ padding: '1.5rem' }}>
                    <div className="conflict-message" style={{ marginBottom: '1.5rem', fontSize: '0.9rem' }}>
                        {t('conflict_msg' as any).replace('{name}', current.name)}
                    </div>

                    <div className="conflict-comparison">
                        {/* Source File (New) - LEFT */}
                        <div className="conflict-card highlighted">
                            <div className="conflict-card-header">
                                <span className="conflict-card-label">{t('source_file' as any)}</span>
                            </div>
                            <div className="conflict-file-info">
                                {getIcon(current.source)}
                                <div className="conflict-details">
                                    <div className="conflict-name" data-tooltip={current.source.path}>{current.name}</div>
                                    <div className="conflict-info-grid">
                                        <div className="conflict-info-label">{t('location')}</div>
                                        <div className="conflict-info-value">{getParent(current.source.path) || ''}</div>

                                        <div className="conflict-info-label">{t('date')}</div>
                                        <div className="conflict-info-value">{formatDate(current.source.modified, dateFormat)}</div>

                                        <div className="conflict-info-label">{t('size')}</div>
                                        <div className="conflict-info-value">{formatSize(current.source.size, 1, t)}</div>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div className="conflict-arrow">
                            <ArrowRight size={24} style={{ opacity: 0.3 }} />
                        </div>

                        {/* Target File (Existing) - RIGHT */}
                        <div className="conflict-card">
                            <div className="conflict-card-header">
                                <span className="conflict-card-label">{t('target_file' as any)}</span>
                            </div>
                            <div className="conflict-file-info">
                                {getIcon(current.target)}
                                <div className="conflict-details">
                                    <div className="conflict-name" data-tooltip={current.target.path}>{current.name}</div>
                                    <div className="conflict-info-grid">
                                        <div className="conflict-info-label">{t('location')}</div>
                                        <div className="conflict-info-value">{getParent(current.target.path) || ''}</div>

                                        <div className="conflict-info-label">{t('date')}</div>
                                        <div className="conflict-info-value">{formatDate(current.target.modified, dateFormat)}</div>

                                        <div className="conflict-info-label">{t('size')}</div>
                                        <div className="conflict-info-value">{formatSize(current.target.size, 1, t)}</div>
                                    </div>
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
                    <button className="btn" onClick={() => handleAction('skip')} style={{ minWidth: '6.25rem' }}>
                        {t('skip' as any)}
                    </button>
                    <button className="btn primary" onClick={() => handleAction('replace')} style={{ minWidth: '6.25rem' }}>
                        {t('replace' as any)}
                    </button>
                </div>
            </div>

            <style>{`
                .conflict-comparison {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                }
                .conflict-card {
                    flex: 1;
                    padding: 1rem;
                    background: var(--surface-secondary);
                    border: 1px solid var(--border-color);
                    border-radius: 0.5rem;
                    display: flex;
                    flex-direction: column;
                    gap: 0.5rem;
                }
                .conflict-card.highlighted {
                    border-color: var(--accent-color);
                    background: color-mix(in srgb, var(--accent-color), transparent 95%);
                }
                .conflict-card-label {
                    font-size: 0.75rem;
                    font-weight: 600;
                    opacity: 0.6;
                    text-transform: uppercase;
                }
                .conflict-file-info {
                    display: flex;
                    gap: 1rem;
                    align-items: flex-start;
                }
                .conflict-details {
                    display: flex;
                    flex-direction: column;
                    min-width: 0;
                }
                .conflict-name {
                    font-weight: 600;
                    font-size: 0.85rem;
                    word-break: break-all;
                    margin-bottom: 2px;
                }
                .conflict-info-grid {
                    display: grid;
                    grid-template-columns: max-content 1fr;
                    gap: 0.25rem 0.75rem;
                    margin-top: 0.5rem;
                    align-items: baseline;
                }
                .conflict-info-label {
                    font-size: 0.75rem;
                    opacity: 0.6;
                }
                .conflict-info-value {
                    font-size: 0.8125rem;
                    word-break: break-all;
                    font-weight: 500;
                }
            `}</style>
        </div>
    );
};

