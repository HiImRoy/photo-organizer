import { useCallback, useEffect, useState } from "react";

import { fetchThumbnail } from "../api";
import type { AssetListItem } from "../types";

const thumbnailCache = new Map<number, string>();
const thumbnailRequests = new Map<number, Promise<string>>();
const queuedThumbnailRequests = new Map<number, PendingThumbnailRequest>();
const thumbnailQueue: PendingThumbnailRequest[] = [];
const MAX_CONCURRENT_THUMBNAIL_REQUESTS = 6;
let activeThumbnailRequests = 0;
let thumbnailRequestSequence = 0;

type PendingThumbnailRequest = {
  assetId: number;
  priority: number;
  sequence: number;
  resolve: (source: string) => void;
  reject: (reason: unknown) => void;
};

export type ThumbnailLoadRef = (node: HTMLElement | null) => void;

function drainThumbnailQueue() {
  while (activeThumbnailRequests < MAX_CONCURRENT_THUMBNAIL_REQUESTS && thumbnailQueue.length) {
    thumbnailQueue.sort(
      (left, right) => left.priority - right.priority || left.sequence - right.sequence,
    );
    const pending = thumbnailQueue.shift();
    if (!pending) return;
    queuedThumbnailRequests.delete(pending.assetId);
    activeThumbnailRequests += 1;

    void fetchThumbnail(pending.assetId)
      .then((source) => {
        thumbnailCache.set(pending.assetId, source);
        pending.resolve(source);
      })
      .catch((reason: unknown) => {
        pending.reject(reason);
      })
      .finally(() => {
        activeThumbnailRequests -= 1;
        thumbnailRequests.delete(pending.assetId);
        drainThumbnailQueue();
      });
  }
}

export function requestThumbnail(assetId: number, priority = 0) {
  const cached = thumbnailCache.get(assetId);
  if (cached) return Promise.resolve(cached);

  const inFlight = thumbnailRequests.get(assetId);
  if (inFlight) {
    const queued = queuedThumbnailRequests.get(assetId);
    if (queued && priority < queued.priority) {
      queued.priority = priority;
      drainThumbnailQueue();
    }
    return inFlight;
  }

  const request = new Promise<string>((resolve, reject) => {
    const pending: PendingThumbnailRequest = {
      assetId,
      priority,
      sequence: thumbnailRequestSequence++,
      resolve,
      reject,
    };
    queuedThumbnailRequests.set(assetId, pending);
    thumbnailQueue.push(pending);
  });
  thumbnailRequests.set(assetId, request);
  drainThumbnailQueue();
  return request;
}

export function useThumbnailSource(asset: AssetListItem) {
  const [source, setSource] = useState<string | null>(() => thumbnailCache.get(asset.id) ?? null);
  const [failed, setFailed] = useState(false);
  const [loadTarget, setLoadTarget] = useState<HTMLElement | null>(null);
  const [eligible, setEligible] = useState(false);
  const loadRef = useCallback<ThumbnailLoadRef>((node) => setLoadTarget(node), []);

  useEffect(() => {
    if (!loadTarget || source || failed || !asset.thumbnailAvailable) return undefined;
    if (typeof IntersectionObserver === "undefined") return undefined;

    const root = loadTarget.closest<HTMLElement>(".grid-workspace-results");
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setEligible(true);
          observer.disconnect();
        }
      },
      {
        root,
        rootMargin: root ? "720px 0px" : "240px",
        threshold: 0,
      },
    );
    observer.observe(loadTarget);
    return () => observer.disconnect();
  }, [asset.thumbnailAvailable, failed, loadTarget, source]);

  useEffect(() => {
    let active = true;
    if (
      !asset.thumbnailAvailable ||
      source ||
      failed ||
      (typeof IntersectionObserver !== "undefined" && !eligible)
    ) {
      return undefined;
    }

    void requestThumbnail(asset.id, 1)
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
  }, [asset.id, asset.thumbnailAvailable, eligible, failed, source]);

  return { source, failed, loadRef };
}
