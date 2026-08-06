import { useEffect, useState } from "react";

import { fetchThumbnail } from "../api";
import type { AssetListItem } from "../types";
import { ImageIcon } from "./Icons";

const thumbnailCache = new Map<number, string>();

interface ThumbnailProps {
  asset: AssetListItem;
}

export function Thumbnail({ asset }: ThumbnailProps) {
  const [source, setSource] = useState<string | null>(() => thumbnailCache.get(asset.id) ?? null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    if (!asset.thumbnailAvailable || source) return undefined;

    void fetchThumbnail(asset.id)
      .then((value) => {
        if (!active) return;
        thumbnailCache.set(asset.id, value);
        setSource(value);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [asset.id, asset.thumbnailAvailable, source]);

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
