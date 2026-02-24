import React, { useEffect, useRef } from 'react';

interface ToggleProps {
    checked: boolean;
    indeterminate?: boolean;
    onChange: (checked: boolean) => void;
    label?: string;
    className?: string;
    disabled?: boolean;
}

export const Toggle: React.FC<ToggleProps> = ({ checked, indeterminate = false, onChange, label, className = '', disabled = false }) => {
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (inputRef.current) {
            inputRef.current.indeterminate = indeterminate;
        }
    }, [indeterminate]);

    return (
        <label
            className={`toggle-switch ${className} ${disabled ? 'disabled' : ''}`}
            onClick={(e) => e.stopPropagation()}
        >
            <input
                ref={inputRef}
                type="checkbox"
                className="toggle-input"
                checked={checked}
                onChange={(e) => !disabled && onChange(e.target.checked)}
                disabled={disabled}
            />
            <span className="toggle-slider" />
            {label && <span className="toggle-label">{label}</span>}
        </label>
    );
};

