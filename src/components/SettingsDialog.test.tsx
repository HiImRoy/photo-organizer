import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DEFAULT_APP_SETTINGS } from "../settings";
import { SettingsDialog } from "./SettingsDialog";

describe("SettingsDialog keyboard focus management", () => {
  it("focuses the dialog, traps Tab in both directions, and restores the trigger focus", async () => {
    const onClose = vi.fn();
    render(<button type="button">打开设置</button>);
    const trigger = screen.getByRole("button", { name: "打开设置" });
    trigger.focus();

    const dialogView = render(
      <SettingsDialog
        settings={DEFAULT_APP_SETTINGS}
        gpuCapabilities={null}
        themeMode="dark"
        onChange={vi.fn()}
        onThemeChange={vi.fn()}
        onReset={vi.fn()}
        onClose={onClose}
      />,
    );
    const dialog = screen.getByRole("dialog", { name: "设置" });
    await vi.waitFor(() =>
      expect(within(dialog).getByRole("radio", { name: "深色" })).toHaveFocus(),
    );

    const focusable = Array.from(
      dialog.querySelectorAll<HTMLElement>(
        'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])',
      ),
    );
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    expect(first).toHaveAccessibleName("关闭设置");
    expect(last).toHaveAccessibleName("完成");

    last.focus();
    const forwardTab = new KeyboardEvent("keydown", {
      bubbles: true,
      cancelable: true,
      key: "Tab",
    });
    last.dispatchEvent(forwardTab);
    expect(forwardTab.defaultPrevented).toBe(true);
    expect(first).toHaveFocus();

    first.focus();
    const backwardTab = new KeyboardEvent("keydown", {
      bubbles: true,
      cancelable: true,
      key: "Tab",
      shiftKey: true,
    });
    first.dispatchEvent(backwardTab);
    expect(backwardTab.defaultPrevented).toBe(true);
    expect(last).toHaveFocus();

    const escape = new KeyboardEvent("keydown", {
      bubbles: true,
      cancelable: true,
      key: "Escape",
    });
    dialog.dispatchEvent(escape);
    expect(escape.defaultPrevented).toBe(true);
    expect(onClose).toHaveBeenCalledTimes(1);

    dialogView.unmount();
    expect(trigger).toHaveFocus();
  });
});
