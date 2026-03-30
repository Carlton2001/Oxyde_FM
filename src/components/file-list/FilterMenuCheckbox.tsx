import React from 'react';
import cx from 'classnames';
import { Check } from 'lucide-react';

interface FilterMenuCheckboxProps {
    checked: boolean;
    label: string;
    onClick: () => void;
}

export const FilterMenuCheckbox: React.FC<FilterMenuCheckboxProps> = ({ checked, label, onClick }) => (
    <div className={cx('context-menu-item', 'filter-menu-item')} onClick={onClick}>
        <div className={cx('filter-checkbox', { checked })}>
            {checked && <Check size={10} strokeWidth={3} />}
        </div>
        <span>{label}</span>
    </div>
);
