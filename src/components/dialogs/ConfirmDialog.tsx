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
    zIndex?: number;
    onFocus?: () => void;
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
    destination,
    zIndex,
    onFocus
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
        <div className="dialog-overlay" onClick={onClose} style={{ zIndex }}>
            <div
                ref={dragRef}
                className="dialog-window mini-dialog confirm-dialog"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="dialog-header" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <div className="dialog-title">
                        {isDanger && <AlertTriangle size={14} className="icon-danger" />}
                        <span>{title}</span>
                    </div>
                    <button className="dialog-close-btn btn-icon" onClick={onClose}>
                        <X size={16} />
                    </button>
                </div>
                {(sources || destination) && (
                    <div className="progress-details">
                        {sources && sources.length > 0 && (
                            <div>{t('source_dir' as any)}: {renderSources()}</div>
                        )}
                        {destination && (
                            <div>{t('target_dir' as any)}: {destination}</div>
                        )}
                    </div>
                )}
                
                {(message || subMessage) && (
                    <div className="dialog-content">
                        {message && <p className="dialog-message">{message}</p>}
                        {subMessage && <p className="dialog-sub-message">{subMessage}</p>}
                    </div>
                )}

                <div className="dialog-footer">
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

