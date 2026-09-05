import { useCallback, useEffect, useState } from "react";

import { fetchPreview, fetchThumbnail } from "../api";
import type { AssetListItem } from "../types";

export type GridThumbnailProfile =
  | { key: "thumbnail"; maxWidth: null; maxHeight: null }
  | { key: "grid-768"; maxWidth: 768; maxHeight: 576 }
  | { key: "grid-960"; maxWidth: 960; maxHeight: 720 }
  | { key: "grid-1280"; maxWidth: 1280; maxHeight: 960 };

const THUMBNAIL_PROFILE: GridThumbnailProfile = {
  key: "thumbnail",
  maxWidth: null,
  maxHeight: null,
};
const GRID_768_PROFILE: GridThumbnailProfile = {
  key: "grid-768",
  maxWidth: 768,
  maxHeight: 576,
};
const GRID_960_PROFILE: GridThumbnailProfile = {
  key: "grid-960",
  maxWidth: 960,
  maxHeight: 720,
};
const GRID_1280_PROFILE: GridThumbnailProfile = {
  key: "grid-1280",
  maxWidth: 1280,
  maxHeight: 960,
};

const thumbnailCache = new Map<number, string>();
const thumbnailRequests = new Map<number, Promise<string>>();
const queuedThumbnailRequests = new Map<number, PendingThumbnailRequest>();
const thumbnailQueue: PendingThumbnailRequest[] = [];
const previewCache = new Map<string, string>();
const previewRequests = new Map<string, Promise<string>>();
const queuedPreviewRequests = new Map<string, PendingPreviewRequest>();
const previewQueue: PendingPreviewRequest[] = [];
const MAX_CONCURRENT_THUMBNAIL_REQUESTS = 6;
const MAX_CONCURRENT_PREVIEW_REQUESTS = 2;
const MAX_CACHED_GRID_PREVIEWS = 48;
let activeThumbnailRequests = 0;
let activePreviewRequests = 0;
let thumbnailRequestSequence = 0;
let previewRequestSequence = 0;

type PendingThumbnailRequest = {
  assetId: number;
  priority: number;
  sequence: number;
  resolve: (source: string) => void;
  reject: (reason: unknown) => void;
};

type PendingPreviewRequest = {
  key: string;
  assetId: number;
  profile: Exclude<GridThumbnailProfile, { key: "thumbnail" }>;
  sequence: number;
  resolve: (source: string) => void;
  reject: (reason: unknown) => void;
};

export type ThumbnailLoadRef = (node: HTMLElement | null) => void;

export function gridThumbnailProfileForColumns(columns: number): GridThumbnailProfile {
  if (columns <= 2) return GRID_1280_PROFILE;
  if (columns <= 4) return GRID_960_PROFILE;
  if (columns < 8) return GRID_768_PROFILE;
  return THUMBNAIL_PROFILE;
}

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

function drainPreviewQueue() {
  while (activePreviewRequests < MAX_CONCURRENT_PREVIEW_REQUESTS && previewQueue.length) {
    const pending = previewQueue.shift();
    if (!pending) return;
    queuedPreviewRequests.delete(pending.key);
    activePreviewRequests += 1;

    void fetchPreview(
      pending.assetId,
      "screen",
      pending.profile.maxWidth,
      pending.profile.maxHeight,
    )
      .then((source) => {
        rememberPreview(pending.key, source);
        pending.resolve(source);
      })
      .catch((reason: unknown) => pending.reject(reason))
      .finally(() => {
        activePreviewRequests -= 1;
        previewRequests.delete(pending.key);
        drainPreviewQueue();
      });
  }
}

function rememberPreview(key: string, source: string) {
  previewCache.delete(key);
  previewCache.set(key, source);
  while (previewCache.size > MAX_CACHED_GRID_PREVIEWS) {
    const oldestKey = previewCache.keys().next().value;
    if (typeof oldestKey !== "string") return;
    previewCache.delete(oldestKey);
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

export function requestGridPreview(
  assetId: number,
  profile: Exclude<GridThumbnailProfile, { key: "thumbnail" }>,
) {
  const key = `${assetId}:${profile.key}`;
  const cached = previewCache.get(key);
  if (cached) {
    rememberPreview(key, cached);
    return Promise.resolve(cached);
  }
  const inFlight = previewRequests.get(key);
  if (inFlight) return inFlight;

  const request = new Promise<string>((resolve, reject) => {
    const pending: PendingPreviewRequest = {
      key,
      assetId,
      profile,
      sequence: previewRequestSequence++,
      resolve,
      reject,
    };
    queuedPreviewRequests.set(key, pending);
    previewQueue.push(pending);
    previewQueue.sort((left, right) => left.sequence - right.sequence);
  });
  previewRequests.set(key, request);
  drainPreviewQueue();
  return request;
}

export function cancelGridPreview(
  assetId: number,
  profile: Exclude<GridThumbnailProfile, { key: "thumbnail" }>,
) {
  const key = `${assetId}:${profile.key}`;
  const pending = queuedPreviewRequests.get(key);
  if (!pending) return false;
  queuedPreviewRequests.delete(key);
  previewRequests.delete(key);
  const index = previewQueue.indexOf(pending);
  if (index >= 0) previewQueue.splice(index, 1);
  pending.reject(new Error("grid preview request left the viewport"));
  return true;
}
export function useThumbnailSource(
  asset: AssetListItem,
  profile: GridThumbnailProfile = THUMBNAIL_PROFILE,
) {
  const [thumbnailState, setThumbnailState] = useState<{
    assetId: number;
    source: string | null;
    failed: boolean;
  }>(() => ({
    assetId: asset.id,
    source: thumbnailCache.get(asset.id) ?? null,
    failed: false,
  }));
  const [enhancedSource, setEnhancedSource] = useState<{
    assetId: number;
    key: string;
    source: string;
  } | null>(null);
  const [loadTarget, setLoadTarget] = useState<HTMLElement | null>(null);
  const [eligible, setEligible] = useState(false);
  const [visibleProfileKey, setVisibleProfileKey] = useState<string | null>(null);
  const loadRef = useCallback<ThumbnailLoadRef>((node) => setLoadTarget(node), []);
  const source =
    thumbnailState.assetId === asset.id
      ? thumbnailState.source
      : (thumbnailCache.get(asset.id) ?? null);
  const failed = thumbnailState.assetId === asset.id ? thumbnailState.failed : false;

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
  }, [asset.id, asset.thumbnailAvailable, failed, loadTarget, source]);

  useEffect(() => {
    if (!loadTarget || profile.key === "thumbnail" || typeof IntersectionObserver === "undefined") {
      return undefined;
    }
    const root = loadTarget.closest<HTMLElement>(".grid-workspace-results");
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.some((entry) => entry.isIntersecting);
        setVisibleProfileKey(visible ? profile.key : null);
        if (!visible) setEnhancedSource(null);
      },
      { root, rootMargin: root ? "96px 0px" : "48px", threshold: 0 },
    );
    observer.observe(loadTarget);
    return () => observer.disconnect();
  }, [asset.id, loadTarget, profile.key]);

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
        setThumbnailState({ assetId: asset.id, source: value, failed: false });
      })
      .catch(() => {
        if (active) setThumbnailState({ assetId: asset.id, source: null, failed: true });
      });
    return () => {
      active = false;
    };
  }, [asset.id, asset.thumbnailAvailable, eligible, failed, source]);

  useEffect(() => {
    let active = true;
    if (profile.key === "thumbnail" || visibleProfileKey !== profile.key || !source) {
      return undefined;
    }
    void requestGridPreview(asset.id, profile)
      .then((value) => {
        if (active) setEnhancedSource({ assetId: asset.id, key: profile.key, source: value });
      })
      .catch(() => {
        // The bounded grid preview is an optional clarity upgrade. Keep the
        // analysis thumbnail visible if generation fails.
      });
    return () => {
      active = false;
      cancelGridPreview(asset.id, profile);
    };
  }, [asset.id, profile, source, visibleProfileKey]);

  const displaySource =
    visibleProfileKey === profile.key &&
    enhancedSource?.assetId === asset.id &&
    enhancedSource.key === profile.key
      ? enhancedSource.source
      : source;

  return { source: displaySource, failed, loadRef };
}
