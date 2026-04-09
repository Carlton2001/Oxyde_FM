import React from 'react';
import cx from 'classnames';
import { Check } from 'lucide-react';

interface FilterMenuCheckboxProps {
    checked: boolean;
    label: string;
    count?: number;
    onClick: () => void;
}

export const FilterMenuCheckbox: React.FC<FilterMenuCheckboxProps> = ({ checked, label, count, onClick }) => (
    <div className={cx('context-menu-item', 'filter-menu-item')} onClick={onClick}>
        <div className={cx('filter-checkbox', { checked })}>
            {checked && <Check size={10} strokeWidth={3} />}
        </div>
        <span className="filter-label">{label}</span>
        {count !== undefined && <span className="filter-count">{count}</span>}
    </div>
);
