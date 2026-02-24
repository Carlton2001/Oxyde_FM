/**
 * RenameInput — Shared inline rename input for file items
 * 
 * Used by both DetailsRow and GridCell to avoid duplicating the rename logic.
 */

import React from 'react';

interface RenameInputProps {
    renameText: string;
    onRenameTextChange: (text: string) => void;
    onRenameCommit: () => void;
    onRenameCancel: () => void;
    className?: string;
}

export const RenameInput: React.FC<RenameInputProps> = React.memo(({
    renameText, onRenameTextChange, onRenameCommit, onRenameCancel, className
}) => (
    <input
        type="text"
        className={className || "rename-input"}
        value={renameText}
        onChange={(e) => onRenameTextChange(e.target.value)}
        onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === 'Enter') onRenameCommit();
            if (e.key === 'Escape') onRenameCancel();
        }}
        onBlur={onRenameCommit}
        autoFocus
        onClick={(e) => e.stopPropagation()}
        onDoubleClick={(e) => e.stopPropagation()}
        onFocus={(e) => {
            const dotIndex = renameText.lastIndexOf('.');
            if (dotIndex > 0) {
                e.target.setSelectionRange(0, dotIndex);
            } else {
                e.target.select();
            }
        }}
    />
));

