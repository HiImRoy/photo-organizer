import type { PointerEvent as ReactPointerEvent } from "react";
import { useRef } from "react";

import {
  DEFAULT_COLOR_HUE_STRICTNESS,
  colorHueMatchThresholdPercent,
  colorHueStrictnessLabel,
  normalizeColorHueStrictness,
} from "../colorFilter";

const CENTER = 90;
const RING_RADIUS = 57;
const RING_WIDTH = 25;
const HANDLE_RADIUS = 6;
const DEFAULT_WIDTH = 60;
const MIN_WIDTH = 15;
const MAX_WIDTH = 330;

type DragMode = "move" | "resize-start" | "resize-end";

type DragState = {
  mode: DragMode;
  offset: number;
  fixedStart: number;
  fixedEnd: number;
};

interface ColorRangeFilterProps {
  center: number | null;
  width: number | null;
  onChange: (center: number | null, width: number | null) => void;
  strictness?: number;
  onStrictnessChange?: (strictness: number) => void;
}

export function ColorRangeFilter({
  center,
  width,
  onChange,
  strictness = DEFAULT_COLOR_HUE_STRICTNESS,
  onStrictnessChange,
}: ColorRangeFilterProps) {
  const dragRef = useRef<DragState | null>(null);
  const hasSelection = center !== null && width !== null;
  const normalizedStrictness = normalizeColorHueStrictness(strictness);
  const strictnessPercent = Math.round(normalizedStrictness * 100);
  const expectedShare = colorHueMatchThresholdPercent(normalizedStrictness);
  const normalizedCenter = hasSelection ? normalizeHue(center) : 0;
  const normalizedWidth = hasSelection ? clampWidth(width) : DEFAULT_WIDTH;
  const start = normalizeHue(normalizedCenter - normalizedWidth / 2);
  const end = normalizeHue(normalizedCenter + normalizedWidth / 2);

  function angleFromEvent(event: ReactPointerEvent<SVGSVGElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = event.clientX - (rect.left + rect.width / 2);
    const y = event.clientY - (rect.top + rect.height / 2);
    return normalizeHue((Math.atan2(x, -y) * 180) / Math.PI);
  }

  function beginDrag(event: ReactPointerEvent<SVGSVGElement>) {
    if (event.button !== undefined && event.button !== 0) return;
    event.preventDefault();
    const angle = angleFromEvent(event);
    const currentStart = start;
    const currentEnd = end;

    let mode: DragMode = "move";
    let offset = 0;
    let fixedStart = currentStart;
    let fixedEnd = currentEnd;

    if (!hasSelection) {
      onChange(angle, DEFAULT_WIDTH);
    } else if (angularDistance(angle, currentStart) <= 12) {
      mode = "resize-start";
      fixedEnd = currentEnd;
    } else if (angularDistance(angle, currentEnd) <= 12) {
      mode = "resize-end";
      fixedStart = currentStart;
    } else if (isInRange(angle, currentStart, normalizedWidth)) {
      offset = signedAngleDistance(angle, normalizedCenter);
    } else {
      onChange(angle, DEFAULT_WIDTH);
    }

    dragRef.current = { mode, offset, fixedStart, fixedEnd };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function updateDrag(event: ReactPointerEvent<SVGSVGElement>) {
    const drag = dragRef.current;
    if (!drag) return;
    const angle = angleFromEvent(event);

    if (drag.mode === "move") {
      onChange(normalizeHue(angle + drag.offset), normalizedWidth);
      return;
    }

    if (drag.mode === "resize-start") {
      const nextWidth = clampWidth(clockwiseDistance(angle, drag.fixedEnd));
      onChange(normalizeHue(angle + nextWidth / 2), nextWidth);
      return;
    }

    const nextWidth = clampWidth(clockwiseDistance(drag.fixedStart, angle));
    onChange(normalizeHue(drag.fixedStart + nextWidth / 2), nextWidth);
  }

  function finishDrag(event: ReactPointerEvent<SVGSVGElement>) {
    if (dragRef.current) {
      dragRef.current = null;
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
    }
  }

  return (
    <div className="color-range-filter" data-testid="color-range-filter">
      <div className="color-range-wheel-wrap">
        <div className="color-range-hue-gradient" aria-hidden="true" />
        <svg
          className="color-range-wheel"
          viewBox="0 0 180 180"
          role="application"
          aria-label="颜色范围环形筛选器"
          onPointerDown={beginDrag}
          onPointerMove={updateDrag}
          onPointerUp={finishDrag}
          onPointerCancel={finishDrag}
        >
          {hasSelection ? (
            <>
              <path
                className="color-range-selection"
                d={arcPath(start, normalizedWidth, RING_RADIUS)}
                fill="none"
                strokeWidth={RING_WIDTH + 7}
                strokeLinecap="round"
              />
              <path
                className="color-range-selection-edge"
                d={arcPath(start, normalizedWidth, RING_RADIUS)}
                fill="none"
                strokeWidth="2"
                strokeLinecap="round"
              />
              <circle
                className="color-range-handle"
                data-testid="color-range-handle-start"
                cx={pointAt(start, RING_RADIUS).x}
                cy={pointAt(start, RING_RADIUS).y}
                r={HANDLE_RADIUS}
              />
              <circle
                className="color-range-handle"
                data-testid="color-range-handle-end"
                cx={pointAt(end, RING_RADIUS).x}
                cy={pointAt(end, RING_RADIUS).y}
                r={HANDLE_RADIUS}
              />
            </>
          ) : null}
          <circle className="color-range-wheel-center" cx={CENTER} cy={CENTER} r={38} />
          <text className="color-range-wheel-label" x={CENTER} y={CENTER - 2} textAnchor="middle">
            颜色范围
          </text>
          <text className="color-range-wheel-value" x={CENTER} y={CENTER + 14} textAnchor="middle">
            {hasSelection ? `${Math.round(normalizedWidth)}°` : "未选择"}
          </text>
        </svg>
      </div>
      <div className="color-range-filter-meta">
        <span>{hasSelection ? `中心 ${Math.round(normalizedCenter)}°` : "未选择"}</span>
        <button
          type="button"
          className="color-range-reset"
          disabled={!hasSelection}
          onClick={() => onChange(null, null)}
        >
          清除
        </button>
      </div>
      <div className="color-range-strictness">
        <div className="color-range-strictness-heading">
          <span>颜色匹配严格程度</span>
          <output aria-live="polite">
            {colorHueStrictnessLabel(normalizedStrictness)} · {strictnessPercent}%
          </output>
        </div>
        <input
          type="range"
          min="0"
          max="100"
          step="5"
          value={strictnessPercent}
          aria-label="颜色匹配严格程度"
          onChange={(event) => onStrictnessChange?.(Number(event.target.value) / 100)}
        />
        <div className="color-range-strictness-scale" aria-hidden="true">
          <span>宽松</span>
          <span>平衡</span>
          <span>严格</span>
        </div>
        {hasSelection ? <small>至少占彩色区域 {expectedShare}%</small> : null}
      </div>
    </div>
  );
}

function normalizeHue(value: number) {
  return ((value % 360) + 360) % 360;
}

function clampWidth(value: number) {
  return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, value));
}

function clockwiseDistance(from: number, to: number) {
  return normalizeHue(to - from);
}

function signedAngleDistance(from: number, to: number) {
  const difference = normalizeHue(to - from);
  return difference > 180 ? difference - 360 : difference;
}

function angularDistance(left: number, right: number) {
  return Math.abs(signedAngleDistance(left, right));
}

function isInRange(angle: number, start: number, width: number) {
  return clockwiseDistance(start, angle) <= width;
}

function pointAt(angle: number, radius: number) {
  const radians = (angle * Math.PI) / 180;
  return {
    x: CENTER + radius * Math.sin(radians),
    y: CENTER - radius * Math.cos(radians),
  };
}

function arcPath(start: number, width: number, radius: number) {
  const startPoint = pointAt(start, radius);
  const endPoint = pointAt(start + width, radius);
  const largeArcFlag = width > 180 ? 1 : 0;
  return `M ${startPoint.x.toFixed(3)} ${startPoint.y.toFixed(3)} A ${radius} ${radius} 0 ${largeArcFlag} 1 ${endPoint.x.toFixed(3)} ${endPoint.y.toFixed(3)}`;
}
