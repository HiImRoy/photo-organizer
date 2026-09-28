import { createEvent, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ColorRangeFilter } from "./ColorRangeFilter";

type PointerPhase = "down" | "move" | "up" | "cancel";

function pointerEvent(
  target: Element,
  phase: PointerPhase,
  pointerId: number,
  x: number,
  y: number,
) {
  const event =
    phase === "down"
      ? createEvent.pointerDown(target, { button: 0, pointerId })
      : phase === "move"
        ? createEvent.pointerMove(target, { pointerId })
        : phase === "up"
          ? createEvent.pointerUp(target, { pointerId })
          : createEvent.pointerCancel(target, { pointerId });
  Object.defineProperties(event, {
    clientX: { value: x },
    clientY: { value: y },
  });
  fireEvent(target, event);
}

function pointAt(angle: number, radius = 57) {
  const radians = (angle * Math.PI) / 180;
  return {
    x: 90 + radius * Math.sin(radians),
    y: 90 - radius * Math.cos(radians),
  };
}

function setupWheel(center: number | null, width: number | null, strictness = 0.5) {
  const onChange = vi.fn();
  const onStrictnessChange = vi.fn();
  render(
    <ColorRangeFilter
      center={center}
      width={width}
      strictness={strictness}
      onChange={onChange}
      onStrictnessChange={onStrictnessChange}
    />,
  );
  const wheel = screen.getByRole("group", { name: "颜色范围色轮" });
  Object.defineProperty(wheel, "getBoundingClientRect", {
    configurable: true,
    value: () => ({ left: 0, top: 0, width: 180, height: 180 }),
  });
  Object.defineProperty(wheel, "setPointerCapture", { configurable: true, value: vi.fn() });
  Object.defineProperty(wheel, "hasPointerCapture", { configurable: true, value: () => true });
  Object.defineProperty(wheel, "releasePointerCapture", { configurable: true, value: vi.fn() });
  return { wheel, onChange, onStrictnessChange };
}

describe("ColorRangeFilter", () => {
  it("maps the top of the wheel to zero and neighboring positions across 360 consistently", () => {
    const { wheel, onChange } = setupWheel(null, null);
    const track = screen.getByTestId("color-range-track");

    for (const [pointerId, angle] of [359, 0, 1].entries()) {
      const point = pointAt(angle);
      pointerEvent(track, "down", pointerId + 1, point.x, point.y);
      pointerEvent(wheel, "up", pointerId + 1, point.x, point.y);
    }

    expect(onChange).toHaveBeenCalledTimes(3);
    expect(onChange.mock.calls[0][0]).toBeCloseTo(359, 1);
    expect(onChange.mock.calls[1]).toEqual([0, 60]);
    expect(onChange.mock.calls[2][0]).toBeCloseTo(1, 1);
  });

  it("creates a range on a blank-ring click and ignores blank drags", () => {
    const { wheel, onChange } = setupWheel(null, null);
    const track = screen.getByTestId("color-range-track");
    const firstPoint = pointAt(0);
    pointerEvent(track, "down", 1, firstPoint.x, firstPoint.y);
    pointerEvent(wheel, "move", 1, ...(Object.values(pointAt(90)) as [number, number]));
    pointerEvent(wheel, "up", 1, ...(Object.values(pointAt(90)) as [number, number]));
    expect(onChange).not.toHaveBeenCalled();

    const blankPoint = pointAt(270);
    pointerEvent(track, "down", 2, blankPoint.x, blankPoint.y);
    pointerEvent(wheel, "up", 2, blankPoint.x, blankPoint.y);
    expect(onChange).toHaveBeenCalledWith(270, 60);
  });

  it("relocates an existing range on a blank-ring click and preserves its width", () => {
    const { wheel, onChange } = setupWheel(90, 60);
    const track = screen.getByTestId("color-range-track");
    const blankPoint = pointAt(240);
    pointerEvent(track, "down", 1, blankPoint.x, blankPoint.y);
    pointerEvent(wheel, "up", 1, blankPoint.x, blankPoint.y);

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange.mock.calls[0][0]).toBeCloseTo(240, 6);
    expect(onChange.mock.calls[0][1]).toBe(60);
  });

  it("keeps the center inert and leaves a gap between narrow-range endpoint hit areas", () => {
    const { wheel, onChange } = setupWheel(90, 15);
    pointerEvent(wheel, "down", 1, 90, 90);
    pointerEvent(wheel, "up", 1, 90, 90);
    expect(onChange).not.toHaveBeenCalled();

    const startHit = screen
      .getByTestId("color-range-handle-start")
      .querySelector(".color-range-handle-hit");
    const endHit = screen
      .getByTestId("color-range-handle-end")
      .querySelector(".color-range-handle-hit");
    expect(startHit).not.toBeNull();
    expect(endHit).not.toBeNull();
    const startX = Number(startHit?.getAttribute("cx"));
    const startY = Number(startHit?.getAttribute("cy"));
    const endX = Number(endHit?.getAttribute("cx"));
    const endY = Number(endHit?.getAttribute("cy"));
    const startRadius = Number(startHit?.getAttribute("r"));
    const endRadius = Number(endHit?.getAttribute("r"));
    expect(Math.hypot(endX - startX, endY - startY)).toBeGreaterThan(startRadius + endRadius);
    expect(wheel.querySelector("[data-wheel-action='place']")).toHaveAttribute("r", "57");
  });

  it("resizes from an endpoint without rotating the opposite endpoint", () => {
    const { wheel, onChange } = setupWheel(90, 60);
    const startHandle = screen.getByRole("slider", { name: "调整色相范围起点" });
    const startPoint = pointAt(60);
    const nextPoint = pointAt(80);
    pointerEvent(startHandle, "down", 1, startPoint.x, startPoint.y);
    pointerEvent(wheel, "move", 1, nextPoint.x, nextPoint.y);
    pointerEvent(wheel, "up", 1, nextPoint.x, nextPoint.y);

    expect(onChange).toHaveBeenLastCalledWith(100, 40);
  });

  it("rotates only from the separated arc hit path and preserves width during fast drags", () => {
    const { wheel, onChange } = setupWheel(90, 90);
    const arc = screen.getByTestId("color-range-arc-hit");
    const downPoint = pointAt(100);
    const movePoint = pointAt(300);
    pointerEvent(arc, "down", 1, downPoint.x, downPoint.y);
    pointerEvent(wheel, "move", 1, ...(Object.values(pointAt(190)) as [number, number]));
    pointerEvent(wheel, "move", 1, movePoint.x, movePoint.y);
    pointerEvent(wheel, "up", 1, movePoint.x, movePoint.y);

    expect(onChange).toHaveBeenLastCalledWith(290, 90);
  });

  it("does not relocate when the outer edge of the highlighted arc is clicked", () => {
    const { wheel, onChange } = setupWheel(90, 90);
    const track = screen.getByTestId("color-range-track");
    const outerEdge = pointAt(90, 68);
    pointerEvent(track, "down", 1, outerEdge.x, outerEdge.y);
    pointerEvent(wheel, "up", 1, outerEdge.x, outerEdge.y);
    expect(onChange).not.toHaveBeenCalled();

    const nextAngle = pointAt(120, 68);
    pointerEvent(track, "down", 2, outerEdge.x, outerEdge.y);
    pointerEvent(wheel, "move", 2, nextAngle.x, nextAngle.y);
    pointerEvent(wheel, "up", 2, nextAngle.x, nextAngle.y);
    expect(onChange).toHaveBeenLastCalledWith(120, 90);
  });

  it("resizes across zero degrees and restores the starting range on pointer cancellation", () => {
    const { wheel, onChange } = setupWheel(350, 60);
    const startHandle = screen.getByTestId("color-range-handle-start");
    const downPoint = pointAt(320);
    const movedPoint = pointAt(340);
    pointerEvent(startHandle, "down", 1, downPoint.x, downPoint.y);
    pointerEvent(wheel, "move", 1, movedPoint.x, movedPoint.y);
    pointerEvent(wheel, "up", 1, movedPoint.x, movedPoint.y);
    expect(onChange).toHaveBeenLastCalledWith(0, 40);

    onChange.mockClear();
    pointerEvent(startHandle, "down", 2, downPoint.x, downPoint.y);
    pointerEvent(wheel, "move", 2, movedPoint.x, movedPoint.y);
    pointerEvent(wheel, "cancel", 2, movedPoint.x, movedPoint.y);
    expect(onChange).toHaveBeenNthCalledWith(1, 0, 40);
    expect(onChange).toHaveBeenLastCalledWith(350, 60);
  });

  it("restores an in-progress edit when focus leaves the wheel", () => {
    const { wheel, onChange } = setupWheel(90, 60);
    const endHandle = screen.getByTestId("color-range-handle-end");
    const downPoint = pointAt(120);
    const movedPoint = pointAt(140);
    pointerEvent(endHandle, "down", 1, downPoint.x, downPoint.y);
    pointerEvent(wheel, "move", 1, movedPoint.x, movedPoint.y);
    fireEvent.blur(endHandle, { relatedTarget: null });

    expect(onChange).toHaveBeenLastCalledWith(90, 60);
  });

  it("restores an in-progress edit when Escape cancels it", () => {
    const { wheel, onChange } = setupWheel(90, 60);
    const startHandle = screen.getByTestId("color-range-handle-start");
    const downPoint = pointAt(60);
    const movedPoint = pointAt(80);
    pointerEvent(startHandle, "down", 1, downPoint.x, downPoint.y);
    pointerEvent(wheel, "move", 1, movedPoint.x, movedPoint.y);
    fireEvent.keyDown(startHandle, { key: "Escape" });

    expect(onChange).toHaveBeenLastCalledWith(90, 60);
  });

  it("supports keyboard endpoint adjustment and announces the whole-image rule", () => {
    const { onChange } = setupWheel(90, 60, 0.8);
    const startHandle = screen.getByRole("slider", { name: "调整色相范围起点" });
    expect(
      screen.getByText("全图目标色面积 ≥ 26%；且主色候选须与所选色相一致"),
    ).toBeInTheDocument();
    fireEvent.keyDown(startHandle, { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith(90.5, 59);
  });

  it("keeps keyboard endpoint adjustments above the minimum width", () => {
    const { onChange } = setupWheel(90, 15);
    fireEvent.keyDown(screen.getByRole("slider", { name: "调整色相范围起点" }), {
      key: "ArrowRight",
    });
    expect(onChange).toHaveBeenCalledWith(90, 15);
  });

  it("keeps keyboard endpoint adjustments below the maximum width", () => {
    const { onChange } = setupWheel(90, 330);
    fireEvent.keyDown(screen.getByRole("slider", { name: "调整色相范围终点" }), {
      key: "ArrowRight",
    });
    expect(onChange).toHaveBeenCalledWith(90, 330);
  });

  it("updates matching strictness without changing the selected hue range", () => {
    const { onChange, onStrictnessChange } = setupWheel(120, 45, 0.5);
    fireEvent.change(screen.getByRole("slider", { name: "颜色匹配严格程度" }), {
      target: { value: "80" },
    });

    expect(onStrictnessChange).toHaveBeenCalledWith(0.8);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("clears the selected range", () => {
    const { onChange } = setupWheel(120, 45);
    expect(screen.getByTestId("color-range-handle-start")).toBeInTheDocument();
    expect(screen.getByTestId("color-range-handle-end")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "清除" }));

    expect(onChange).toHaveBeenCalledWith(null, null);
  });
});
