import type { AssetListItem } from "../types";
import { ImageIcon } from "./Icons";
import { useThumbnailSource } from "./thumbnailSource";

interface ThumbnailProps {
  asset: AssetListItem;
}

export function Thumbnail({ asset }: ThumbnailProps) {
  const { source, failed } = useThumbnailSource(asset);

  if (!asset.thumbnailAvailable || failed) {
    return (
      <div className="thumbnail-placeholder" data-status={asset.analysisStatus}>
        <ImageIcon width="30" height="30" />
        <span>{asset.errorMessage ? "无法读取" : "等待缩略图"}</span>
      </div>
    );
  }

  if (!source) {
    return <div className="thumbnail-skeleton" aria-label="正在加载缩略图" />;
  }

  return <img className="thumbnail-image" src={source} alt="" loading="lazy" draggable={false} />;
}
