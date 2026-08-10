import { useEffect, useId, useMemo, useState } from "react";

import type { AssetListItem } from "../types";
import { useThumbnailSource } from "./thumbnailSource";

const HISTOGRAM_BIN_COUNT = 64;
const HISTOGRAM_WIDTH = 256;
const HISTOGRAM_HEIGHT = 64;

export type HistogramChannel = "luma" | "red" | "green" | "blue";

type HistogramData = Record<HistogramChannel, number[]>;
type HistogramSelection = HistogramChannel | "all";

const histogramChannels: Array<{ id: HistogramChannel; label: string; className: string }> = [
  { id: "luma", label: "L", className: "is-luma" },
  { id: "red", label: "R", className: "is-red" },
  { id: "green", label: "G", className: "is-green" },
  { id: "blue", label: "B", className: "is-blue" },
];

function createEmptyHistogram(): HistogramData {
  return {
    luma: Array.from({ length: HISTOGRAM_BIN_COUNT }, () => 0),
    red: Array.from({ length: HISTOGRAM_BIN_COUNT }, () => 0),
    green: Array.from({ length: HISTOGRAM_BIN_COUNT }, () => 0),
    blue: Array.from({ length: HISTOGRAM_BIN_COUNT }, () => 0),
  };
}

function histogramDisplayMax(values: number[]) {
  const inner = values.slice(2, -2);
  return Math.max(...inner, 1);
}

function histogramPath(values: number[]) {
  const max = histogramDisplayMax(values);
  const points = values.map((value, index) => ({
    x: (index / (values.length - 1)) * HISTOGRAM_WIDTH,
    y: HISTOGRAM_HEIGHT - (value / max) * (HISTOGRAM_HEIGHT - 2),
  }));
  let path = `M 0 ${HISTOGRAM_HEIGHT}`;
  for (const point of points) path += ` L ${point.x.toFixed(2)} ${point.y.toFixed(2)}`;
  return `${path} L ${HISTOGRAM_WIDTH} ${HISTOGRAM_HEIGHT} Z`;
}

function calculateHistogram(source: string): Promise<HistogramData | null> {
  return new Promise((resolve) => {
    const image = new Image();
    image.onload = () => {
      const sourceWidth = image.naturalWidth || image.width;
      const sourceHeight = image.naturalHeight || image.height;
      if (!sourceWidth || !sourceHeight) {
        resolve(null);
        return;
      }

      const scale = Math.min(256 / sourceWidth, 256 / sourceHeight, 1);
      const width = Math.max(1, Math.round(sourceWidth * scale));
      const height = Math.max(1, Math.round(sourceHeight * scale));
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;

      try {
        const context = canvas.getContext("2d", { willReadFrequently: true });
        if (!context) {
          resolve(null);
          return;
        }
        context.drawImage(image, 0, 0, width, height);
        const pixels = context.getImageData(0, 0, width, height).data;
        const histogram = createEmptyHistogram();
        for (let index = 0; index < pixels.length; index += 4) {
          if (pixels[index + 3] === 0) continue;
          const red = pixels[index];
          const green = pixels[index + 1];
          const blue = pixels[index + 2];
          const luma = Math.round(0.2126 * red + 0.7152 * green + 0.0722 * blue);
          histogram.luma[Math.min(HISTOGRAM_BIN_COUNT - 1, (luma * HISTOGRAM_BIN_COUNT) >> 8)] += 1;
          histogram.red[(red * HISTOGRAM_BIN_COUNT) >> 8] += 1;
          histogram.green[(green * HISTOGRAM_BIN_COUNT) >> 8] += 1;
          histogram.blue[(blue * HISTOGRAM_BIN_COUNT) >> 8] += 1;
        }
        resolve(histogram);
      } catch {
        resolve(null);
      }
    };
    image.onerror = () => resolve(null);
    image.src = source;
  });
}

export function ImageHistogram({ asset }: { asset: AssetListItem }) {
  const { source, failed } = useThumbnailSource(asset);
  const gradientId = `histogram-gradient-${useId().replaceAll(":", "")}`;
  const [histogramState, setHistogramState] = useState<{
    source: string;
    data: HistogramData | null;
  } | null>(null);
  const [selectedChannel, setSelectedChannel] = useState<HistogramSelection>("all");

  useEffect(() => {
    let active = true;
    if (!source) return undefined;
    void calculateHistogram(source).then((result) => {
      if (active) setHistogramState({ source, data: result });
    });
    return () => {
      active = false;
    };
  }, [asset.id, source]);

  const data = histogramState?.source === source ? histogramState.data : null;
  const paths = useMemo(() => {
    if (!data) return null;
    return {
      luma: histogramPath(data.luma),
      red: histogramPath(data.red),
      green: histogramPath(data.green),
      blue: histogramPath(data.blue),
    };
  }, [data]);

  function isChannelVisible(channel: HistogramChannel) {
    return selectedChannel === "all" || selectedChannel === channel;
  }

  const status = !asset.thumbnailAvailable
    ? "暂无缩略图"
    : failed
      ? "缩略图不可用"
      : source && data
        ? null
        : "正在生成";

  return (
    <div className="image-histogram" data-source="thumbnail">
      <div className="histogram-toolbar">
        <span>通道</span>
        <div className="histogram-channel-controls" role="group" aria-label="直方图通道">
          <button
            type="button"
            className={`is-all${selectedChannel === "all" ? " is-active" : ""}`}
            aria-label="显示全部通道"
            aria-pressed={selectedChannel === "all"}
            onClick={() => setSelectedChannel("all")}
          >
            A
          </button>
          {histogramChannels.map((channel) => (
            <button
              type="button"
              key={channel.id}
              className={`${channel.className}${selectedChannel === channel.id ? " is-active" : ""}`}
              aria-label={`显示${channel.label}通道`}
              aria-pressed={selectedChannel === channel.id}
              onClick={() => setSelectedChannel(channel.id)}
            >
              {channel.label}
            </button>
          ))}
        </div>
      </div>
      <div className="histogram-chart" role="img" aria-label="直方图">
        {paths ? (
          <svg viewBox={`0 0 ${HISTOGRAM_WIDTH} ${HISTOGRAM_HEIGHT}`} preserveAspectRatio="none">
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="rgba(214, 224, 232, 0.62)" />
                <stop offset="100%" stopColor="rgba(214, 224, 232, 0.1)" />
              </linearGradient>
            </defs>
            <g className="histogram-grid-lines">
              <line x1="64" y1="0" x2="64" y2={HISTOGRAM_HEIGHT} />
              <line x1="128" y1="0" x2="128" y2={HISTOGRAM_HEIGHT} />
              <line x1="192" y1="0" x2="192" y2={HISTOGRAM_HEIGHT} />
            </g>
            {isChannelVisible("luma") ? <path d={paths.luma} fill={`url(#${gradientId})`} /> : null}
            {isChannelVisible("red") ? <path d={paths.red} className="histogram-red" /> : null}
            {isChannelVisible("green") ? (
              <path d={paths.green} className="histogram-green" />
            ) : null}
            {isChannelVisible("blue") ? <path d={paths.blue} className="histogram-blue" /> : null}
          </svg>
        ) : (
          <span className="histogram-status">{status}</span>
        )}
      </div>
      <div className="histogram-scale" aria-hidden="true">
        <span>阴影</span>
        <span>中间调</span>
        <span>高光</span>
      </div>
    </div>
  );
}
