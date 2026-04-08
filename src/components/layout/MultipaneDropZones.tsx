import React from 'react';
import cx from 'classnames';
import { PanelId } from '../../types';
import { useTabs } from '../../context/TabsContext';

interface MultipaneDropZonesProps {
    panelId: PanelId;
    draggedTab: { id: string, panelId: PanelId } | null;
}

export const MultipaneDropZones: React.FC<MultipaneDropZonesProps> = ({ panelId, draggedTab }) => {
    const [activeZone, setActiveZone] = React.useState<'top' | 'bottom' | 'left' | 'right' | null>(null);
    const { setActiveDropZone } = useTabs();

    if (!draggedTab) return null;

    const handleMouseEnter = (side: 'top' | 'bottom' | 'left' | 'right') => {
        setActiveZone(side);
        setActiveDropZone({ panelId, side });
    };

    const handleMouseLeave = () => {
        setActiveZone(null);
        setActiveDropZone(null);
    };

    return (
        <div className="multipane-drop-zones" data-panel-id={panelId}>
            {(['top', 'bottom', 'left', 'right'] as const).map(side => (
                <div
                    key={side}
                    className={cx("drop-zone", `drop-zone-${side}`, { active: activeZone === side })}
                    onMouseEnter={() => handleMouseEnter(side)}
                    onMouseLeave={handleMouseLeave}
                />
            ))}
        </div>
    );
};
