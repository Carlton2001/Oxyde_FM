import React from 'react';
import { formatSize } from '../../utils/format';
import './DiskUsageChart.css';

interface DiskUsageChartProps {
    total: number;
    free: number;
    size?: number;
    showText?: boolean;
    t?: any;
}

export const DiskUsageChart: React.FC<DiskUsageChartProps> = ({ total, free, size = 60, showText = true, t }) => {
    const used = total - free;
    const percent = Math.round((used / total) * 100);
    const radius = 16;
    const circumference = 2 * Math.PI * radius;
    const offset = circumference - (percent / 100) * circumference;

    const isNearFull = (free / total) < 0.1;
    const isDanger = (free / total) < 0.05;
    const strokeColor = isDanger ? '#ef4444' : isNearFull ? '#f59e0b' : 'var(--accent-color, #0078d7)';

    return (
        <div className="disk-usage-chart-container">
            <svg width={size} height={size} viewBox="0 0 40 40">
                {/* Background circle */}
                <circle
                    cx="20" cy="20" r="16"
                    fill="none"
                    stroke="currentColor"
                    opacity="0.15"
                    strokeWidth="6"
                />
                {/* Progress circle */}
                <circle
                    cx="20" cy="20" r="16"
                    fill="none"
                    stroke={strokeColor}
                    strokeWidth="6"
                    strokeDasharray={circumference}
                    strokeDashoffset={offset}
                    strokeLinecap="butt"
                    transform="rotate(-90 20 20)"
                />
                {/* Text in center */}
                <text
                    x="20"
                    y="20"
                    textAnchor="middle"
                    style={{ dominantBaseline: 'middle' }}
                    fontSize="8"
                    fill="currentColor"
                    fontWeight="bold"
                >
                    {percent}%
                </text>
            </svg>
            {showText && (
                <div className="disk-usage-chart-text">
                    {formatSize(used, 1, t)} / {formatSize(total, 1, t)}
                </div>
            )}
        </div>
    );
};
