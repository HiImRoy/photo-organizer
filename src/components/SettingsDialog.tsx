import { useState } from "react";

import {
  analysisBatchLimit,
  AppSettings,
  GPU_ANALYSIS_BATCH_MAX,
  AppThemeMode,
  ColorShortcut,
  RatingShortcut,
  ViewShortcut,
} from "../settings";
import type { GpuCapabilities } from "../types";
import { CloseIcon, PanelIcon, SettingsIcon, SortIcon } from "./Icons";

const ratingRows: Array<{ id: RatingShortcut; label: string }> = [
  { id: "0", label: "清除星级" },
  { id: "1", label: "1 星" },
  { id: "2", label: "2 星" },
  { id: "3", label: "3 星" },
  { id: "4", label: "4 星" },
  { id: "5", label: "5 星" },
];

const colorRows: Array<{ id: ColorShortcut; label: string }> = [
  { id: "red", label: "红色" },
  { id: "yellow", label: "黄色" },
  { id: "green", label: "绿色" },
  { id: "blue", label: "蓝色" },
];

const viewRows: Array<{ id: ViewShortcut; label: string }> = [
  { id: "grid", label: "多图预览" },
  { id: "single", label: "单图预览" },
];

type SettingsSectionId = "display" | "processing" | "shortcuts";

const settingsSections: Array<{
  id: SettingsSectionId;
  label: string;
  hint: string;
}> = [
  { id: "display", label: "显示", hint: "主题与启动" },
  { id: "processing", label: "处理", hint: "导入与分析" },
  { id: "shortcuts", label: "快捷键", hint: "浏览与标记" },
];

export function SettingsDialog({
  settings,
  gpuCapabilities,
  themeMode,
  onChange,
  onThemeChange,
  onReset,
  onClose,
}: {
  settings: AppSettings;
  gpuCapabilities: GpuCapabilities | null;
  themeMode: AppThemeMode;
  onChange: (settings: AppSettings) => void;
  onThemeChange: (theme: AppThemeMode) => void;
  onReset: () => void;
  onClose: () => void;
}) {
  const [activeSection, setActiveSection] = useState<SettingsSectionId>("display");
  const gpuProviderReady = gpuCapabilities?.directml.state === "ready";
  const batchLimit = analysisBatchLimit(gpuCapabilities, settings.gpuAccelerationEnabled);

  return (
    <div
      className="modal-backdrop settings-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-dialog-title"
      >
        <header className="settings-dialog-heading">
          <h2 id="settings-dialog-title">设置</h2>
          <button type="button" className="dialog-close" onClick={onClose} aria-label="关闭设置">
            <CloseIcon width="16" height="16" />
          </button>
        </header>

        <div className="settings-dialog-body">
          <nav className="settings-side-nav" aria-label="设置栏目" role="tablist">
            {settingsSections.map((section) => {
              const selected = activeSection === section.id;
              return (
                <button
                  key={section.id}
                  id={`settings-tab-${section.id}`}
                  type="button"
                  role="tab"
                  aria-selected={selected}
                  aria-controls={`settings-panel-${section.id}`}
                  className={selected ? "is-active" : ""}
                  onClick={() => setActiveSection(section.id)}
                >
                  {section.id === "display" ? (
                    <PanelIcon width="16" height="16" />
                  ) : section.id === "processing" ? (
                    <SortIcon width="16" height="16" />
                  ) : (
                    <SettingsIcon width="16" height="16" />
                  )}
                  <span>
                    <strong>{section.label}</strong>
                    <small>{section.hint}</small>
                  </span>
                </button>
              );
            })}
          </nav>

          <div className="settings-dialog-content">
            {activeSection === "display" ? (
              <section
                className="settings-page"
                id="settings-panel-display"
                role="tabpanel"
                aria-labelledby="settings-tab-display"
              >
                <SettingsPageHeading title="显示" detail="主题与启动浏览方式" />
                <div className="settings-groups">
                  <SettingsGroup title="外观">
                    <div className="settings-field-row">
                      <div>
                        <strong>主题</strong>
                      </div>
                      <div className="settings-choice-row" role="radiogroup" aria-label="界面主题">
                        <label className={themeMode === "dark" ? "is-active" : ""}>
                          <input
                            type="radio"
                            name="settings-theme"
                            checked={themeMode === "dark"}
                            onChange={() => onThemeChange("dark")}
                          />
                          深色
                        </label>
                        <label className={themeMode === "light" ? "is-active" : ""}>
                          <input
                            type="radio"
                            name="settings-theme"
                            checked={themeMode === "light"}
                            onChange={() => onThemeChange("light")}
                          />
                          白天
                        </label>
                      </div>
                    </div>
                  </SettingsGroup>

                  <SettingsGroup title="启动">
                    <div className="settings-field-row">
                      <div>
                        <strong>默认预览</strong>
                        <small>下次启动时打开</small>
                      </div>
                      <div
                        className="settings-choice-row"
                        role="radiogroup"
                        aria-label="启动默认预览"
                      >
                        <label className={settings.startupView === "grid" ? "is-active" : ""}>
                          <input
                            type="radio"
                            name="settings-startup-view"
                            checked={settings.startupView === "grid"}
                            onChange={() => onChange({ ...settings, startupView: "grid" })}
                          />
                          多图
                        </label>
                        <label className={settings.startupView === "single" ? "is-active" : ""}>
                          <input
                            type="radio"
                            name="settings-startup-view"
                            checked={settings.startupView === "single"}
                            onChange={() => onChange({ ...settings, startupView: "single" })}
                          />
                          单图
                        </label>
                      </div>
                    </div>
                    <label className="settings-field-row settings-select-field">
                      <span>
                        <strong>默认每行图片数</strong>
                        <small>多图预览的初始密度</small>
                      </span>
                      <select
                        aria-label="默认每行图片数"
                        value={settings.defaultGridColumns}
                        onChange={(event) =>
                          onChange({ ...settings, defaultGridColumns: Number(event.target.value) })
                        }
                      >
                        {[2, 4, 6, 8, 10, 12].map((value) => (
                          <option value={value} key={value}>
                            {value} 张
                          </option>
                        ))}
                      </select>
                    </label>
                  </SettingsGroup>
                </div>
              </section>
            ) : null}

            {activeSection === "processing" ? (
              <section
                className="settings-page"
                id="settings-panel-processing"
                role="tabpanel"
                aria-labelledby="settings-tab-processing"
              >
                <SettingsPageHeading title="处理" detail="调整新任务的资源占用" />
                <div className="settings-groups">
                  <SettingsGroup title="导入">
                    <label className="settings-field-row settings-select-field">
                      <span>
                        <strong>并行任务</strong>
                        <small>缩略图生成与基础特征处理</small>
                      </span>
                      <select
                        aria-label="导入并行数"
                        value={settings.importWorkerCount}
                        onChange={(event) =>
                          onChange({ ...settings, importWorkerCount: Number(event.target.value) })
                        }
                      >
                        <option value="1">1</option>
                        <option value="2">2（推荐）</option>
                      </select>
                    </label>
                  </SettingsGroup>

                  <SettingsGroup title="分析" aside="单任务运行">
                    <div className="settings-readonly-row">
                      <span>
                        <strong>独立 GPU 加速</strong>
                        <small>{gpuCapabilities?.message ?? "正在检测硬件…"}</small>
                      </span>
                      <b>{gpuStatusLabel(gpuCapabilities)}</b>
                    </div>
                    <label className="settings-field-row settings-toggle-field">
                      <span>
                        <strong>启用 GPU 加速</strong>
                        <small>
                          {gpuProviderReady
                            ? "Provider 已就绪"
                            : "Provider 未就绪，当前分析仍使用 CPU"}
                        </small>
                      </span>
                      <input
                        type="checkbox"
                        aria-label="启用 GPU 加速"
                        checked={settings.gpuAccelerationEnabled}
                        disabled={!gpuProviderReady}
                        onChange={(event) =>
                          onChange({ ...settings, gpuAccelerationEnabled: event.target.checked })
                        }
                      />
                    </label>
                    <label className="settings-field-row settings-select-field">
                      <span>
                        <strong>批大小</strong>
                        <small>一次送入模型的缩略图数量；当前有效上限 {batchLimit}</small>
                      </span>
                      <select
                        aria-label="分析批大小"
                        value={Math.min(settings.analysisBatchSize, batchLimit)}
                        onChange={(event) =>
                          onChange({ ...settings, analysisBatchSize: Number(event.target.value) })
                        }
                      >
                        {Array.from(
                          { length: GPU_ANALYSIS_BATCH_MAX },
                          (_, index) => index + 1,
                        ).map((value) => (
                          <option value={value} key={value} disabled={value > batchLimit}>
                            {value === 4
                              ? "4（推荐）"
                              : value > batchLimit
                                ? value + "（当前上限）"
                                : value}
                          </option>
                        ))}
                      </select>
                    </label>
                  </SettingsGroup>
                </div>
              </section>
            ) : null}

            {activeSection === "shortcuts" ? (
              <section
                className="settings-page"
                id="settings-panel-shortcuts"
                role="tabpanel"
                aria-labelledby="settings-tab-shortcuts"
              >
                <SettingsPageHeading title="快捷键" detail="输入单个字符后自动保存" />
                <div className="settings-groups settings-shortcut-groups">
                  <SettingsGroup title="浏览">
                    <div className="settings-shortcut-list settings-shortcut-list-two">
                      {viewRows.map((row) => (
                        <ShortcutInput
                          key={row.id}
                          label={row.label}
                          value={settings.shortcuts.view[row.id]}
                          onChange={(value) =>
                            onChange({
                              ...settings,
                              shortcuts: {
                                ...settings.shortcuts,
                                view: { ...settings.shortcuts.view, [row.id]: value },
                              },
                            })
                          }
                        />
                      ))}
                    </div>
                  </SettingsGroup>

                  <SettingsGroup title="星级">
                    <div className="settings-shortcut-list settings-shortcut-list-rating">
                      {ratingRows.map((row) => (
                        <ShortcutInput
                          key={row.id}
                          label={row.label}
                          value={settings.shortcuts.ratings[row.id]}
                          onChange={(value) =>
                            onChange({
                              ...settings,
                              shortcuts: {
                                ...settings.shortcuts,
                                ratings: { ...settings.shortcuts.ratings, [row.id]: value },
                              },
                            })
                          }
                        />
                      ))}
                      <ShortcutInput
                        label="降低一级"
                        value={settings.shortcuts.ratingDown}
                        onChange={(value) =>
                          onChange({
                            ...settings,
                            shortcuts: { ...settings.shortcuts, ratingDown: value },
                          })
                        }
                      />
                      <ShortcutInput
                        label="提高一级"
                        value={settings.shortcuts.ratingUp}
                        onChange={(value) =>
                          onChange({
                            ...settings,
                            shortcuts: { ...settings.shortcuts, ratingUp: value },
                          })
                        }
                      />
                    </div>
                  </SettingsGroup>

                  <SettingsGroup title="色标">
                    <div className="settings-shortcut-list settings-shortcut-list-color">
                      {colorRows.map((row) => (
                        <ShortcutInput
                          key={row.id}
                          label={row.label}
                          value={settings.shortcuts.colors[row.id]}
                          onChange={(value) =>
                            onChange({
                              ...settings,
                              shortcuts: {
                                ...settings.shortcuts,
                                colors: { ...settings.shortcuts.colors, [row.id]: value },
                              },
                            })
                          }
                        />
                      ))}
                    </div>
                  </SettingsGroup>
                </div>
              </section>
            ) : null}
          </div>
        </div>

        <footer className="settings-dialog-footer">
          <span className="settings-footer-note">自动保存到本机</span>
          <div className="settings-footer-actions" role="group" aria-label="设置操作">
            <button
              type="button"
              className="settings-footer-action settings-footer-action-secondary"
              onClick={onReset}
            >
              恢复默认
            </button>
            <button
              type="button"
              className="settings-footer-action settings-footer-action-primary"
              onClick={onClose}
            >
              完成
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}

function gpuStatusLabel(capabilities: GpuCapabilities | null) {
  if (!capabilities) return "检测中";
  if (capabilities.directml.state === "ready") return "可用";
  if (capabilities.dedicatedGpuAvailable) return "待接入";
  if (
    capabilities.status === "no_gpu" ||
    capabilities.status === "integrated_only" ||
    capabilities.status === "unsupported_platform"
  ) {
    return "不可用";
  }
  return "检查失败";
}

function SettingsPageHeading({ title, detail }: { title: string; detail: string }) {
  return (
    <header className="settings-page-heading">
      <h3>{title}</h3>
      <p>{detail}</p>
    </header>
  );
}

function SettingsGroup({
  title,
  aside,
  children,
}: {
  title: string;
  aside?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="settings-group">
      <header className="settings-group-heading">
        <h4>{title}</h4>
        {aside ? <span>{aside}</span> : null}
      </header>
      <div className="settings-group-content">{children}</div>
    </section>
  );
}

function ShortcutInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="settings-shortcut-field">
      <span>{label}</span>
      <input
        aria-label={`${label}快捷键`}
        value={value}
        maxLength={1}
        onFocus={(event) => event.currentTarget.select()}
        onChange={(event) => onChange(event.target.value.slice(-1))}
      />
    </label>
  );
}
