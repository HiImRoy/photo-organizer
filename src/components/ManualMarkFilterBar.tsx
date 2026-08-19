import type { AssetFilter } from "../types";
import { MANUAL_COLOR_LABEL_OPTIONS } from "../types";
import { RatingStars } from "./RatingStars";

interface ManualMarkFilterBarProps {
  filter: AssetFilter;
  onFilterChange: (filter: AssetFilter) => void;
}

export function ManualMarkFilterBar({ filter, onFilterChange }: ManualMarkFilterBarProps) {
  const selectedRating = filter.ratings.length > 0 ? Math.max(...filter.ratings) : null;
  const hasActiveFilter = filter.ratings.length > 0 || filter.colorLabels.length > 0;

  return (
    <div className="manual-mark-filter-bar" role="toolbar" aria-label="人工标记筛选">
      <div className="manual-mark-filter-controls">
        <RatingStars
          className="manual-mark-filter-group"
          value={selectedRating ?? 0}
          ariaLabel="按星级及以上筛选"
          buttonLabel={(rating) => `${rating} 星及以上`}
          pressedMode="exact"
          onChange={(rating) =>
            onFilterChange({
              ...filter,
              ratings: selectedRating === rating ? [] : [rating],
            })
          }
        />
        <span className="manual-mark-filter-divider" aria-hidden="true" />
        <div
          className="manual-mark-filter-group manual-mark-color-group"
          role="group"
          aria-label="按色标筛选"
        >
          {MANUAL_COLOR_LABEL_OPTIONS.map((option) => {
            const active = filter.colorLabels.includes(option.id);
            return (
              <button
                type="button"
                className={active ? "is-active" : ""}
                key={option.id}
                data-manual-color-label={option.id}
                style={{ backgroundColor: option.color }}
                aria-label={option.label}
                aria-pressed={active}
                title={option.label}
                onClick={() =>
                  onFilterChange({
                    ...filter,
                    colorLabels: toggleValue(filter.colorLabels, option.id),
                  })
                }
              />
            );
          })}
        </div>
        {hasActiveFilter ? (
          <button
            type="button"
            className="manual-mark-filter-clear"
            onClick={() => onFilterChange({ ...filter, ratings: [], colorLabels: [] })}
            aria-label="清除人工标记筛选"
          >
            清除
          </button>
        ) : null}
      </div>
    </div>
  );
}

function toggleValue<T>(values: T[], value: T) {
  return values.includes(value) ? values.filter((item) => item !== value) : [...values, value];
}
