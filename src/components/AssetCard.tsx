import { formatPercent } from "../format";
import type { AssetListItem } from "../types";
import { CheckIcon } from "./Icons";
import { Thumbnail } from "./Thumbnail";

interface AssetCardProps {
  asset: AssetListItem;
  selected: boolean;
  marked?: boolean;
  onSelect: (asset: AssetListItem) => void;
  onToggleMarked?: (asset: AssetListItem) => void;
}

export function AssetCard({
  asset,
  selected,
  marked = false,
  onSelect,
  onToggleMarked,
}: AssetCardProps) {
  return (
    <div className="asset-card-shell">
      <button
        type="button"
        className={`asset-card${selected ? " is-selected" : ""}`}
        onClick={() => onSelect(asset)}
        aria-pressed={selected}
        aria-label={`${asset.fileName}${selected ? "，已选中" : ""}`}
        title={asset.fileName}
      >
        <div className="asset-image-wrap">
          <Thumbnail asset={asset} />
          {selected ? (
            <span className="asset-selection" aria-hidden="true">
              <CheckIcon width="14" height="14" />
            </span>
          ) : null}
          {asset.fileStatus === "missing" ? <span className="asset-alert">源文件缺失</span> : null}
          {asset.analysisStatus === "failed" ? <span className="asset-alert">分析失败</span> : null}
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
          {asset.semanticLabels.length > 0 ? (
            <div className="asset-labels">
              {asset.semanticLabels.slice(0, 2).map((label) => (
                <span key={label.labelId}>{label.displayName}</span>
              ))}
            </div>
          ) : null}
        </div>
      </button>
      {onToggleMarked ? (
        <button
          type="button"
          className={`asset-mark-toggle${marked ? " is-marked" : ""}`}
          onClick={() => onToggleMarked(asset)}
          aria-pressed={marked}
          aria-label={`${asset.fileName}${marked ? "，取消整理预览选择" : "，加入整理预览选择"}`}
          title={marked ? "取消整理预览选择" : "加入整理预览选择"}
        >
          {marked ? <CheckIcon width="12" height="12" /> : "＋"}
        </button>
      ) : null}
    </div>
  );
}
