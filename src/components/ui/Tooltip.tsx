import React, { useState, useEffect, useRef, useLayoutEffect } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { readTextFileLines, readFile } from "@tauri-apps/plugin-fs";
import { getPdfThumbnail, PDF_THUMBNAIL_CACHE } from "../../utils/pdf";
import {
  PREVIEWABLE_EXTENSIONS,
  PREVIEWABLE_VIDEO_EXTENSIONS,
  PREVIEWABLE_TEXT_EXTENSIONS,
  PREVIEWABLE_PDF_EXTENSIONS,
  PREVIEWABLE_OFFICE_EXTENSIONS,
} from "../../utils/fileIcons";
import { DiskUsageChart } from "./DiskUsageChart";
import { useApp } from "../../context/AppContext";
import "./Tooltip.css";

interface TooltipState {
  visible: boolean;
  content: string;
  multiline: boolean;
  mediaSrc: string | null;
  mediaType: "image" | "video" | "text" | "pdf" | null;
  textContent: string | null;
  diskStats: { total: number; free: number } | null;
}

interface TooltipProps {
  isShiftPressed?: boolean;
}

const TOOLTIP_OFFSET = 24;
const SHOW_DELAY = 750; // 0.75 second delay
const EDGE_PADDING = 50;
const MAX_TEXT_PREVIEW_LINES = 15;
const MAX_TOTAL_CHARS = 1500;
const BRIDGE_DELAY = 300; // Bridge gaps between tooltip zones

export const Tooltip: React.FC<TooltipProps> = ({ isShiftPressed }) => {
  const [tooltip, setTooltip] = useState<TooltipState>({
    visible: false,
    content: "",
    multiline: false,
    mediaSrc: null,
    mediaType: null,
    textContent: null,
    diskStats: null,
  });
  const tooltipRef = useRef<HTMLDivElement>(null);
  const showTimeoutRef = useRef<number | null>(null);
  const hideTimeoutRef = useRef<number | null>(null);
  const currentTargetRef = useRef<HTMLElement | null>(null);
  const mousePosRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  // Position is managed via ref + direct DOM — never via React state
  const positionRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const isTooltipVisibleRef = useRef(false);
  const { showPreviews, t } = useApp();

  useEffect(() => {
    isTooltipVisibleRef.current = tooltip.visible;
  }, [tooltip.visible]);

  const applyPosition = (x: number, y: number) => {
    if (!tooltipRef.current) {
      positionRef.current = { x, y };
      return;
    }

    const rect = tooltipRef.current.getBoundingClientRect();
    let finalX = x;
    let finalY = y;

    if (x + rect.width > window.innerWidth - EDGE_PADDING) {
      finalX = Math.max(
        EDGE_PADDING,
        window.innerWidth - rect.width - EDGE_PADDING,
      );
    }
    if (y + rect.height > window.innerHeight - EDGE_PADDING) {
      finalY = Math.max(
        EDGE_PADDING,
        window.innerHeight - rect.height - EDGE_PADDING,
      );
    }

    positionRef.current = { x: finalX, y: finalY };
    tooltipRef.current.style.left = `${finalX}px`;
    tooltipRef.current.style.top = `${finalY}px`;
  };

  useEffect(() => {
    const handleMouseEnterTarget = (e: MouseEvent) => {
      // Do not show tooltips if a context menu is open
      if (document.querySelector(".context-menu")) {
        if (showTimeoutRef.current) {
          clearTimeout(showTimeoutRef.current);
          showTimeoutRef.current = null;
        }
        setTooltip((prev) => ({ ...prev, visible: false }));
        return;
      }

      const target = (e.target as HTMLElement).closest(
        "[data-tooltip]",
      ) as HTMLElement | null;

      if (target && target !== currentTargetRef.current) {
        // Clear any existing timeouts
        if (showTimeoutRef.current) clearTimeout(showTimeoutRef.current);
        if (hideTimeoutRef.current) clearTimeout(hideTimeoutRef.current);
        hideTimeoutRef.current = null;

        const wasVisible = isTooltipVisibleRef.current;
        currentTargetRef.current = target;

        const content = target.getAttribute("data-tooltip");
        const multiline = target.hasAttribute("data-tooltip-multiline");
        const path = target.getAttribute("data-tooltip-image-path");
        const totalAttr = target.getAttribute("data-tooltip-total");
        const freeAttr = target.getAttribute("data-tooltip-free");

        if (content) {
          const currentTarget = target;
          const updateTooltip = async () => {
            if (document.querySelector(".context-menu")) return;
            if (currentTarget !== currentTargetRef.current) return;

            let mediaSrc = null;
            let mediaType: "image" | "video" | "text" | "pdf" | null = null;
            let textContent = null;

            if (path && showPreviews) {
              const ext = path.split(".").pop()?.toLowerCase() || "";
              if (PREVIEWABLE_EXTENSIONS.includes(ext)) {
                mediaSrc = convertFileSrc(path);
                mediaType = "image";
              } else if (PREVIEWABLE_VIDEO_EXTENSIONS.includes(ext)) {
                mediaSrc = convertFileSrc(path);
                mediaType = "video";
              } else if (PREVIEWABLE_TEXT_EXTENSIONS.includes(ext)) {
                try {
                  const lines = await readTextFileLines(path);
                  let preview = "";
                  let lineCount = 0;
                  let charCount = 0;
                  for await (const line of lines) {
                    preview += line + "\n";
                    lineCount++;
                    charCount += line.length;
                    if (
                      lineCount >= MAX_TEXT_PREVIEW_LINES ||
                      charCount >= MAX_TOTAL_CHARS
                    ) {
                      preview += "...";
                      break;
                    }
                  }
                  if (preview.trim()) {
                    textContent = preview;
                    mediaType = "text";
                  }
                } catch (err) {
                  console.error("Failed to read text file for preview", err);
                }
              } else if (PREVIEWABLE_PDF_EXTENSIONS.includes(ext)) {
                try {
                  if (PDF_THUMBNAIL_CACHE.has(path)) {
                    mediaSrc = PDF_THUMBNAIL_CACHE.get(path)!;
                  } else {
                    const fileData = await readFile(path);
                    const thumbnail = await getPdfThumbnail(
                      fileData.buffer,
                      1.0,
                    );
                    PDF_THUMBNAIL_CACHE.set(path, thumbnail);
                    mediaSrc = thumbnail;
                  }
                  mediaType = "pdf";
                } catch (err) {
                  console.error("Failed to generate PDF thumbnail", err);
                }
              } else if (PREVIEWABLE_OFFICE_EXTENSIONS.includes(ext)) {
                try {
                  const cachedPath = await invoke<string>(
                    "get_office_thumbnail",
                    { path },
                  );
                  mediaSrc = convertFileSrc(cachedPath);
                  mediaType = "pdf";
                } catch (err) {
                  try {
                    const textPreview = await invoke<string>(
                      "get_office_text_preview",
                      { path },
                    );
                    if (textPreview) {
                      textContent = textPreview;
                      mediaType = "text";
                    }
                  } catch (textErr) {
                    console.debug(
                      "No embedded thumbnail or text found for office document",
                      err,
                      textErr,
                    );
                  }
                }
              }
            }

            if (currentTarget !== currentTargetRef.current) return;

            let diskStats = null;
            if (totalAttr && freeAttr) {
              diskStats = {
                total: parseInt(totalAttr, 10),
                free: parseInt(freeAttr, 10),
              };
            }

            // Set initial position from current cursor
            positionRef.current = {
              x: mousePosRef.current.x + TOOLTIP_OFFSET,
              y: mousePosRef.current.y + TOOLTIP_OFFSET,
            };
            setTooltip({
              visible: true,
              content,
              multiline,
              mediaSrc,
              mediaType,
              textContent,
              diskStats,
            });
            showTimeoutRef.current = null;
          };

          if (wasVisible) {
            // Responsive update: set text instantly, then let updateTooltip fill in the rest
            setTooltip((prev) => ({
              ...prev,
              visible: true,
              content,
              multiline,
              mediaSrc: null,
              mediaType: null,
              textContent: null,
              diskStats: null,
            }));
            updateTooltip();
          } else {
            showTimeoutRef.current = window.setTimeout(
              updateTooltip,
              SHOW_DELAY,
            );
          }
        }
      } else if (!target) {
        // Left all tooltip zones — determine if we hide instantly or start bridge timer
        if (showTimeoutRef.current) {
          clearTimeout(showTimeoutRef.current);
          showTimeoutRef.current = null;
        }

        if (currentTargetRef.current && isTooltipVisibleRef.current) {
          const containerSelector =
            ".file-list, .sidebar, .top-bar, .status-bar, .dialog-content, .tabs-container, .header, .hamburger-menu, .breadcrumb-menu, .context-menu-container";
          const prevContainer =
            currentTargetRef.current.closest(containerSelector);
          const currentContainer = (e.target as HTMLElement).closest(
            containerSelector,
          );

          // If we left the container entirely, hide immediately
          if (!currentContainer || currentContainer !== prevContainer) {
            currentTargetRef.current = null;
            setTooltip((prev) => ({
              ...prev,
              visible: false,
              mediaSrc: null,
              mediaType: null,
              textContent: null,
              diskStats: null,
            }));
            if (hideTimeoutRef.current) {
              clearTimeout(hideTimeoutRef.current);
              hideTimeoutRef.current = null;
            }
          } else {
            // Still in the same container — start bridge timer for gaps
            if (!hideTimeoutRef.current) {
              hideTimeoutRef.current = window.setTimeout(() => {
                currentTargetRef.current = null;
                setTooltip((prev) => ({
                  ...prev,
                  visible: false,
                  mediaSrc: null,
                  mediaType: null,
                  textContent: null,
                  diskStats: null,
                }));
                hideTimeoutRef.current = null;
              }, BRIDGE_DELAY);
            }
          }
        } else if (!isTooltipVisibleRef.current) {
          currentTargetRef.current = null;
        }
      }
    };

    const handleMouseMove = (e: MouseEvent) => {
      mousePosRef.current = { x: e.clientX, y: e.clientY };

      if (tooltipRef.current) {
        applyPosition(e.clientX + TOOLTIP_OFFSET, e.clientY + TOOLTIP_OFFSET);
      }
    };

    const handleContextMenu = () => {
      if (showTimeoutRef.current) {
        clearTimeout(showTimeoutRef.current);
        showTimeoutRef.current = null;
      }
      if (hideTimeoutRef.current) {
        clearTimeout(hideTimeoutRef.current);
        hideTimeoutRef.current = null;
      }
      setTooltip((prev) => ({ ...prev, visible: false }));
    };

    document.addEventListener("mouseover", handleMouseEnterTarget);
    document.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("contextmenu", handleContextMenu, true);

    return () => {
      document.removeEventListener("mouseover", handleMouseEnterTarget);
      document.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("contextmenu", handleContextMenu, true);
      if (showTimeoutRef.current) clearTimeout(showTimeoutRef.current);
      if (hideTimeoutRef.current) clearTimeout(hideTimeoutRef.current);
    };
  }, [showPreviews]);

  // React immediately to shift changes if tooltip is visible
  useEffect(() => {
    if (tooltip.visible && currentTargetRef.current) {
      const content = currentTargetRef.current.getAttribute("data-tooltip");
      if (content && content !== tooltip.content) {
        setTooltip((prev) => ({ ...prev, content }));
      }
    }
  }, [isShiftPressed, tooltip.visible]);

  const [layoutVersion, setLayoutVersion] = useState(0);

  // After any render, re-apply position from ref so React never overrides it
  // Also handles edge detection (overflow correction)
  useLayoutEffect(() => {
    if (!tooltip.visible || !tooltipRef.current) return;

    // Apply current position
    tooltipRef.current.style.left = `${positionRef.current.x}px`;
    tooltipRef.current.style.top = `${positionRef.current.y}px`;

    // Edge overflow correction — update DOM directly, no state update
    const rect = tooltipRef.current.getBoundingClientRect();
    let x = positionRef.current.x;
    let y = positionRef.current.y;

    if (rect.right > window.innerWidth - EDGE_PADDING) {
      x = Math.max(EDGE_PADDING, window.innerWidth - rect.width - EDGE_PADDING);
    }
    if (rect.bottom > window.innerHeight - EDGE_PADDING) {
      y = Math.max(
        EDGE_PADDING,
        window.innerHeight - rect.height - EDGE_PADDING,
      );
    }

    if (x !== positionRef.current.x || y !== positionRef.current.y) {
      positionRef.current = { x, y };
      tooltipRef.current.style.left = `${x}px`;
      tooltipRef.current.style.top = `${y}px`;
    }
  }, [
    tooltip.visible,
    tooltip.mediaSrc,
    tooltip.textContent,
    tooltip.content,
    tooltip.diskStats,
    layoutVersion,
  ]);

  if (!tooltip.visible || !tooltip.content) return null;

  return (
    <div
      ref={tooltipRef}
      className={`tooltip-portal ${tooltip.multiline ? "multiline" : ""}`}
    >
      {tooltip.mediaType && (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "flex-start",
            justifyContent: "flex-start",
            marginBottom: "4px",
            width: "100%",
          }}
        >
          {(tooltip.mediaType === "image" || tooltip.mediaType === "pdf") &&
            tooltip.mediaSrc && (
              <img
                src={tooltip.mediaSrc}
                alt="Preview"
                onLoad={() => setLayoutVersion((v) => v + 1)}
                onError={() => {
                  setTooltip((prev) => ({
                    ...prev,
                    mediaSrc: null,
                    mediaType: null,
                  }));
                  setLayoutVersion((v) => v + 1);
                }}
                style={{
                  maxWidth: "100%",
                  maxHeight:
                    tooltip.mediaType === "pdf" ? "18.75rem" : "12.5rem",
                  objectFit: "contain",
                  display: "block",
                }}
              />
            )}
          {tooltip.mediaType === "video" && tooltip.mediaSrc && (
            <video
              src={tooltip.mediaSrc}
              autoPlay
              muted
              loop
              playsInline
              onLoadedData={() => setLayoutVersion((v) => v + 1)}
              onError={() => {
                setTooltip((prev) => ({
                  ...prev,
                  mediaSrc: null,
                  mediaType: null,
                }));
                setLayoutVersion((v) => v + 1);
              }}
              style={{
                maxWidth: "100%",
                maxHeight: "12.5rem",
                display: "block",
              }}
            />
          )}
          {tooltip.mediaType === "text" && tooltip.textContent && (
            <div className="tooltip-text-preview">{tooltip.textContent}</div>
          )}
        </div>
      )}

      <div style={{ overflowWrap: "anywhere", wordBreak: "break-word" }}>
        {tooltip.multiline
          ? tooltip.content.split("\n").map((line, i) => (
              <React.Fragment key={i}>
                {i === 0 ? <strong>{line}</strong> : line}
                {i < tooltip.content.split("\n").length - 1 && <br />}
              </React.Fragment>
            ))
          : (() => {
              const parts = tooltip.content.split('  ');
              if (parts.length > 1) {
                return (
                  <>
                    {parts[0]}
                    <span className="tooltip-shortcut">{parts.slice(1).join('  ')}</span>
                  </>
                );
              }
              return tooltip.content;
            })()}
      </div>

      {tooltip.diskStats && (
        <div style={{ marginTop: "0.5rem" }}>
          <DiskUsageChart
            total={tooltip.diskStats.total}
            free={tooltip.diskStats.free}
            t={t}
          />
        </div>
      )}
    </div>
  );
};
