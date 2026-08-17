import { useEffect, useRef } from "react";

import { classificationValueLabel } from "../classificationLabels";
import { formatPercent } from "../format";
import { MANUAL_COLOR_LABEL_OPTIONS, type AssetListItem, type ManualColorLabel } from "../types";
import { CheckIcon } from "./Icons";
import { Thumbnail } from "./Thumbnail";

type SelectionModifiers = {
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

interface AssetCardProps {
  asset: AssetListItem;
  active: boolean;
  selected: boolean;
  onSelect: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
  onToggleSelection: (asset: AssetListItem, modifiers?: SelectionModifiers) => void;
  onStartDrag?: (asset: AssetListItem, event: React.PointerEvent<HTMLButtonElement>) => void;
  onOpen?: (asset: AssetListItem) => void;
  onUpdateRating: (assetId: number, rating: number) => void;
  onUpdateColorLabel: (assetId: number, colorLabel: ManualColorLabel | null) => void;
  favorite: boolean;
  onToggleFavorite: (assetId: number) => void;
}

export function AssetCard({
  asset,
  active,
  selected,
  onSelect,
  onToggleSelection,
  onStartDrag,
  onOpen,
  onUpdateRating,
  onUpdateColorLabel,
  favorite,
  onToggleFavorite,
}: AssetCardProps) {
  const clickTimer = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
    },
    [],
  );

  const shellClassName = [
    "asset-card-shell",
    active ? "is-active" : "",
    asset.rating > 0 ? "has-rating" : "",
    asset.rating > 0 ? `rating-${asset.rating}` : "",
    asset.colorLabel ? "has-color-label" : "",
    asset.colorLabel ? `color-label-${asset.colorLabel}` : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={shellClassName}>
      <button
        type="button"
        className={`asset-check${selected ? " is-selected" : ""}`}
        onClick={(event) => {
          event.stopPropagation();
          onToggleSelection(asset, {
            ctrlKey: event.ctrlKey,
            metaKey: event.metaKey,
            shiftKey: event.shiftKey,
          });
        }}
        aria-label={selected ? `取消选择 ${asset.fileName}` : `选择 ${asset.fileName}`}
        aria-pressed={selected}
      >
        {selected ? <CheckIcon width="14" height="14" /> : null}
      </button>
      <button
        type="button"
        className={`asset-favorite${favorite ? " is-active" : ""}`}
        onClick={(event) => {
          event.stopPropagation();
          onToggleFavorite(asset.id);
        }}
        aria-label={favorite ? `取消收藏 ${asset.fileName}` : `收藏 ${asset.fileName}`}
        aria-pressed={favorite}
        title={favorite ? "取消收藏" : "收藏"}
      >
        {favorite ? "♥" : "♡"}
      </button>
      <div className="asset-card-frame">
        <button
          type="button"
          className={`asset-card${selected ? " is-selected" : ""}`}
          onClick={(event) => {
            const modifiers = {
              ctrlKey: event.ctrlKey,
              metaKey: event.metaKey,
              shiftKey: event.shiftKey,
            };
            if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
            clickTimer.current = window.setTimeout(() => {
              onSelect(asset, modifiers);
              clickTimer.current = null;
            }, 180);
          }}
          onPointerDown={(event) => onStartDrag?.(asset, event)}
          onDoubleClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
            clickTimer.current = null;
            onOpen?.(asset);
          }}
          aria-pressed={active}
          aria-label={`${asset.fileName}${active ? "，当前图片" : ""}`}
          title={asset.fileName}
        >
          <div className="asset-image-wrap">
            <Thumbnail asset={asset} />
            {asset.fileStatus === "missing" ? (
              <span className="asset-alert">源文件缺失</span>
            ) : null}
            {asset.semanticStatus === "failed" ? (
              <span className="asset-alert">分析失败</span>
            ) : null}
          </div>
          <div className="asset-card-body">
            <div className="asset-title-row">
              <span className="asset-title" title={asset.fileName}>
                {asset.fileName}
              </span>
              {asset.colorPalette?.prominentPalette.length ? (
                <span className="asset-color-palette" aria-label="强调色">
                  {asset.colorPalette.prominentPalette.slice(0, 3).map((candidate) => (
                    <i
                      className="color-swatch"
                      key={`${candidate.rank}-${candidate.color}`}
                      style={{ backgroundColor: candidate.color }}
                      title={`强调色 ${candidate.color}`}
                    />
                  ))}
                </span>
              ) : asset.dominantColor ? (
                <span
                  className="color-swatch"
                  style={{ backgroundColor: asset.dominantColor }}
                  title={`主色 ${asset.dominantColor}`}
                />
              ) : null}
            </div>
            <div className="asset-metrics">
              <span>亮度 {formatPercent(asset.brightness)}</span>
              <span>饱和度 {formatPercent(asset.saturation)}</span>
            </div>
            {asset.classification.primaryCategory.effective ||
            asset.classification.auxiliaryTags.effective.length > 0 ? (
              <div className="asset-labels">
                {[
                  asset.classification.primaryCategory.effective
                    ? classificationValueLabel(
                        asset.classification.primaryCategory.effective,
                        "primary",
                      )
                    : null,
                  ...asset.classification.auxiliaryTags.effective.map((value) =>
                    classificationValueLabel(value, "tag"),
                  ),
                ]
                  .filter((label): label is string => Boolean(label))
                  .slice(0, 2)
                  .map((label) => (
                    <span key={label}>{label}</span>
                  ))}
              </div>
            ) : null}
          </div>
        </button>
        <div
          className="asset-mark-controls"
          aria-label={`人工标记 ${asset.fileName}`}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={(event) => event.stopPropagation()}
        >
          <div className="asset-rating-controls" role="group" aria-label="星级">
            {Array.from({ length: 5 }, (_, index) => {
              const value = index + 1;
              const isActive = value <= asset.rating;
              return (
                <button
                  type="button"
                  className={isActive ? "is-active" : ""}
                  key={value}
                  aria-label={`${value} 星`}
                  aria-pressed={isActive}
                  onClick={() => onUpdateRating(asset.id, asset.rating === value ? 0 : value)}
                >
                  {isActive ? "★" : "☆"}
                </button>
              );
            })}
          </div>
          <div className="asset-color-label-controls" role="group" aria-label="色标">
            {MANUAL_COLOR_LABEL_OPTIONS.map((option) => {
              const isActive = asset.colorLabel === option.id;
              return (
                <button
                  type="button"
                  key={option.id}
                  className={isActive ? "is-active" : ""}
                  data-manual-color-label={option.id}
                  style={{ backgroundColor: option.color }}
                  aria-label={option.label}
                  aria-pressed={isActive}
                  onClick={() => onUpdateColorLabel(asset.id, isActive ? null : option.id)}
                />
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
