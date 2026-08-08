import { useEffect, useRef } from "react";

import { formatPercent } from "../format";
import type { AssetListItem } from "../types";
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
}

export function AssetCard({
  asset,
  active,
  selected,
  onSelect,
  onToggleSelection,
  onStartDrag,
  onOpen,
}: AssetCardProps) {
  const clickTimer = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
    },
    [],
  );

  return (
    <div className={`asset-card-shell${active ? " is-active" : ""}`}>
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
          {asset.fileStatus === "missing" ? <span className="asset-alert">源文件缺失</span> : null}
          {asset.semanticStatus === "failed" ? <span className="asset-alert">分析失败</span> : null}
        </div>
        <div className="asset-card-body">
          <div className="asset-title-row">
            <span className="asset-title" title={asset.fileName}>
              {asset.fileName}
            </span>
            {asset.dominantColor ? (
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
                asset.classification.primaryCategory.effective,
                ...asset.classification.auxiliaryTags.effective,
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
    </div>
  );
}
