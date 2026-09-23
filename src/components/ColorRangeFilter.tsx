import type {
  FocusEvent as ReactFocusEvent,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import { useRef, useState } from "react";

import {
  DEFAULT_COLOR_HUE_STRICTNESS,
  colorHueFilterSummary,
  colorHueStrictnessLabel,
  normalizeColorHueStrictness,
} from "../colorFilter";

const CENTER = 90;
const RING_RADIUS = 57;
const RING_WIDTH = 25;
const HANDLE_TRIM_ANGLE = 20;
const DEFAULT_WIDTH = 60;
const MIN_WIDTH = 15;
const MAX_WIDTH = 330;
const CLICK_SLOP = 5;

type DragMode = "rotate" | "resize-start" | "resize-end" | "place";

type DragState = {
  mode: DragMode;
  pointerId: number;
  downX: number;
  downY: number;
  initialCenter: number | null;
  initialWidth: number;
  startAngle: number;
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
  const [isDragging, setIsDragging] = useState(false);
  const hasSelection = center !== null && width !== null;
  const normalizedStrictness = normalizeColorHueStrictness(strictness);
  const strictnessPercent = Math.round(normalizedStrictness * 100);
  const matchSummary = colorHueFilterSummary(normalizedStrictness);
  const normalizedCenter = hasSelection ? normalizeHue(center) : 0;
  const normalizedWidth = hasSelection ? clampWidth(width) : DEFAULT_WIDTH;
  const handleRadius = Math.min(
    9,
    Math.max(0, RING_RADIUS * Math.sin((normalizedWidth * Math.PI) / 360) - 1),
  );
  const start = normalizeHue(normalizedCenter - normalizedWidth / 2);
  const end = normalizeHue(normalizedCenter + normalizedWidth / 2);
  const interactionWidth = normalizedWidth - HANDLE_TRIM_ANGLE * 2;

  function angleFromEvent(event: ReactPointerEvent<SVGSVGElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = event.clientX - (rect.left + rect.width / 2);
    const y = event.clientY - (rect.top + rect.height / 2);
    return normalizeHue((Math.atan2(x, -y) * 180) / Math.PI);
  }

  function beginDrag(event: ReactPointerEvent<SVGSVGElement>) {
    if (event.button !== undefined && event.button !== 0) return;

    const actionElement =
      event.target instanceof Element
        ? event.target.closest<SVGElement>("[data-wheel-action]")
        : null;
    const action = actionElement?.dataset.wheelAction;
    if (
      action !== "place" &&
      action !== "rotate" &&
      action !== "resize-start" &&
      action !== "resize-end"
    ) {
      return;
    }

    if (
      (action === "rotate" || action === "resize-start" || action === "resize-end") &&
      !hasSelection
    ) {
      return;
    }

    event.preventDefault();
    const angle = angleFromEvent(event);
    let mode: DragMode = action;
    let offset = 0;
    if (action === "rotate") offset = signedAngleDistance(normalizedCenter, angle);
    if (action === "place" && hasSelection && isInRange(angle, start, normalizedWidth)) {
      const endpointClearance = (Math.asin(handleRadius / RING_RADIUS) * 180) / Math.PI + 1;
      if (
        angularDistance(angle, start) <= endpointClearance ||
        angularDistance(angle, end) <= endpointClearance
      ) {
        return;
      }
      mode = "rotate";
      offset = signedAngleDistance(normalizedCenter, angle);
    }

    dragRef.current = {
      mode,
      pointerId: event.pointerId,
      downX: event.clientX,
      downY: event.clientY,
      initialCenter: hasSelection ? normalizedCenter : null,
      initialWidth: normalizedWidth,
      startAngle: angle,
      offset,
      fixedStart: start,
      fixedEnd: end,
    };
    setIsDragging(true);

    try {
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
      // Some embedded WebViews do not expose pointer capture for SVG roots.
      // The pointer events still finish normally while the wheel stays mounted.
    }
  }

  function updateDrag(event: ReactPointerEvent<SVGSVGElement>) {
    const drag = dragRef.current;
    if (!drag || event.pointerId !== drag.pointerId || drag.mode === "place") return;
    const angle = angleFromEvent(event);

    if (drag.mode === "rotate") {
      onChange(normalizeHue(angle - drag.offset), drag.initialWidth);
      return;
    }

    if (drag.mode === "resize-start") {
      const requestedWidth = clockwiseDistance(angle, drag.fixedEnd);
      const nextWidth = clampWidth(requestedWidth);
      const nextStart =
        requestedWidth < MIN_WIDTH
          ? normalizeHue(drag.fixedEnd - MIN_WIDTH)
          : requestedWidth > MAX_WIDTH
            ? normalizeHue(drag.fixedEnd - MAX_WIDTH)
            : angle;
      onChange(normalizeHue(nextStart + nextWidth / 2), nextWidth);
      return;
    }

    const requestedWidth = clockwiseDistance(drag.fixedStart, angle);
    const nextWidth = clampWidth(requestedWidth);
    const nextEnd =
      requestedWidth < MIN_WIDTH
        ? normalizeHue(drag.fixedStart + MIN_WIDTH)
        : requestedWidth > MAX_WIDTH
          ? normalizeHue(drag.fixedStart + MAX_WIDTH)
          : angle;
    onChange(
      normalizeHue(drag.fixedStart + clockwiseDistance(drag.fixedStart, nextEnd) / 2),
      nextWidth,
    );
  }

  function finishDrag(event: ReactPointerEvent<SVGSVGElement>, canceled = false) {
    const drag = dragRef.current;
    if (!drag || event.pointerId !== drag.pointerId) return;

    if (canceled) {
      cancelActiveDrag(event.currentTarget);
      return;
    }

    dragRef.current = null;
    setIsDragging(false);
    if (drag.mode === "place") {
      const moved = Math.hypot(event.clientX - drag.downX, event.clientY - drag.downY);
      if (moved <= CLICK_SLOP) {
        onChange(drag.startAngle, drag.initialCenter === null ? DEFAULT_WIDTH : drag.initialWidth);
      }
    }

    releasePointerCapture(event.currentTarget, event.pointerId);
  }

  function cancelActiveDrag(wheel: SVGSVGElement) {
    const drag = dragRef.current;
    if (!drag) return;
    dragRef.current = null;
    setIsDragging(false);
    if (drag.mode !== "place")
      onChange(drag.initialCenter, drag.initialCenter === null ? null : drag.initialWidth);
    releasePointerCapture(wheel, drag.pointerId);
  }

  function cancelOnBlur(event: ReactFocusEvent<SVGSVGElement>) {
    if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget))
      return;
    cancelActiveDrag(event.currentTarget);
  }

  function handleKeyDown(event: ReactKeyboardEvent<SVGSVGElement>) {
    if (event.key === "Escape" && dragRef.current) {
      event.preventDefault();
      cancelActiveDrag(event.currentTarget);
      return;
    }

    const actionElement =
      event.target instanceof Element
        ? event.target.closest<SVGElement>("[data-wheel-action]")
        : null;
    const action = actionElement?.dataset.wheelAction;
    if (!hasSelection || (action !== "resize-start" && action !== "resize-end")) return;

    const step = event.shiftKey ? 5 : event.key === "PageUp" || event.key === "PageDown" ? 15 : 1;
    const direction =
      event.key === "ArrowRight" || event.key === "ArrowUp" || event.key === "PageUp"
        ? 1
        : event.key === "ArrowLeft" || event.key === "ArrowDown" || event.key === "PageDown"
          ? -1
          : 0;
    if (direction === 0) return;
    event.preventDefault();

    if (action === "resize-start") {
      const nextStart = normalizeHue(start + direction * step);
      const requestedWidth = clockwiseDistance(nextStart, end);
      const nextWidth = clampWidth(requestedWidth);
      const boundedStart =
        requestedWidth < MIN_WIDTH
          ? normalizeHue(end - MIN_WIDTH)
          : requestedWidth > MAX_WIDTH
            ? normalizeHue(end - MAX_WIDTH)
            : nextStart;
      onChange(normalizeHue(boundedStart + nextWidth / 2), nextWidth);
      return;
    }

    const nextEnd = normalizeHue(end + direction * step);
    const requestedWidth = clockwiseDistance(start, nextEnd);
    const nextWidth = clampWidth(requestedWidth);
    const boundedEnd =
      requestedWidth < MIN_WIDTH
        ? normalizeHue(start + MIN_WIDTH)
        : requestedWidth > MAX_WIDTH
          ? normalizeHue(start + MAX_WIDTH)
          : nextEnd;
    onChange(normalizeHue(start + clockwiseDistance(start, boundedEnd) / 2), nextWidth);
  }

  function handleLostPointerCapture(event: ReactPointerEvent<SVGSVGElement>) {
    if (dragRef.current?.pointerId === event.pointerId) finishDrag(event, true);
  }

  const tickMarks = Array.from({ length: 12 }, (_, index) => {
    const angle = index * 30;
    const outer = pointAt(angle, 75);
    const inner = pointAt(angle, index % 2 === 0 ? 69 : 71);
    return (
      <line
        key={angle}
        className={index % 2 === 0 ? "color-range-tick is-major" : "color-range-tick"}
        x1={inner.x}
        y1={inner.y}
        x2={outer.x}
        y2={outer.y}
      />
    );
  });

  return (
    <div className="color-range-filter" data-testid="color-range-filter">
      <div className="color-range-wheel-wrap">
        <div className="color-range-hue-gradient" aria-hidden="true" />
        <svg
          className={`color-range-wheel${isDragging ? " is-dragging" : ""}`}
          viewBox="0 0 180 180"
          role="group"
          aria-label="颜色范围色轮"
          onPointerDown={beginDrag}
          onPointerMove={updateDrag}
          onPointerUp={(event) => finishDrag(event)}
          onPointerCancel={(event) => finishDrag(event, true)}
          onLostPointerCapture={handleLostPointerCapture}
          onBlurCapture={cancelOnBlur}
          onKeyDown={handleKeyDown}
        >
          {tickMarks}
          <circle
            className="color-range-track-hit"
            data-wheel-action="place"
            data-testid="color-range-track"
            cx={CENTER}
            cy={CENTER}
            r={RING_RADIUS}
          />
          {hasSelection ? (
            <>
              <path
                className="color-range-selection"
                d={arcPath(start, normalizedWidth, RING_RADIUS)}
                fill="none"
                strokeWidth={RING_WIDTH}
                strokeLinecap="round"
              />
              <path
                className="color-range-selection-edge"
                d={arcPath(start, normalizedWidth, RING_RADIUS)}
                fill="none"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
              {interactionWidth > 0 ? (
                <path
                  className="color-range-arc-hit"
                  data-wheel-action="rotate"
                  data-testid="color-range-arc-hit"
                  d={arcPath(start + HANDLE_TRIM_ANGLE, interactionWidth, RING_RADIUS)}
                  fill="none"
                  strokeWidth="20"
                  strokeLinecap="round"
                />
              ) : null}
              <g
                className="color-range-endpoint"
                data-wheel-action="resize-start"
                data-testid="color-range-handle-start"
                role="slider"
                aria-label="调整色相范围起点"
                aria-valuemin={0}
                aria-valuemax={359}
                aria-valuenow={Math.round(start) % 360}
                aria-valuetext={`起点 ${Math.round(start)} 度`}
                tabIndex={0}
              >
                <circle
                  className="color-range-handle-hit"
                  cx={pointAt(start, RING_RADIUS).x}
                  cy={pointAt(start, RING_RADIUS).y}
                  r={handleRadius}
                />
                <circle
                  className="color-range-handle"
                  cx={pointAt(start, RING_RADIUS).x}
                  cy={pointAt(start, RING_RADIUS).y}
                  r="4.5"
                />
              </g>
              <g
                className="color-range-endpoint"
                data-wheel-action="resize-end"
                data-testid="color-range-handle-end"
                role="slider"
                aria-label="调整色相范围终点"
                aria-valuemin={0}
                aria-valuemax={359}
                aria-valuenow={Math.round(end) % 360}
                aria-valuetext={`终点 ${Math.round(end)} 度`}
                tabIndex={0}
              >
                <circle
                  className="color-range-handle-hit"
                  cx={pointAt(end, RING_RADIUS).x}
                  cy={pointAt(end, RING_RADIUS).y}
                  r={handleRadius}
                />
                <circle
                  className="color-range-handle"
                  cx={pointAt(end, RING_RADIUS).x}
                  cy={pointAt(end, RING_RADIUS).y}
                  r="4.5"
                />
              </g>
            </>
          ) : null}
          <circle className="color-range-wheel-center" cx={CENTER} cy={CENTER} r={38} />
          <text className="color-range-wheel-label" x={CENTER} y={CENTER - 2} textAnchor="middle">
            色相范围
          </text>
          <text className="color-range-wheel-value" x={CENTER} y={CENTER + 14} textAnchor="middle">
            {hasSelection ? `${Math.round(normalizedWidth)}°` : "点环设置"}
          </text>
        </svg>
      </div>
      <p className="color-range-filter-hint">点空白色环定位 · 拖弧旋转 · 拖端点调边界</p>
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
        {hasSelection ? <small>{matchSummary}</small> : null}
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

function releasePointerCapture(wheel: SVGSVGElement, pointerId: number) {
  try {
    if (wheel.hasPointerCapture(pointerId)) wheel.releasePointerCapture(pointerId);
  } catch {
    // Pointer capture may already have been released by the platform.
  }
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
