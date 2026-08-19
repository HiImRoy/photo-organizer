import { useState } from "react";

import type {
  AppSettings,
  AppThemeMode,
  ColorShortcut,
  RatingShortcut,
  ViewShortcut,
} from "../settings";
import { PanelIcon, SettingsIcon, SortIcon } from "./Icons";

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

type SettingsSectionId = "appearance" | "performance" | "shortcuts";

const settingsSections: Array<{
  id: SettingsSectionId;
  label: string;
  hint: string;
}> = [
  { id: "appearance", label: "界面", hint: "主题与显示" },
  { id: "performance", label: "性能", hint: "导入与分析" },
  { id: "shortcuts", label: "快捷键", hint: "视图、评分与色标" },
];

export function SettingsDialog({
  settings,
  themeMode,
  onChange,
  onThemeChange,
  onReset,
  onClose,
}: {
  settings: AppSettings;
  themeMode: AppThemeMode;
  onChange: (settings: AppSettings) => void;
  onThemeChange: (theme: AppThemeMode) => void;
  onReset: () => void;
  onClose: () => void;
}) {
  const [activeSection, setActiveSection] = useState<SettingsSectionId>("appearance");

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
          <div>
            <span className="settings-dialog-kicker">PhotoOrganizer</span>
            <h2 id="settings-dialog-title">设置</h2>
          </div>
          <button type="button" className="dialog-close" onClick={onClose} aria-label="关闭设置">
            ×
          </button>
        </header>

        <div className="settings-dialog-body">
          <nav className="settings-side-nav" aria-label="设置栏目" role="tablist">
            <span className="settings-side-nav-label">应用设置</span>
            {settingsSections.map((section) => {
              const selected = activeSection === section.id;
              return (
                <button
                  key={section.id}
                  type="button"
                  role="tab"
                  aria-selected={selected}
                  aria-controls={`settings-panel-${section.id}`}
                  className={selected ? "is-active" : ""}
                  onClick={() => setActiveSection(section.id)}
                >
                  {section.id === "appearance" ? (
                    <PanelIcon width="15" height="15" />
                  ) : section.id === "performance" ? (
                    <SortIcon width="15" height="15" />
                  ) : (
                    <SettingsIcon width="15" height="15" />
                  )}
                  <span>
                    <strong>{section.label}</strong>
                    <small>{section.hint}</small>
                  </span>
                </button>
              );
            })}
            <p className="settings-side-nav-note">修改会自动保存</p>
          </nav>

          <div className="settings-dialog-content">
            {activeSection === "appearance" ? (
              <section
                className="settings-page"
                id="settings-panel-appearance"
                role="tabpanel"
                aria-labelledby="settings-appearance-title"
              >
                <div className="settings-page-heading">
                  <span>界面</span>
                  <h3 id="settings-appearance-title">主题与显示</h3>
                  <p>调整 PhotoOrganizer 的整体显示方式。</p>
                </div>
                <div className="settings-form">
                  <div className="settings-field-row">
                    <div>
                      <strong>主题</strong>
                      <small>在深色和白天主题之间切换</small>
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
                </div>
              </section>
            ) : null}

            {activeSection === "performance" ? (
              <section
                className="settings-page"
                id="settings-panel-performance"
                role="tabpanel"
                aria-labelledby="settings-performance-title"
              >
                <div className="settings-page-heading">
                  <span>性能</span>
                  <h3 id="settings-performance-title">导入与分析</h3>
                  <p>控制缩略图处理和模型分析的资源占用。修改对新任务生效。</p>
                </div>
                <div className="settings-form">
                  <label className="settings-field-row settings-number-field">
                    <span>
                      <strong>导入并行数</strong>
                      <small>缩略图生成与基础特征处理 worker</small>
                    </span>
                    <select
                      aria-label="导入并行数"
                      value={settings.importWorkerCount}
                      onChange={(event) =>
                        onChange({ ...settings, importWorkerCount: Number(event.target.value) })
                      }
                    >
                      <option value="1">1</option>
                      <option value="2">2</option>
                    </select>
                  </label>
                  <label className="settings-field-row settings-number-field">
                    <span>
                      <strong>分析批大小</strong>
                      <small>CPU 模型一次送入的缩略图数量</small>
                    </span>
                    <select
                      aria-label="分析批大小"
                      value={settings.analysisBatchSize}
                      onChange={(event) =>
                        onChange({ ...settings, analysisBatchSize: Number(event.target.value) })
                      }
                    >
                      {[1, 2, 3, 4, 5, 6, 7, 8].map((value) => (
                        <option value={value} key={value}>
                          {value}
                        </option>
                      ))}
                    </select>
                  </label>
                  <div className="settings-field-row settings-readonly-row">
                    <span>
                      <strong>分析任务并行数</strong>
                      <small>单模型单任务，避免 CPU 争用</small>
                    </span>
                    <b>1</b>
                  </div>
                </div>
              </section>
            ) : null}

            {activeSection === "shortcuts" ? (
              <section
                className="settings-page"
                id="settings-panel-shortcuts"
                role="tabpanel"
                aria-labelledby="settings-shortcuts-title"
              >
                <div className="settings-page-heading">
                  <span>快捷键</span>
                  <h3 id="settings-shortcuts-title">视图与标记</h3>
                  <p>设置视图、评分和色标快捷键；输入单个字符后立即保存。</p>
                </div>
                <div className="settings-shortcut-groups">
                  <section className="settings-shortcut-group">
                    <h4>视图</h4>
                    <div className="settings-shortcut-list">
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
                  </section>
                  <section className="settings-shortcut-group">
                    <h4>星级</h4>
                    <div className="settings-shortcut-list">
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
                    </div>
                  </section>
                  <section className="settings-shortcut-group">
                    <h4>色标</h4>
                    <div className="settings-shortcut-list settings-color-shortcuts">
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
                  </section>
                  <section className="settings-shortcut-group settings-step-group">
                    <h4>星级调整</h4>
                    <div className="settings-shortcut-list settings-step-shortcuts">
                      <ShortcutInput
                        label="星级减少"
                        value={settings.shortcuts.ratingDown}
                        onChange={(value) =>
                          onChange({
                            ...settings,
                            shortcuts: { ...settings.shortcuts, ratingDown: value },
                          })
                        }
                      />
                      <ShortcutInput
                        label="星级增加"
                        value={settings.shortcuts.ratingUp}
                        onChange={(value) =>
                          onChange({
                            ...settings,
                            shortcuts: { ...settings.shortcuts, ratingUp: value },
                          })
                        }
                      />
                    </div>
                  </section>
                </div>
              </section>
            ) : null}
          </div>
        </div>

        <footer className="settings-dialog-footer">
          <span className="settings-footer-note">设置保存在本机</span>
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
        onChange={(event) => onChange(event.target.value.slice(-1))}
      />
    </label>
  );
}
