export function RangePair({
  label,
  minHint,
  maxHint,
  min,
  max,
  onChange,
}: {
  label: string;
  minHint: string;
  maxHint: string;
  min: number | null;
  max: number | null;
  onChange: (min: number | null, max: number | null) => void;
}) {
  const minPercent = Math.round((min ?? 0) * 100);
  const maxPercent = Math.round((max ?? 1) * 100);
  const rangeText = formatPercentRange(min, max);

  function updateMin(value: string) {
    const next = Math.min(Number(value) / 100, maxPercent / 100);
    onChange(next <= 0 ? null : next, max);
  }

  function updateMax(value: string) {
    const next = Math.max(Number(value) / 100, minPercent / 100);
    onChange(min, next >= 1 ? null : next);
  }

  return (
    <div className="range-filter-card">
      <div className="range-filter-heading">
        <strong>{label}</strong>
        <output aria-live="polite">{rangeText}</output>
      </div>
      <div className="range-slider" aria-label={`${label}筛选范围`}>
        <span
          className="range-slider-fill"
          style={{ left: `${minPercent}%`, width: `${Math.max(0, maxPercent - minPercent)}%` }}
        />
        <input
          className="range-slider-input range-slider-min"
          aria-label={`${label}最低百分比`}
          aria-valuetext={`${minPercent}%（${minHint}方向）`}
          type="range"
          min="0"
          max="100"
          step="5"
          value={minPercent}
          onChange={(event) => updateMin(event.target.value)}
        />
        <input
          className="range-slider-input range-slider-max"
          aria-label={`${label}最高百分比`}
          aria-valuetext={`${maxPercent}%（${maxHint}方向）`}
          type="range"
          min="0"
          max="100"
          step="5"
          value={maxPercent}
          onChange={(event) => updateMax(event.target.value)}
        />
      </div>
      <div className="range-slider-scale" aria-hidden="true">
        <span>{minHint}</span>
        <span>0% — 100%</span>
        <span>{maxHint}</span>
      </div>
      {min !== null || max !== null ? (
        <div className="range-filter-summary">当前显示：{rangeText}范围内</div>
      ) : null}
    </div>
  );
}

function formatPercentRange(min: number | null, max: number | null) {
  const format = (value: number) => `${Math.round(value * 100)}%`;
  if (min !== null && max !== null) return `${format(min)} — ${format(max)}`;
  if (min !== null) return `≥ ${format(min)}`;
  if (max !== null) return `≤ ${format(max)}`;
  return "全部";
}
