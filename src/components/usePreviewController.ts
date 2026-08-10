import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { fetchPreview } from "../api";
import type { AssetListItem } from "../types";
import { formatZoomPercent, smoothZoomLevel } from "./previewZoom";
import type { NavigatorFrame, PreviewNavigatorProps } from "./PreviewNavigator";

const PREVIEW_TIMEOUT_MS = 15_000;
const ORIGINAL_PREFETCH_DELAY_MS = 300;
const MAX_ORIGINAL_CACHE_ENTRIES = 3;
const MAX_ORIGINAL_CACHE_BYTES = 96 * 1024 * 1024;

type CachedOriginal = {
  version: string;
  source: string;
  estimatedBytes: number;
};

function previewAssetVersion(asset: AssetListItem) {
  return `${asset.absolutePath}\u0000${asset.fileSize}\u0000${asset.modifiedAt}`;
}

function calculateFitScale(
  rect: { width: number; height: number } | null,
  size: { width: number; height: number },
) {
  if (
    !rect ||
    !Number.isFinite(rect.width) ||
    !Number.isFinite(rect.height) ||
    rect.width <= 0 ||
    rect.height <= 0 ||
    !Number.isFinite(size.width) ||
    !Number.isFinite(size.height) ||
    size.width <= 0 ||
    size.height <= 0
  ) {
    return null;
  }
  return Math.max(
    0.05,
    Math.min(1, (rect.width - 32) / size.width, (rect.height - 32) / size.height),
  );
}

export interface PreviewController {
  asset: AssetListItem | null;
  stageRef: React.RefObject<HTMLDivElement | null>;
  screenSource: string | null;
  displaySource: string | null;
  loadState: "loading" | "loaded" | "error";
  originalState: "idle" | "loading" | "loaded" | "error";
  naturalSize: { width: number; height: number };
  fitScale: number;
  displayScale: number;
  zoom: number | "fit";
  offset: { x: number; y: number };
  dragging: boolean;
  zoomLabel: string;
  navigatorFrame: NavigatorFrame;
  navigator: PreviewNavigatorProps | null;
  onImageLoad: (size: { width: number; height: number }) => void;
  onWheel: (event: React.WheelEvent<HTMLDivElement>) => void;
  onPointerDown: (event: React.PointerEvent<HTMLDivElement>) => void;
  onPointerMove: (event: React.PointerEvent<HTMLDivElement>) => void;
  onPointerUp: (event: React.PointerEvent<HTMLDivElement>) => void;
  onPointerCancel: () => void;
  onDoubleClick: (event: React.MouseEvent<HTMLDivElement>) => void;
}

export function usePreviewController(
  asset: AssetListItem | null,
  active: boolean,
  prefetchAssets: AssetListItem[] = [],
): PreviewController {
  const stageRef = useRef<HTMLDivElement | null>(null);
  const [screenSource, setScreenSource] = useState<string | null>(null);
  const [originalSource, setOriginalSource] = useState<string | null>(null);
  const [loadState, setLoadState] = useState<"loading" | "loaded" | "error">("loading");
  const [originalState, setOriginalState] = useState<"idle" | "loading" | "loaded" | "error">(
    "idle",
  );
  const [naturalSize, setNaturalSize] = useState({ width: 1, height: 1 });
  const [stageSize, setStageSize] = useState({ width: 0, height: 0 });
  const [fitScale, setFitScale] = useState(1);
  const [zoom, setZoom] = useState<number | "fit">("fit");
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const generation = useRef(0);
  const assetRef = useRef<AssetListItem | null>(asset);
  const currentAssetIdRef = useRef<number | null>(null);
  const originalCacheRef = useRef(new Map<number, CachedOriginal>());
  const originalCacheBytesRef = useRef(0);
  const originalInFlightRef = useRef(new Map<string, Promise<string>>());
  const assetId = asset?.id ?? null;
  const assetWidth = asset?.width && asset.width > 0 ? asset.width : 1;
  const assetHeight = asset?.height && asset.height > 0 ? asset.height : 1;
  const assetVersionKey = asset ? previewAssetVersion(asset) : "";
  assetRef.current = asset;
  currentAssetIdRef.current = assetId;

  const loadOriginalForAsset = useCallback((target: AssetListItem): Promise<string> => {
    const version = previewAssetVersion(target);
    const cached = originalCacheRef.current.get(target.id);
    if (cached?.version === version) {
      originalCacheRef.current.delete(target.id);
      originalCacheRef.current.set(target.id, cached);
      return Promise.resolve(cached.source);
    }
    if (cached) {
      originalCacheRef.current.delete(target.id);
      originalCacheBytesRef.current -= cached.estimatedBytes;
    }

    const requestKey = `${target.id}:${version}`;
    const inFlight = originalInFlightRef.current.get(requestKey);
    if (inFlight) return inFlight;

    const request = fetchPreview(target.id, "original")
      .then((source) => {
        const estimatedBytes = source.length * 2;
        if (estimatedBytes <= MAX_ORIGINAL_CACHE_BYTES) {
          const previous = originalCacheRef.current.get(target.id);
          if (previous) originalCacheBytesRef.current -= previous.estimatedBytes;
          originalCacheRef.current.delete(target.id);
          originalCacheRef.current.set(target.id, { version, source, estimatedBytes });
          originalCacheBytesRef.current += estimatedBytes;

          while (
            originalCacheRef.current.size > MAX_ORIGINAL_CACHE_ENTRIES ||
            originalCacheBytesRef.current > MAX_ORIGINAL_CACHE_BYTES
          ) {
            const oldestId = Array.from(originalCacheRef.current.keys()).find(
              (id) => id !== currentAssetIdRef.current,
            );
            const idToRemove = oldestId ?? originalCacheRef.current.keys().next().value;
            if (idToRemove === undefined) break;
            const removed = originalCacheRef.current.get(idToRemove);
            if (removed) originalCacheBytesRef.current -= removed.estimatedBytes;
            originalCacheRef.current.delete(idToRemove);
          }
        }
        return source;
      })
      .finally(() => {
        originalInFlightRef.current.delete(requestKey);
      });
    originalInFlightRef.current.set(requestKey, request);
    return request;
  }, []);

  useLayoutEffect(() => {
    const requestGeneration = ++generation.current;
    // The preview source is an async resource; clear it when the asset identity changes.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setScreenSource(null);
    setOriginalSource(null);
    setLoadState(assetId !== null ? "loading" : "loaded");
    setOriginalState("idle");
    setNaturalSize({ width: assetWidth, height: assetHeight });
    const measuredFitScale = calculateFitScale(stageRef.current?.getBoundingClientRect() ?? null, {
      width: assetWidth,
      height: assetHeight,
    });
    if (measuredFitScale !== null) setFitScale(measuredFitScale);
    setZoom("fit");
    setOffset({ x: 0, y: 0 });
    setDragging(false);

    if (assetId === null) return undefined;

    const timeout = window.setTimeout(() => {
      if (generation.current !== requestGeneration) return;
      generation.current += 1;
      setLoadState("error");
    }, PREVIEW_TIMEOUT_MS);
    void (async () => {
      try {
        const requestAsset = assetRef.current;
        if (!requestAsset) return;
        const original = await loadOriginalForAsset(requestAsset);
        if (generation.current !== requestGeneration) return;
        setOriginalSource(original);
        setScreenSource(original);
        setOriginalState("loaded");
        setLoadState("loaded");
      } catch {
        if (generation.current !== requestGeneration) return;
        setOriginalState("error");
        try {
          const fallback = await fetchPreview(assetId, "screen");
          if (generation.current !== requestGeneration) return;
          setScreenSource(fallback);
          setLoadState("loaded");
        } catch {
          if (generation.current === requestGeneration) setLoadState("error");
        }
      } finally {
        window.clearTimeout(timeout);
      }
    })();
    return () => {
      window.clearTimeout(timeout);
      generation.current += 1;
    };
  }, [assetHeight, assetId, assetVersionKey, assetWidth, loadOriginalForAsset]);

  useEffect(() => {
    const targets = active ? prefetchAssets.slice(1, 3) : prefetchAssets.slice(0, 1);
    if (targets.length === 0) return undefined;
    const timer = window.setTimeout(() => {
      targets.forEach((target) => {
        void loadOriginalForAsset(target).catch(() => undefined);
      });
    }, ORIGINAL_PREFETCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [active, assetId, loadOriginalForAsset, prefetchAssets]);

  useEffect(() => {
    if (!active) return undefined;
    const stage = stageRef.current;
    if (!stage) return undefined;
    const naturalWidth = naturalSize.width;
    const naturalHeight = naturalSize.height;
    const updateFit = () => {
      const rect = stage.getBoundingClientRect();
      const nextFitScale = calculateFitScale(rect, {
        width: naturalWidth,
        height: naturalHeight,
      });
      if (nextFitScale === null) return;
      setStageSize({ width: rect.width, height: rect.height });
      setFitScale(nextFitScale);
      if (zoom === "fit") setOffset({ x: 0, y: 0 });
    };
    updateFit();
    window.addEventListener("resize", updateFit);
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(updateFit);
    observer?.observe(stage);
    return () => {
      window.removeEventListener("resize", updateFit);
      observer?.disconnect();
    };
  }, [active, naturalSize.height, naturalSize.width, zoom]);

  function currentScale() {
    return zoom === "fit" ? fitScale : zoom;
  }

  function constrainPan(nextOffset: { x: number; y: number }, scale = currentScale()) {
    const rect = stageRef.current?.getBoundingClientRect();
    if (!rect) return nextOffset;
    const maxX = Math.max(0, (naturalSize.width * scale - rect.width) / 2);
    const maxY = Math.max(0, (naturalSize.height * scale - rect.height) / 2);
    return {
      x: Math.max(-maxX, Math.min(maxX, nextOffset.x)),
      y: Math.max(-maxY, Math.min(maxY, nextOffset.y)),
    };
  }

  function loadOriginalIfNeeded() {
    if (!asset || originalSource || originalState === "loading" || originalState === "loaded")
      return;
    const requestGeneration = generation.current;
    setOriginalState("loading");
    void loadOriginalForAsset(asset)
      .then((value) => {
        if (generation.current !== requestGeneration) return;
        setOriginalSource(value);
        setOriginalState("loaded");
      })
      .catch(() => {
        if (generation.current === requestGeneration) setOriginalState("error");
      });
  }

  function zoomAround(next: number | "fit", anchor = { x: 0, y: 0 }) {
    if (next === "fit") {
      setZoom("fit");
      setOffset({ x: 0, y: 0 });
      updateFitMeasurement();
      return;
    }
    const previousScale = currentScale();
    const ratio = next / Math.max(previousScale, 0.0001);
    const nextOffset = {
      x: anchor.x - (anchor.x - offset.x) * ratio,
      y: anchor.y - (anchor.y - offset.y) * ratio,
    };
    setZoom(next);
    setOffset(next < fitScale ? { x: 0, y: 0 } : nextOffset);
    if (next > 1) loadOriginalIfNeeded();
  }

  function onWheel(event: React.WheelEvent<HTMLDivElement>) {
    event.preventDefault();
    const oldScale = currentScale();
    const deltaY =
      event.deltaMode === 1
        ? event.deltaY * 16
        : event.deltaMode === 2
          ? event.deltaY * Math.max(event.currentTarget.clientHeight, 1)
          : event.deltaY;
    const nextScale = smoothZoomLevel(oldScale, deltaY);
    zoomAround(nextScale);
  }

  function onPointerDown(event: React.PointerEvent<HTMLDivElement>) {
    if (currentScale() <= fitScale) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
    setDragStart({ x: event.clientX - offset.x, y: event.clientY - offset.y });
  }

  function onPointerMove(event: React.PointerEvent<HTMLDivElement>) {
    if (dragging)
      setOffset(constrainPan({ x: event.clientX - dragStart.x, y: event.clientY - dragStart.y }));
  }

  function onPointerUp(event: React.PointerEvent<HTMLDivElement>) {
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId);
    setDragging(false);
  }

  function onDoubleClick() {
    if (zoom === "fit") {
      zoomAround(1);
    } else {
      zoomAround("fit");
    }
  }

  function updateFitMeasurement(size = naturalSize) {
    const rect = stageRef.current?.getBoundingClientRect();
    const nextFitScale = calculateFitScale(rect ?? null, size);
    if (nextFitScale === null) return;
    setStageSize({ width: rect!.width, height: rect!.height });
    setFitScale(nextFitScale);
    if (zoom === "fit") setOffset({ x: 0, y: 0 });
  }

  function onImageLoad(size: { width: number; height: number }) {
    if (!Number.isFinite(size.width) || !Number.isFinite(size.height)) return;
    const nextSize = {
      width: Math.max(1, size.width),
      height: Math.max(1, size.height),
    };
    setNaturalSize(nextSize);
    updateFitMeasurement(nextSize);
  }

  function centerNavigatorAt(point: { x: number; y: number }) {
    const scale = currentScale();
    setOffset(
      constrainPan(
        {
          x: (0.5 - point.x) * naturalSize.width * scale,
          y: (0.5 - point.y) * naturalSize.height * scale,
        },
        scale,
      ),
    );
  }

  const displayScale = currentScale();
  const displaySource =
    zoom !== "fit" && zoom >= 1 && originalSource ? originalSource : screenSource;
  const zoomLabel = zoom === "fit" ? formatZoomPercent(fitScale) : formatZoomPercent(zoom);
  const navigatorFrame = useMemo(() => {
    const scale = Math.max(displayScale, 0.0001);
    const imageWidth = Math.max(naturalSize.width, 1) * scale;
    const imageHeight = Math.max(naturalSize.height, 1) * scale;
    const viewportWidth = stageSize.width || imageWidth;
    const viewportHeight = stageSize.height || imageHeight;
    const width = Math.min(1, viewportWidth / imageWidth);
    const height = Math.min(1, viewportHeight / imageHeight);
    const centerX = 0.5 - offset.x / imageWidth;
    const centerY = 0.5 - offset.y / imageHeight;
    return {
      left: Math.max(0, Math.min(1 - width, centerX - width / 2)),
      top: Math.max(0, Math.min(1 - height, centerY - height / 2)),
      width,
      height,
    };
  }, [displayScale, naturalSize.height, naturalSize.width, offset.x, offset.y, stageSize]);

  const navigator = asset
    ? {
        source: screenSource,
        naturalSize,
        frame: navigatorFrame,
        zoomLabel,
        onCenterAt: centerNavigatorAt,
      }
    : null;

  return {
    asset,
    stageRef,
    screenSource,
    displaySource,
    loadState,
    originalState,
    naturalSize,
    fitScale,
    displayScale,
    zoom,
    offset,
    dragging,
    zoomLabel,
    navigatorFrame,
    navigator,
    onImageLoad,
    onWheel,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel: () => setDragging(false),
    onDoubleClick,
  };
}
