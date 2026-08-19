import type { AssetFilter } from "../types";

interface AnalysisStatusFilterBarProps {
  filter: AssetFilter;
  visible: boolean;
  onFilterChange: (filter: AssetFilter) => void;
}

export function AnalysisStatusFilterBar({
  filter,
  visible,
  onFilterChange,
}: AnalysisStatusFilterBarProps) {
  if (!visible && filter.analysisStatus === null) return null;

  return (
    <section className="analysis-status-filter-bar" aria-label="分析状态筛选">
      <div className="analysis-status-filter-heading">
        <strong>分析状态</strong>
      </div>
      <div className="analysis-status-filter-controls" role="group" aria-label="分析状态选项">
        <button
          type="button"
          className={filter.analysisStatus === "not_analyzed" ? "is-active" : ""}
          aria-pressed={filter.analysisStatus === "not_analyzed"}
          onClick={() =>
            onFilterChange({
              ...filter,
              analysisStatus: filter.analysisStatus === "not_analyzed" ? null : "not_analyzed",
            })
          }
        >
          <i className="analysis-status-dot" aria-hidden="true" />
          尚未语义分析
        </button>
        <button
          type="button"
          className={filter.analysisStatus === "failed" ? "is-active" : ""}
          aria-pressed={filter.analysisStatus === "failed"}
          onClick={() =>
            onFilterChange({
              ...filter,
              analysisStatus: filter.analysisStatus === "failed" ? null : "failed",
            })
          }
        >
          <i className="analysis-status-dot is-failed" aria-hidden="true" />
          分析失败
        </button>
      </div>
    </section>
  );
}
