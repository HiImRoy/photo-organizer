import { useEffect, useState } from "react";

import { fetchThumbnail } from "../api";
import type { AssetListItem } from "../types";

const thumbnailCache = new Map<number, string>();
const thumbnailRequests = new Map<number, Promise<string>>();

function requestThumbnail(assetId: number) {
  const cached = thumbnailCache.get(assetId);
  if (cached) return Promise.resolve(cached);

  const inFlight = thumbnailRequests.get(assetId);
  if (inFlight) return inFlight;

  const request = fetchThumbnail(assetId)
    .then((value) => {
      thumbnailCache.set(assetId, value);
      return value;
    })
    .finally(() => {
      thumbnailRequests.delete(assetId);
    });
  thumbnailRequests.set(assetId, request);
  return request;
}

export function useThumbnailSource(asset: AssetListItem) {
  const [source, setSource] = useState<string | null>(() => thumbnailCache.get(asset.id) ?? null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    if (!asset.thumbnailAvailable || source) return undefined;

    void requestThumbnail(asset.id)
      .then((value) => {
        if (!active) return;
        setSource(value);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [asset.id, asset.thumbnailAvailable, source]);

  return { source, failed };
}
