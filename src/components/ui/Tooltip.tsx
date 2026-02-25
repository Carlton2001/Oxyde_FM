import React, { useState, useEffect, useRef } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { readTextFileLines, readFile } from '@tauri-apps/plugin-fs';
import { getPdfThumbnail, PDF_THUMBNAIL_CACHE } from '../../utils/pdf';
import { PREVIEWABLE_EXTENSIONS, PREVIEWABLE_VIDEO_EXTENSIONS, PREVIEWABLE_TEXT_EXTENSIONS, PREVIEWABLE_PDF_EXTENSIONS, PREVIEWABLE_OFFICE_EXTENSIONS } from '../../utils/fileIcons';
import { DiskUsageChart } from './DiskUsageChart';
import { useApp } from '../../context/AppContext';
import './Tooltip.css';

interface TooltipState {
    visible: boolean;
    content: string;
    x: number;
    y: number;
    multiline: boolean;
    mediaSrc: string | null;
    mediaType: 'image' | 'video' | 'text' | 'pdf' | null;
    textContent: string | null;
    diskStats: { total: number; free: number } | null;
}

interface TooltipProps {
    isShiftPressed?: boolean;
}

const TOOLTIP_OFFSET = 12;
const SHOW_DELAY = 500; // 0.5 second delay
const EDGE_PADDING = 50;
const MAX_TEXT_PREVIEW_LINES = 15;
const MAX_TOTAL_CHARS = 1500;

export const Tooltip: React.FC<TooltipProps> = ({ isShiftPressed }) => {
    const [tooltip, setTooltip] = useState<TooltipState>({
        visible: false,
        content: '',
        x: 0,
        y: 0,
        multiline: false,
        mediaSrc: null,
        mediaType: null,
        textContent: null,
        diskStats: null
    });
    const tooltipRef = useRef<HTMLDivElement>(null);
    const showTimeoutRef = useRef<number | null>(null);
    const currentTargetRef = useRef<HTMLElement | null>(null);
    const { showPreviews } = useApp();

    useEffect(() => {
        const handleMouseEnterTarget = (e: MouseEvent) => {
            // Do not show tooltips if a context menu is open
            if (document.querySelector('.context-menu')) {
                if (showTimeoutRef.current) {
                    clearTimeout(showTimeoutRef.current);
                    showTimeoutRef.current = null;
                }
                setTooltip(prev => ({ ...prev, visible: false }));
                return;
            }

            const target = (e.target as HTMLElement).closest('[data-tooltip]') as HTMLElement | null;

            if (target && target !== currentTargetRef.current) {
                // Clear any existing timeout
                if (showTimeoutRef.current) {
                    clearTimeout(showTimeoutRef.current);
                }

                // Hide current tooltip immediately
                setTooltip(prev => ({ ...prev, visible: false, mediaSrc: null, mediaType: null, textContent: null, diskStats: null }));
                currentTargetRef.current = target;

                const content = target.getAttribute('data-tooltip');
                const multiline = target.hasAttribute('data-tooltip-multiline');
                const path = target.getAttribute('data-tooltip-image-path');
                const totalAttr = target.getAttribute('data-tooltip-total');
                const freeAttr = target.getAttribute('data-tooltip-free');

                if (content) {
                    // Capture position at hover start
                    const x = e.clientX + TOOLTIP_OFFSET;
                    const y = e.clientY + TOOLTIP_OFFSET;

                    // Start delay timer
                    showTimeoutRef.current = window.setTimeout(async () => {
                        if (document.querySelector('.context-menu')) {
                            showTimeoutRef.current = null;
                            return;
                        }

                        let mediaSrc = null;
                        let mediaType: 'image' | 'video' | 'text' | 'pdf' | null = null;
                        let textContent = null;

                        if (path && showPreviews) {
                            const ext = path.split('.').pop()?.toLowerCase() || '';
                            if (PREVIEWABLE_EXTENSIONS.includes(ext)) {
                                mediaSrc = convertFileSrc(path);
                                mediaType = 'image';
                            } else if (PREVIEWABLE_VIDEO_EXTENSIONS.includes(ext)) {
                                mediaSrc = convertFileSrc(path);
                                mediaType = 'video';
                            } else if (PREVIEWABLE_TEXT_EXTENSIONS.includes(ext)) {
                                try {
                                    // Optimized approach: read line by line until limits are hit
                                    const lines = await readTextFileLines(path);
                                    let preview = '';
                                    let lineCount = 0;
                                    let charCount = 0;

                                    for await (const line of lines) {
                                        preview += line + '\n';
                                        lineCount++;
                                        charCount += line.length;

                                        if (lineCount >= MAX_TEXT_PREVIEW_LINES || charCount >= MAX_TOTAL_CHARS) {
                                            preview += '...';
                                            break;
                                        }
                                    }

                                    if (preview.trim()) {
                                        textContent = preview;
                                        mediaType = 'text';
                                    }
                                } catch (err) {
                                    console.error('Failed to read text file for preview', err);
                                }
                            } else if (PREVIEWABLE_PDF_EXTENSIONS.includes(ext)) {
                                try {
                                    if (PDF_THUMBNAIL_CACHE.has(path)) {
                                        mediaSrc = PDF_THUMBNAIL_CACHE.get(path)!;
                                    } else {
                                        const fileData = await readFile(path);
                                        const thumbnail = await getPdfThumbnail(fileData.buffer, 1.0);
                                        PDF_THUMBNAIL_CACHE.set(path, thumbnail);
                                        mediaSrc = thumbnail;
                                    }
                                    mediaType = 'pdf';
                                } catch (err) {
                                    console.error('Failed to generate PDF thumbnail', err);
                                }
                            } else if (PREVIEWABLE_OFFICE_EXTENSIONS.includes(ext)) {
                                try {
                                    const cachedPath = await invoke<string>('get_office_thumbnail', { path });
                                    mediaSrc = convertFileSrc(cachedPath);
                                    // Treat as pdf to get the nice drop-shadow / presentation in tooltip
                                    mediaType = 'pdf';
                                } catch (err) {
                                    // Soft fail, no thumbnail available in the archive, try text preview
                                    try {
                                        const textPreview = await invoke<string>('get_office_text_preview', { path });
                                        if (textPreview) {
                                            textContent = textPreview;
                                            mediaType = 'text';
                                        }
                                    } catch (textErr) {
                                        console.debug('No embedded thumbnail or text found for office document', err, textErr);
                                    }
                                }
                            }
                        }

                        let diskStats = null;
                        if (totalAttr && freeAttr) {
                            diskStats = { total: parseInt(totalAttr, 10), free: parseInt(freeAttr, 10) };
                        }

                        setTooltip({ visible: true, content, x, y, multiline, mediaSrc, mediaType, textContent, diskStats });
                        showTimeoutRef.current = null;
                    }, SHOW_DELAY);
                }
            } else if (!target) {
                // Left all tooltip zones
                if (showTimeoutRef.current) {
                    clearTimeout(showTimeoutRef.current);
                    showTimeoutRef.current = null;
                }
                if (currentTargetRef.current) {
                    currentTargetRef.current = null;
                    setTooltip(prev => ({ ...prev, visible: false, mediaSrc: null, mediaType: null, textContent: null, diskStats: null }));
                }
            }
        };

        const handleContextMenu = () => {
            if (showTimeoutRef.current) {
                clearTimeout(showTimeoutRef.current);
                showTimeoutRef.current = null;
            }
            setTooltip(prev => ({ ...prev, visible: false }));
        };

        document.addEventListener('mouseover', handleMouseEnterTarget);
        window.addEventListener('contextmenu', handleContextMenu, true);

        return () => {
            document.removeEventListener('mouseover', handleMouseEnterTarget);
            window.removeEventListener('contextmenu', handleContextMenu, true);
            if (showTimeoutRef.current) clearTimeout(showTimeoutRef.current);
        };
    }, [showPreviews]);

    // React immediately to shift changes if tooltip is visible
    useEffect(() => {
        if (tooltip.visible && currentTargetRef.current) {
            const content = currentTargetRef.current.getAttribute('data-tooltip');
            if (content && content !== tooltip.content) {
                setTooltip(prev => ({ ...prev, content }));
            }
        }
    }, [isShiftPressed, tooltip.visible]);

    const [layoutVersion, setLayoutVersion] = useState(0);

    // Position adjustment effect
    useEffect(() => {
        if (tooltip.visible && tooltipRef.current) {
            const rect = tooltipRef.current.getBoundingClientRect();
            let newX = tooltip.x;
            let newY = tooltip.y;
            let needsUpdate = false;

            // Check right overflow
            if (rect.right > window.innerWidth - EDGE_PADDING) {
                newX = window.innerWidth - rect.width - EDGE_PADDING;
                newX = Math.max(EDGE_PADDING, newX);
                needsUpdate = true;
            }

            // Check bottom overflow
            if (rect.bottom > window.innerHeight - EDGE_PADDING) {
                newY = window.innerHeight - rect.height - EDGE_PADDING;
                newY = Math.max(EDGE_PADDING, newY);
                needsUpdate = true;
            }

            if (needsUpdate) {
                // Ensure we don't push it off the left/top either
                newX = Math.max(EDGE_PADDING, newX);
                newY = Math.max(EDGE_PADDING, newY);
                setTooltip(prev => ({ ...prev, x: newX, y: newY }));
            }
        }
    }, [tooltip.visible, tooltip.x, tooltip.y, tooltip.mediaSrc, tooltip.textContent, tooltip.content, layoutVersion]);

    if (!tooltip.visible || !tooltip.content) return null;

    return (
        <div
            ref={tooltipRef}
            className={`tooltip-portal ${tooltip.multiline ? 'multiline' : ''}`}
            style={{ left: tooltip.x, top: tooltip.y }}
        >
            {tooltip.mediaType && (
                <div style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'flex-start',
                    justifyContent: 'flex-start',
                    marginBottom: '4px',
                    width: '100%'
                }}>
                    {(tooltip.mediaType === 'image' || tooltip.mediaType === 'pdf') && tooltip.mediaSrc && (
                        <img
                            src={tooltip.mediaSrc}
                            alt="Preview"
                            onLoad={() => setLayoutVersion(v => v + 1)}
                            onError={() => {
                                setTooltip(prev => ({ ...prev, mediaSrc: null, mediaType: null }));
                                setLayoutVersion(v => v + 1);
                            }}
                            style={{
                                maxWidth: '100%',
                                maxHeight: tooltip.mediaType === 'pdf' ? '18.75rem' : '12.5rem',
                                objectFit: 'contain',
                                display: 'block'
                            }}
                        />
                    )}
                    {tooltip.mediaType === 'video' && tooltip.mediaSrc && (
                        <video
                            src={tooltip.mediaSrc}
                            autoPlay
                            muted
                            loop
                            playsInline
                            onLoadedData={() => setLayoutVersion(v => v + 1)}
                            onError={() => {
                                setTooltip(prev => ({ ...prev, mediaSrc: null, mediaType: null }));
                                setLayoutVersion(v => v + 1);
                            }}
                            style={{
                                maxWidth: '100%',
                                maxHeight: '12.5rem',
                                display: 'block'
                            }}
                        />
                    )}
                    {tooltip.mediaType === 'text' && tooltip.textContent && (
                        <div className="tooltip-text-preview">
                            {tooltip.textContent}
                        </div>
                    )}
                </div>
            )}

            <div style={{ overflowWrap: 'anywhere', wordBreak: 'break-word' }}>
                {tooltip.multiline ? (
                    tooltip.content.split('\n').map((line, i) => (
                        <React.Fragment key={i}>
                            {i === 0 ? <strong>{line}</strong> : line}
                            {i < tooltip.content.split('\n').length - 1 && <br />}
                        </React.Fragment>
                    ))
                ) : (
                    tooltip.content
                )}
            </div>

            {tooltip.diskStats && (
                <div style={{ marginTop: '0.5rem' }}>
                    <DiskUsageChart
                        total={tooltip.diskStats.total}
                        free={tooltip.diskStats.free}
                    />
                </div>
            )}
        </div>
    );
};
