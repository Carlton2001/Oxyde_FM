import React, { useState, useEffect, useRef } from 'react';
import { Pause, Play, Square, Zap, Shield } from 'lucide-react';
import { useDraggable } from '../../hooks/useDraggable';
import { formatSize } from '../../utils/format';

export interface ProgressState {
    visible: boolean;
    message: string;
    cancellable?: boolean;
    cancelling?: boolean;
    current?: number;
    total?: number;
    filename?: string;
    paused?: boolean;
    canPause?: boolean;
    sources?: string[];
    destination?: string;
    speed?: number;
    processedFiles?: number;
    totalFiles?: number;
    turbo?: boolean;
}

interface ProgressOverlayProps {
    progress: ProgressState | null;
    onCancel?: () => void;
    onPause?: () => void;
    onResume?: () => void;
    onToggleTurbo?: (enabled: boolean) => void;
    t: (key: string) => string;
}

export const ProgressOverlay: React.FC<ProgressOverlayProps> = ({ progress, t, onCancel, onPause, onResume, onToggleTurbo }) => {
    const dragRef = useRef<HTMLDivElement>(null);
    const { position, handleMouseDown } = useDraggable({ initialPosition: { x: 0, y: 0 }, dragRef });
    const [speedHistory, setSpeedHistory] = useState<number[]>([]);
    const maxHistoryPoints = 40;

    useEffect(() => {
        if (progress?.visible && progress.speed !== undefined) {
            setSpeedHistory(prev => {
                const newHistory = [...prev, progress.speed || 0];
                if (newHistory.length > maxHistoryPoints) {
                    return newHistory.slice(newHistory.length - maxHistoryPoints);
                }
                return newHistory;
            });
        } else if (!progress?.visible) {
            setSpeedHistory([]);
        }
    }, [progress?.speed, progress?.visible]);

    if (!progress || !progress.visible) return null;

    const renderSources = () => {
        if (!progress.sources || progress.sources.length === 0) return null;
        const count = progress.sources.length;
        if (count === 1) return progress.sources[0];
        return `${progress.sources[0]} (+${count - 1})`;
    };

    const renderSizes = () => {
        if (progress.total === undefined || progress.total === 0) return null;
        return `${formatSize(progress.current || 0, 1, t as any)} / ${formatSize(progress.total, 1, t as any)}`;
    };

    const percent = progress.total && progress.total > 0 ? Math.round(((progress.current || 0) / progress.total) * 100) : 0;
    const isCompleted = percent === 100 || (progress as any).status === 'Completed';

    const renderRemainingTime = () => {
        if (isCompleted) return `0${t('seconds_short')}`;
        if (!progress.total || !progress.current) return null;
        if (!progress.speed || progress.speed <= 0) return t('waiting' as any);

        const remainingBytes = progress.total - progress.current;
        if (remainingBytes <= 0) return `0${t('seconds_short')}`;

        const seconds = remainingBytes / progress.speed;
        if (seconds < 1) return t('less_than_second');
        if (seconds < 60) return `${Math.round(seconds)}${t('seconds_short')}`;

        const minutes = Math.floor(seconds / 60);
        const remSeconds = Math.round(seconds % 60);

        if (minutes < 60) {
            return `${minutes}${t('minutes_short')} ${remSeconds}${t('seconds_short')}`;
        }

        const hours = Math.floor(minutes / 60);
        const remMinutes = minutes % 60;
        return `${hours}${t('hours_short')} ${remMinutes}${t('minutes_short')}`;
    };

    const renderRemainingTimeRow = () => {
        if (!progress.total || progress.total <= 0) return null;

        return (
            <div className="flex-row justify-between text-sm pt-1 mt-1">
                <span>{t('time_remaining' as any)}:</span>
                <span className="text-accent font-medium">
                    {renderRemainingTime()}
                </span>
            </div>
        );
    };

    const renderSpeedText = () => {
        if (progress.speed === undefined || progress.speed < 0) return null;
        return `${formatSize(progress.speed, 1, t as any)}/s`;
    };

    const renderBackgroundGraph = () => {
        if (speedHistory.length < 2) return null;

        const width = 300;
        const height = 140; // Slightly taller to accommodate bar
        const maxSpeed = Math.max(...speedHistory, 1024 * 1024);

        const points = speedHistory.map((speed, i) => {
            const x = (i / (maxHistoryPoints - 1)) * width;
            const y = height - (speed / maxSpeed) * (height * 0.5) - 20;
            return { x, y };
        });

        const d = `M ${points.map(p => `${p.x},${p.y}`).join(' L')}`;
        const areaD = `${d} L ${points[points.length - 1].x},${height} L 0,${height} Z`;

        return (
            <div style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                bottom: 0,
                zIndex: 0,
                opacity: 0.2,
                pointerEvents: 'none',
                overflow: 'hidden',
                borderRadius: 'inherit'
            }}>
                <svg width="100%" height="100%" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" style={{ display: 'block' }}>
                    <defs>
                        <linearGradient id="speedGradientBg" x1="0" y1="0" x2="0" y2="1">
                            <stop offset="0%" stopColor="var(--accent-color)" stopOpacity="0.4" />
                            <stop offset="100%" stopColor="var(--accent-color)" stopOpacity="0" />
                        </linearGradient>
                    </defs>
                    <path d={areaD} fill="url(#speedGradientBg)" />
                    <path
                        d={d}
                        fill="none"
                        stroke="var(--accent-color)"
                        strokeWidth="1.5"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        style={{ transition: 'd 0.3s ease-out' }}
                    />
                </svg>
            </div>
        );
    };

    return (
        <div className="progress-overlay" style={{
            transform: `translate(${position.x}px, ${position.y}px)`,
            transition: 'none' // Disable transition during drag or generally for better responsiveness
        }}>
            <div ref={dragRef} className="progress-content-minimal" style={{
                minWidth: '20rem',
                position: 'relative',
                overflow: 'hidden',
                paddingBottom: '0.5rem' // Bottom padding for breathing room
            }}>
                <div
                    className="progress-drag-handle flex-row justify-between items-center"
                    onMouseDown={handleMouseDown}
                >
                    <span className="text-ellipsis" style={{ flex: 1 }}>
                        {progress.cancelling ? t('cancelling' as any) :
                            isCompleted ? t('completed') :
                                progress.message}
                    </span>

                    <div className="progress-controls-row" onMouseDown={(e) => e.stopPropagation()}>
                        {!isCompleted && (
                            <button
                                onClick={() => onToggleTurbo?.(!progress.turbo)}
                                className={`progress-btn-control turbo-toggle-btn ${progress.turbo ? 'turbo-active' : ''}`}
                                style={{
                                    color: progress.turbo ? '#fbbf24' : 'var(--text-muted)',
                                }}
                                data-tooltip={progress.turbo ? t('mode_turbo' as any) : t('mode_discret' as any)}
                            >
                                {progress.turbo ? <Zap size={16} fill="#fbbf24" strokeWidth={1} /> : <Shield size={16} strokeWidth={1.5} />}
                            </button>
                        )}

                        {progress.canPause && !isCompleted && (
                            <button
                                onClick={progress.paused ? onResume : onPause}
                                className={`progress-btn-control pause-btn ${progress.paused ? 'paused' : ''}`}
                                data-tooltip={progress.paused ? t('resume') : t('pause' as any)}
                            >
                                {progress.paused ? (
                                    <>
                                        <Play size={14} fill="currentColor" />
                                        <span>{t('resume')}</span>
                                    </>
                                ) : (
                                    <Pause size={16} fill="currentColor" />
                                )}
                            </button>
                        )}

                        {progress.cancellable && !isCompleted && (
                            <button
                                onClick={onCancel}
                                className="progress-btn-control stop-btn"
                                data-tooltip={t('cancel' as any)}
                            >
                                <Square size={16} fill="currentColor" />
                            </button>
                        )}
                    </div>
                </div>
                {renderBackgroundGraph()}

                <div style={{ position: 'relative', zIndex: 1, width: '100%', marginTop: '0.25rem' }}>
                    {/* Operation name now in title bar */}

                    <div className="progress-details flex-col gap-xs text-sm w-full">
                        {progress.sources && progress.sources.length > 0 && (
                            <div>{t('source_dir' as any)}: {renderSources()}</div>
                        )}
                        {progress.destination && (
                            <div>{t('target_dir' as any)}: {progress.destination}</div>
                        )}

                        <div className="flex-row justify-between items-baseline mt-2">
                            {progress.totalFiles !== undefined && progress.totalFiles > 0 && (
                                <span className="tabular-nums">
                                    {progress.processedFiles || 0} / {progress.totalFiles} {progress.totalFiles > 1 ? t('items') : t('item')}
                                </span>
                            )}
                            <span className="text-accent font-bold text-sm">
                                {percent}%
                            </span>
                        </div>

                        <div className="flex-row justify-between items-baseline">
                            <span className="tabular-nums">
                                {renderSizes()}
                            </span>
                            <span className="font-medium">
                                {renderSpeedText()}
                            </span>
                        </div>

                        {renderRemainingTimeRow()}
                    </div>
                </div>

                {/* Progress Bar at the absolute bottom */}
                <div style={{
                    position: 'absolute',
                    bottom: 0,
                    left: 0,
                    right: 0,
                    height: '1px',
                    background: 'rgba(var(--accent-color-rgb), 0.1)',
                    zIndex: 2
                }}>
                    <div style={{
                        width: `${percent}%`,
                        height: '100%',
                        background: 'var(--accent-color)',
                        transition: 'width 0.4s ease-out'
                    }} />
                </div>
            </div>
        </div>
    );
};

