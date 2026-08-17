import { createEvent, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ColorRangeFilter } from "./ColorRangeFilter";

describe("ColorRangeFilter", () => {
  it("creates a default range when the empty wheel is pressed", () => {
    const onChange = vi.fn();
    render(<ColorRangeFilter center={null} width={null} onChange={onChange} />);
    const wheel = screen.getByRole("application", { name: "颜色范围环形筛选器" });
    Object.defineProperty(wheel, "getBoundingClientRect", {
      value: () => ({ left: 0, top: 0, width: 180, height: 180 }),
    });
    Object.defineProperty(wheel, "setPointerCapture", { value: vi.fn() });

    expect(document.querySelector(".color-range-hue-gradient")).toBeInTheDocument();

    const event = createEvent.pointerDown(wheel, { button: 0, pointerId: 1 });
    Object.defineProperties(event, {
      clientX: { value: 90 },
      clientY: { value: 30 },
    });
    fireEvent(wheel, event);

    expect(onChange).toHaveBeenCalledWith(0, 60);
  });

  it("clears the selected range", () => {
    const onChange = vi.fn();
    render(<ColorRangeFilter center={120} width={45} onChange={onChange} />);

    expect(screen.getByTestId("color-range-handle-start")).toBeInTheDocument();
    expect(screen.getByTestId("color-range-handle-end")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "清除" }));

    expect(onChange).toHaveBeenCalledWith(null, null);
  });

  it("updates matching strictness without changing the selected hue range", () => {
    const onChange = vi.fn();
    const onStrictnessChange = vi.fn();
    render(
      <ColorRangeFilter
        center={120}
        width={45}
        strictness={0.5}
        onChange={onChange}
        onStrictnessChange={onStrictnessChange}
      />,
    );

    fireEvent.change(screen.getByRole("slider", { name: "颜色匹配严格程度" }), {
      target: { value: "80" },
    });

    expect(onStrictnessChange).toHaveBeenCalledWith(0.8);
    expect(onChange).not.toHaveBeenCalled();
  });
});
