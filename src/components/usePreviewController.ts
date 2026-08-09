import { useEffect, useMemo, useRef, useState } from "react";

import { fetchPreview } from "../api";
import type { AssetListItem } from "../types";
import { formatZoomPercent, smoothZoomLevel } from "./previewZoom";
import type { NavigatorFrame, PreviewNavigatorProps } from "./PreviewNavigator";

const PREVIEW_TIMEOUT_MS = 15_000;

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
  const assetId = asset?.id ?? null;
  const assetWidth = asset?.width ?? 1;
  const assetHeight = asset?.height ?? 1;

  useEffect(() => {
    const requestGeneration = ++generation.current;
    // The preview source is an async resource; clear it when the asset identity changes.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setScreenSource(null);
    setOriginalSource(null);
    setLoadState(assetId !== null ? "loading" : "loaded");
    setOriginalState("idle");
    setNaturalSize({ width: assetWidth, height: assetHeight });
    setFitScale(1);
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
        const original = await fetchPreview(assetId, "original");
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
  }, [assetHeight, assetId, assetWidth]);

  useEffect(() => {
    if (!active) return undefined;
    const stage = stageRef.current;
    if (!stage) return undefined;
    const updateFit = () => {
      const rect = stage.getBoundingClientRect();
      setStageSize({ width: rect.width, height: rect.height });
      setFitScale(
        Math.max(
          0.05,
          Math.min(
            1,
            (rect.width - 32) / naturalSize.width,
            (rect.height - 32) / naturalSize.height,
          ),
        ),
      );
      if (zoom === "fit") setOffset({ x: 0, y: 0 });
    };
    updateFit();
    if (typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(updateFit);
    observer.observe(stage);
    return () => observer.disconnect();
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
    void fetchPreview(asset.id, "original")
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
    if (zoom !== "fit" && typeof zoom === "number" && zoom >= 1) {
      zoomAround("fit");
      return;
    }
    zoomAround(1);
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
    onImageLoad: setNaturalSize,
    onWheel,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel: () => setDragging(false),
    onDoubleClick,
  };
}
