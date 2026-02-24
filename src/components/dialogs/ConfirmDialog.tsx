import React, { useRef } from 'react';
import { AlertTriangle, X } from 'lucide-react';
import { TFunc } from '../../i18n';
import { useDraggable } from '../../hooks/useDraggable';
import '../../styles/components/Dialogs.css';
import cx from 'classnames';

interface ConfirmDialogProps {
    isOpen: boolean;
    title: string;
    message: string;
    subMessage?: string;
    onClose: () => void;
    onConfirm: () => void;
    t: TFunc;
    confirmLabel?: string;
    isDanger?: boolean;
    sources?: string[];
    destination?: string;
}

export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
    isOpen,
    title,
    message,
    subMessage,
    onClose,
    onConfirm,
    t,
    confirmLabel,
    isDanger = false,
    sources,
    destination
}) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });

    if (!isOpen) return null;

    const renderSources = () => {
        if (!sources || sources.length === 0) return null;
        const count = sources.length;
        if (count === 1) return sources[0];
        return `${sources[0]} (+${count - 1})`;
    };

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div
                ref={dragRef}
                className="modal"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="modal-header" onMouseDown={handleMouseDown}>
                    <div className="modal-title">
                        {isDanger && <AlertTriangle size={14} color="#ef4444" style={{ marginRight: '0.5rem' }} />}
                        <span>{title}</span>
                    </div>
                    <button className="btn-icon" onClick={onClose} style={{ marginLeft: '0.5rem' }}>
                        <X size={16} />
                    </button>
                </div>
                {(sources || destination) && (
                    <div className="progress-details" style={{ padding: '0.75rem 1rem', background: 'var(--surface-secondary)', borderBottom: '1px solid var(--border-color)' }}>
                        {sources && sources.length > 0 && (
                            <div>{t('source_dir' as any)}: {renderSources()}</div>
                        )}
                        {destination && (
                            <div>{t('target_dir' as any)}: {destination}</div>
                        )}
                    </div>
                )}

                {(message || subMessage) && (
                    <div className="modal-content" style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem', fontSize: '0.875rem' }}>
                        {message && <p style={{ margin: 0, fontWeight: 500, lineHeight: 1.4 }}>{message}</p>}
                        {subMessage && <p style={{ margin: 0, fontSize: '0.8125rem', color: 'var(--text-muted)', lineHeight: 1.4 }}>{subMessage}</p>}
                    </div>
                )}

                <div className="modal-footer">
                    <button className="btn secondary" onClick={onClose}>
                        {t('cancel')}
                    </button>
                    <button
                        className={cx("btn", { "danger": isDanger, "primary": !isDanger })}
                        onClick={onConfirm}
                        autoFocus
                    >
                        {confirmLabel || t('confirm')}
                    </button>
                </div>
            </div>
        </div>
    );
};

