import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ColorSwatches } from "./ColorSwatches";

describe("ColorSwatches", () => {
  it("allows selecting and clearing colors with visual buttons", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    render(<ColorSwatches value={[]} onChange={onChange} ariaLabel="选择主色" />);

    const blue = screen.getByRole("button", { name: "蓝色" });
    expect(blue).toHaveAttribute("aria-pressed", "false");

    await user.click(blue);
    expect(onChange).toHaveBeenLastCalledWith(["blue"]);
  });

  it("shows current selections as pressed", () => {
    render(<ColorSwatches value={["orange", "blue"]} onChange={vi.fn()} ariaLabel="选择主色" />);

    expect(screen.getByRole("button", { name: "橙色" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "红色" })).toHaveAttribute("aria-pressed", "false");
  });
});
