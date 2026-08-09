import { useEffect, useRef, useState } from "react";

export type NavigatorFrame = {
  left: number;
  top: number;
  width: number;
  height: number;
};

export interface PreviewNavigatorProps {
  source: string | null;
  naturalSize: { width: number; height: number };
  frame: NavigatorFrame;
  zoomLabel: string;
  onCenterAt: (point: { x: number; y: number }) => void;
  placement?: "canvas" | "panel";
}

const NAVIGATOR_MAP_WIDTH = 260;
const NAVIGATOR_MAP_HEIGHT = 146;

export function PreviewNavigator({
  source,
  naturalSize,
  frame,
  zoomLabel,
  onCenterAt,
  placement = "canvas",
}: PreviewNavigatorProps) {
  const mapRef = useRef<HTMLDivElement | null>(null);
  const [dragging, setDragging] = useState(false);
  const [mapSize, setMapSize] = useState({
    width: NAVIGATOR_MAP_WIDTH,
    height: NAVIGATOR_MAP_HEIGHT,
  });

  useEffect(() => {
    const map = mapRef.current;
    if (!map) return undefined;
    const updateSize = () => {
      const rect = map.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        setMapSize({ width: rect.width, height: rect.height });
      }
    };
    updateSize();
    if (typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(updateSize);
    observer.observe(map);
    return () => observer.disconnect();
  }, [placement]);

  const imageAspect = naturalSize.width / Math.max(naturalSize.height, 1);
  const mapAspect = mapSize.width / Math.max(mapSize.height, 1);
  const imageArea =
    imageAspect >= mapAspect
      ? { width: mapSize.width, height: mapSize.width / imageAspect }
      : { width: mapSize.height * imageAspect, height: mapSize.height };

  function pointFromEvent(event: React.PointerEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(1, (event.clientX - rect.left) / Math.max(rect.width, 1))),
      y: Math.max(0, Math.min(1, (event.clientY - rect.top) / Math.max(rect.height, 1))),
    };
  }

  return (
    <section
      className={`preview-navigator${placement === "panel" ? " is-panel" : ""}`}
      aria-label="图片导航"
    >
      <div className="preview-navigator-heading">
        <strong>导航</strong>
        <span>当前视口</span>
      </div>
      <div ref={mapRef} className="preview-navigator-map" aria-label="图片导航图" role="group">
        <div
          className={`preview-navigator-image-area${dragging ? " is-dragging" : ""}`}
          style={{ width: `${imageArea.width}px`, height: `${imageArea.height}px` }}
          onPointerDown={(event) => {
            event.stopPropagation();
            event.currentTarget.setPointerCapture(event.pointerId);
            setDragging(true);
            onCenterAt(pointFromEvent(event));
          }}
          onPointerMove={(event) => {
            if (dragging) onCenterAt(pointFromEvent(event));
          }}
          onPointerUp={(event) => {
            event.stopPropagation();
            if (event.currentTarget.hasPointerCapture(event.pointerId))
              event.currentTarget.releasePointerCapture(event.pointerId);
            setDragging(false);
          }}
          onPointerCancel={() => setDragging(false)}
        >
          {source ? (
            <img className="preview-navigator-image" src={source} alt="" draggable={false} />
          ) : (
            <div className="preview-navigator-placeholder">正在加载</div>
          )}
          <div
            className="preview-navigator-viewport"
            aria-hidden="true"
            style={{
              left: `${frame.left * 100}%`,
              top: `${frame.top * 100}%`,
              width: `${frame.width * 100}%`,
              height: `${frame.height * 100}%`,
            }}
          />
        </div>
      </div>
      <div className="preview-navigator-zoom-label">{zoomLabel}</div>
    </section>
  );
}
