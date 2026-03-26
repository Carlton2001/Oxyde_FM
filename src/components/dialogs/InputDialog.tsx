import React, { useState, useEffect, useRef } from 'react';
import { Edit2, FolderPlus, HelpCircle, X } from 'lucide-react';
import { TFunc } from '../../i18n';
import { useDraggable } from '../../hooks/useDraggable';
import '../../styles/components/Dialogs.css';

interface InputDialogProps {
    isOpen: boolean;
    title: string;
    label?: string;
    onClose: () => void;
    onSubmit: (value: string) => void;
    t: TFunc;
    initialValue?: string;
    placeholder?: string;
    confirmLabel?: string;
    icon?: 'rename' | 'new_folder' | 'default';
    zIndex?: number;
    onFocus?: () => void;
}

export const InputDialog: React.FC<InputDialogProps> = ({
    isOpen,
    title,
    label,
    onClose,
    onSubmit,
    t,
    initialValue = '',
    placeholder = '',
    confirmLabel,
    icon = 'default',
    zIndex,
    onFocus
}) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ 
        initialPosition: { x: 0, y: 0 }, 
        dragRef 
    });
    const [value, setValue] = useState(initialValue);
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (isOpen) {
            setValue(initialValue);
            setTimeout(() => {
                if (inputRef.current) {
                    inputRef.current.focus();
                    const dotIndex = initialValue.lastIndexOf('.');
                    if (dotIndex > 0 && icon === 'rename') {
                        inputRef.current.setSelectionRange(0, dotIndex);
                    } else {
                        inputRef.current.select();
                    }
                }
            }, 50);
        }
    }, [isOpen, initialValue]);

    const handleSubmit = (e?: React.FormEvent) => {
        e?.preventDefault();
        if (value.trim()) {
            onSubmit(value.trim());
        }
    };

    const getIcon = () => {
        switch (icon) {
            case 'new_folder': return <FolderPlus size={16} />;
            case 'rename': return <Edit2 size={16} />;
            default: return <HelpCircle size={16} />;
        }
    };

    if (!isOpen) return null;

    return (
        <div className="dialog-overlay" onClick={onClose} style={{ zIndex }}>
            <div
                ref={dragRef}
                className="dialog-window mini-dialog"
                onClick={(e) => e.stopPropagation()}
                style={{
                    transform: `translate(${position.x}px, ${position.y}px)`,
                    transition: 'none'
                }}
            >
                <div className="dialog-header" onMouseDown={(e) => { handleMouseDown(e); onFocus?.(); }}>
                    <div className="dialog-title">
                        {getIcon()}
                        <span>{title}</span>
                    </div>
                    <button className="dialog-close-btn btn-icon" onClick={onClose}>
                        <X size={16} />
                    </button>
                </div>

                <div className="dialog-content">
                    <form onSubmit={handleSubmit}>
                        {label && (
                            <div className="dialog-prompt-label">
                                {label}
                            </div>
                        )}
                        <input
                            ref={inputRef}
                            type="text"
                            className="input-field"
                            value={value}
                            onChange={(e) => setValue(e.target.value)}
                            placeholder={placeholder}
                        />
                    </form>
                </div>

                <div className="dialog-footer">
                    <button type="button" className="btn secondary" onClick={onClose}>
                        {t('cancel' as any)}
                    </button>
                    <button
                        type="button"
                        className="btn primary"
                        disabled={!value.trim()}
                        onClick={() => handleSubmit()}
                    >
                        {confirmLabel || t('create' as any)}
                    </button>
                </div>
            </div>
        </div>
    );
};

