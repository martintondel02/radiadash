import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import type { Peak, SpectrumPayload } from "../lib/types";

export function energyOf(spectrum: SpectrumPayload, channel: number) {
  return spectrum.a0 + spectrum.a1 * channel + spectrum.a2 * channel * channel;
}

export function channelForEnergy(spectrum: SpectrumPayload, energyKeV: number) {
  const { a0, a1, a2 } = spectrum;
  if (Math.abs(a2) < 1e-9) return (energyKeV - a0) / a1;
  const disc = a1 * a1 - 4 * a2 * (a0 - energyKeV);
  if (disc < 0) return Number.NaN;
  const root = Math.sqrt(disc);
  const first = (-a1 + root) / (2 * a2);
  const second = (-a1 - root) / (2 * a2);
  return first > 0 && first < spectrum.counts.length ? first : second;
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

export interface PeakMatch {
  id: string;
  confidence: string;
  delta: number;
  intensity: number;
}

export type PeakMatches = Map<number, PeakMatch[]>;

export function useViewWindow() {
  const [view, setView] = useState({ start: 0, end: 1 });

  const zoom = useCallback((factor: number, anchor = 0.5) => {
    setView((current) => {
      const span = current.end - current.start;
      const nextSpan = clamp(span * factor, 0.002, 1);
      const start = clamp(current.start + (span - nextSpan) * anchor, 0, 1 - nextSpan);
      return { start, end: start + nextSpan };
    });
  }, []);

  const pan = useCallback((fraction: number) => {
    setView((current) => {
      const span = current.end - current.start;
      const start = clamp(current.start + fraction * span, 0, 1 - span);
      return { start, end: start + span };
    });
  }, []);

  const reset = useCallback(() => setView({ start: 0, end: 1 }), []);

  return { view, setView, zoom, pan, reset };
}

function useWheelZoom(ref: RefObject<HTMLDivElement | null>, zoom: (factor: number, anchor: number) => void) {
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const handler = (event: WheelEvent) => {
      if (event.deltaY === 0) return;
      event.preventDefault();
      const rect = element.getBoundingClientRect();
      const anchor = clamp((event.clientX - rect.left) / rect.width, 0, 1);
      const factor = event.deltaY > 0 ? 1.18 : 1 / 1.18;
      zoom(factor, anchor);
    };
    element.addEventListener("wheel", handler, { passive: false });
    return () => element.removeEventListener("wheel", handler);
  }, [ref, zoom]);
}

function useDragPan(
  ref: RefObject<HTMLDivElement | null>,
  view: { start: number; end: number },
  setView: (fn: (current: { start: number; end: number }) => { start: number; end: number }) => void,
) {
  const drag = useRef<{ x: number; start: number; span: number } | null>(null);

  const onPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    drag.current = { x: event.clientX, start: view.start, span: view.end - view.start };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const onPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current || !ref.current) return;
    const width = ref.current.getBoundingClientRect().width || 1;
    const delta = ((event.clientX - drag.current.x) / width) * drag.current.span;
    const span = drag.current.span;
    const start = clamp(drag.current.start - delta, 0, 1 - span);
    setView(() => ({ start, end: start + span }));
  };

  const onPointerUp = () => {
    drag.current = null;
  };

  return { onPointerDown, onPointerMove, onPointerUp, onPointerCancel: onPointerUp, isDragging: () => drag.current != null };
}

export function ZoomControls({
  onZoomIn,
  onZoomOut,
  onReset,
}: {
  onZoomIn: () => void;
  onZoomOut: () => void;
  onReset: () => void;
}) {
  const button = "grid h-6 w-6 place-items-center rounded border border-[var(--border)] text-xs text-[var(--muted)] hover:text-[var(--text)] hover:border-[var(--accent-dim)]";
  return (
    <div className="flex items-center gap-1">
      <button className={button} onClick={onZoomOut} aria-label="Zoom out" title="Zoom out">
        −
      </button>
      <button className={button} onClick={onZoomIn} aria-label="Zoom in" title="Zoom in">
        +
      </button>
      <button className={button} onClick={onReset} aria-label="Reset zoom" title="Reset zoom">
        ⟲
      </button>
    </div>
  );
}

function barFill(ratio: number) {
  if (ratio > 0.75) return "#f0b429";
  if (ratio > 0.2) return "var(--accent)";
  if (ratio > 0.05) return "#5fd6c9";
  return "var(--accent-dim)";
}

function confidenceColor(confidence: string) {
  switch (confidence) {
    case "high":
      return "#4fd1c5";
    case "medium":
      return "#f0b429";
    default:
      return "#8ea1ad";
  }
}

function formatCounts(value: number) {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
  return `${Math.round(value)}`;
}

export function SpectrumChart({
  spectrum,
  height = 320,
  overlay,
  compact = false,
  peaks = [],
  matches,
  focusChannel,
}: {
  spectrum: SpectrumPayload | null;
  height?: number;
  overlay?: { lines: { energy: number; matched: boolean }[] };
  compact?: boolean;
  peaks?: Peak[];
  matches?: PeakMatches;
  focusChannel?: number | null;
}) {
  const container = useRef<HTMLDivElement>(null);
  const { view, setView, zoom, reset } = useViewWindow();
  useWheelZoom(container, zoom);
  const drag = useDragPan(container, view, setView);
  const dragging = useRef(false);

  const [logScale, setLogScale] = useState(true);
  const [hover, setHover] = useState<{ channel: number; peak: Peak | null; x: number } | null>(null);
  const [selected, setSelected] = useState<number | null>(null);

  const counts = spectrum?.counts ?? [];
  const lastChannel = Math.max(0, counts.length - 1);
  const c0 = Math.floor(view.start * lastChannel);
  const c1 = Math.ceil(view.end * lastChannel);
  const visible = counts.slice(c0, c1 + 1);
  const spanChannels = Math.max(1, c1 - c0);

  useEffect(() => {
    if (focusChannel == null || !spectrum) return;
    const peak = peaks.find((item) => item.channel === focusChannel);
    const fwhmChannels = peak ? peak.fwhm_kev / Math.max(0.1, spectrum.a1) : 20;
    const half = Math.max(12, fwhmChannels * 6) / lastChannel;
    const center = focusChannel / lastChannel;
    const start = clamp(center - half, 0, 1 - half * 2);
    setView({ start, end: clamp(start + half * 2, 0, 1) });
    setSelected(focusChannel);
  }, [focusChannel, spectrum, peaks, lastChannel, setView]);

  const maxCounts = useMemo(() => Math.max(...visible, 1), [visible]);

  const bars = useMemo(() => {
    if (visible.length === 0) return [] as { height: number; ratio: number }[];
    const denom = logScale ? Math.log10(1 + maxCounts) : maxCounts;
    return visible.map((value) => ({
      height: logScale ? Math.log10(1 + value) / denom : value / denom,
      ratio: value / maxCounts,
    }));
  }, [visible, maxCounts, logScale]);

  const ticks = useMemo(() => {
    if (!logScale) {
      return [0, 0.5, 1].map((fraction) => ({
        label: formatCounts(maxCounts * fraction),
        fraction,
      }));
    }
    const top = Math.floor(Math.log10(maxCounts));
    const out = [];
    for (let power = 0; power <= top; power += 1) {
      const value = 10 ** power;
      const fraction = Math.log10(1 + value) / Math.log10(1 + maxCounts);
      out.push({ label: formatCounts(value), fraction });
    }
    return out;
  }, [logScale, maxCounts]);

  if (!spectrum || bars.length === 0) {
    return <div className="flex h-44 items-center justify-center text-xs text-[var(--muted)]">no spectrum yet</div>;
  }

  const width = 1024;
  const step = width / bars.length;

  const nearestPeak = (channel: number): Peak | null => {
    let best: Peak | null = null;
    for (const peak of peaks) {
      const tolerance = Math.max(2, (peak.fwhm_kev / Math.max(0.1, spectrum.a1)) * 0.7);
      if (Math.abs(peak.channel - channel) <= tolerance) {
        if (!best || Math.abs(peak.channel - channel) < Math.abs(best.channel - channel)) best = peak;
      }
    }
    return best;
  };

  const hoverPeak = hover?.peak ?? null;
  const activePeak = hoverPeak ?? (selected != null ? peaks.find((item) => item.channel === selected) ?? null : null);
  const activeMatches = activePeak ? matches?.get(activePeak.channel) ?? [] : [];

  const zoomToPeak = (peak: Peak) => {
    const fwhmChannels = peak.fwhm_kev / Math.max(0.1, spectrum.a1);
    const half = Math.max(15, fwhmChannels * 8) / lastChannel;
    const center = peak.channel / lastChannel;
    const start = clamp(center - half, 0, 1 - half * 2);
    setView({ start, end: clamp(start + half * 2, 0, 1) });
  };

  return (
    <div>
      {compact ? (
        <div className="mb-1 flex items-center justify-between">
          <span className="text-[9px] text-[var(--muted)]">scroll / +− to zoom</span>
          <div className="flex items-center gap-1">
            <button
              onClick={() => setLogScale(!logScale)}
              className="rounded border border-[var(--border)] px-1.5 py-0.5 text-[9px] text-[var(--muted)] hover:text-[var(--text)]"
            >
              {logScale ? "log" : "lin"}
            </button>
            <ZoomControls onZoomIn={() => zoom(1 / 1.5)} onZoomOut={() => zoom(1.5)} onReset={reset} />
          </div>
        </div>
      ) : (
        <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
          <div className="flex flex-wrap items-center gap-3 text-[10px] text-[var(--muted)]">
            <span className="flex items-center gap-1">
              <span className="inline-block h-2 w-2 rounded-sm" style={{ background: "var(--accent-dim)" }} /> background
            </span>
            <span className="flex items-center gap-1">
              <span className="inline-block h-2 w-2 rounded-sm" style={{ background: "var(--accent)" }} /> counts
            </span>
            <span className="flex items-center gap-1">
              <span className="inline-block h-2 w-2 rounded-sm bg-[#f0b429]" /> peak
            </span>
            <span className="flex items-center gap-1">
              <span className="inline-block h-0.5 w-3 bg-[var(--amber)]" /> matched line
            </span>
            <span className="hidden sm:inline">scroll to zoom · drag to pan · hover a peak</span>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => setLogScale(!logScale)}
              className="rounded border border-[var(--border)] px-1.5 py-0.5 text-[10px] text-[var(--muted)] hover:text-[var(--text)]"
            >
              {logScale ? "log scale" : "linear"}
            </button>
            <ZoomControls onZoomIn={() => zoom(1 / 1.5)} onZoomOut={() => zoom(1.5)} onReset={reset} />
          </div>
        </div>
      )}

      <div className="mb-2 flex min-h-[28px] items-center justify-between gap-2 rounded border border-[var(--border)] bg-[var(--panel-2)] px-2 py-1 text-[11px]">
        {activePeak ? (
          <>
            <div className="num flex flex-wrap items-center gap-x-3 gap-y-0.5">
              <span className="text-[var(--text)]">{activePeak.energy_kev.toFixed(1)} keV</span>
              <span className="text-[var(--muted)]">net {Math.round(activePeak.net).toLocaleString()}</span>
              <span className="text-[var(--muted)]">{activePeak.significance.toFixed(1)}σ</span>
              {activeMatches.length > 0 ? (
                activeMatches.slice(0, 3).map((match) => (
                  <span key={match.id} className="flex items-center gap-1" style={{ color: confidenceColor(match.confidence) }}>
                    <span className="inline-block h-1.5 w-1.5 rounded-full" style={{ background: confidenceColor(match.confidence) }} />
                    {match.id}
                    <span className="text-[var(--muted)]">({match.delta >= 0 ? "+" : ""}{match.delta.toFixed(1)} keV)</span>
                  </span>
                ))
              ) : (
                <span className="text-[var(--muted)]">no isotope match</span>
              )}
            </div>
            <button
              onClick={() => zoomToPeak(activePeak)}
              className="shrink-0 rounded border border-[var(--accent-dim)] px-2 py-0.5 text-[10px] text-[var(--accent)]"
            >
              zoom to peak
            </button>
          </>
        ) : (
          <span className="text-[var(--muted)]">Hover a peak to identify it, click to pin it, double-click to zoom.</span>
        )}
      </div>

      <div
        ref={container}
        className="relative touch-none select-none"
        style={{ cursor: "crosshair" }}
        onPointerDown={(event) => {
          dragging.current = true;
          drag.onPointerDown(event);
        }}
        onPointerMove={(event) => {
          drag.onPointerMove(event);
          if (dragging.current || !container.current) return;
          const rect = container.current.getBoundingClientRect();
          const fraction = clamp((event.clientX - rect.left) / rect.width, 0, 1);
          const channel = Math.round(c0 + fraction * spanChannels);
          setHover({ channel, peak: nearestPeak(channel), x: fraction });
        }}
        onPointerUp={() => {
          dragging.current = false;
          drag.onPointerUp();
        }}
        onPointerCancel={() => {
          dragging.current = false;
          drag.onPointerCancel();
        }}
        onPointerLeave={() => setHover(null)}
        onClick={() => {
          if (hover?.peak) setSelected(hover.peak.channel);
        }}
        onDoubleClick={() => {
          if (hover?.peak) zoomToPeak(hover.peak);
        }}
      >
        <svg
          viewBox={`0 0 ${width} ${height}`}
          className="w-full"
          style={{ height }}
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          {ticks.map((tick) => (
            <line
              key={tick.label}
              x1="0"
              x2={width}
              y1={height - tick.fraction * (height - 6)}
              y2={height - tick.fraction * (height - 6)}
              stroke="var(--border)"
              strokeWidth="0.5"
              strokeDasharray="3 4"
              opacity="0.6"
            />
          ))}

          {bars.map((bar, index) => (
            <rect
              key={index}
              x={index * step}
              y={height - bar.height * (height - 6)}
              width={Math.max(1, step + 0.4)}
              height={bar.height * (height - 6)}
              fill={barFill(bar.ratio)}
              opacity={0.9}
            />
          ))}

          {peaks.map((peak) => {
            const matchList = matches?.get(peak.channel);
            if (!matchList || matchList.length === 0) return null;
            if (peak.channel < c0 || peak.channel > c1) return null;
            const x = ((peak.channel - c0) / spanChannels) * width;
            const best = matchList[0];
            return (
              <g key={`peak-${peak.channel}`}>
                <line
                  x1={x}
                  y1={4}
                  x2={x}
                  y2={height - 4}
                  stroke={confidenceColor(best.confidence)}
                  strokeWidth="0.6"
                  opacity="0.25"
                />
                <circle cx={x} cy={6} r={2.2} fill={confidenceColor(best.confidence)} opacity="0.9" />
              </g>
            );
          })}

          {overlay?.lines.map((line, index) => {
            const channel = channelForEnergy(spectrum, line.energy);
            if (!Number.isFinite(channel) || channel < c0 || channel > c1) return null;
            const x = ((channel - c0) / spanChannels) * width;
            return (
              <line
                key={`${line.energy}-${index}`}
                x1={x}
                y1={2}
                x2={x}
                y2={height * (line.matched ? 0.62 : 0.4)}
                stroke={line.matched ? "var(--amber)" : "var(--muted)"}
                strokeWidth={line.matched ? 1.5 : 0.9}
                strokeDasharray={line.matched ? undefined : "2 2"}
                opacity={line.matched ? 0.95 : 0.5}
              />
            );
          })}

          {activePeak && activePeak.channel >= c0 && activePeak.channel <= c1 && (
            <>
              <rect
                x={((activePeak.channel - c0) / spanChannels) * width - (activePeak.fwhm_kev / Math.max(0.1, spectrum.a1) / spanChannels) * width * 0.7}
                y={0}
                width={Math.max(3, (activePeak.fwhm_kev / Math.max(0.1, spectrum.a1) / spanChannels) * width * 1.4)}
                height={height}
                fill="var(--amber)"
                opacity="0.1"
              />
              <line
                x1={((activePeak.channel - c0) / spanChannels) * width}
                y1={0}
                x2={((activePeak.channel - c0) / spanChannels) * width}
                y2={height}
                stroke="var(--amber)"
                strokeWidth="1"
                opacity="0.7"
              />
            </>
          )}

          <line x1="0" y1={height - 0.5} x2={width} y2={height - 0.5} stroke="var(--border)" strokeWidth="0.6" />
        </svg>

        <div className="pointer-events-none absolute inset-0">
          {ticks.map((tick) => (
            <span
              key={`label-${tick.label}`}
              className="num absolute left-1 text-[9px] text-[var(--muted)]"
              style={{ top: `${(1 - tick.fraction) * 100}%`, transform: "translateY(-50%)" }}
            >
              {tick.label}
            </span>
          ))}
          {hover?.peak && (
            <div
              className="absolute z-10 -translate-x-1/2 rounded border border-[var(--border)] bg-[var(--panel)]/95 px-2 py-1 text-[10px] shadow-lg"
              style={{ left: `${clamp(hover.x, 0.12, 0.88) * 100}%`, top: 6 }}
            >
              <div className="num text-[var(--text)]">{hover.peak.energy_kev.toFixed(1)} keV</div>
              <div className="num text-[var(--muted)]">{Math.round(hover.peak.net).toLocaleString()} net</div>
            </div>
          )}
        </div>
      </div>

      <div className="mt-1 flex justify-between text-[10px] text-[var(--muted)]">
        <span>{energyOf(spectrum, c0).toFixed(0)} keV</span>
        <span>{energyOf(spectrum, Math.round((c0 + c1) / 2)).toFixed(0)} keV</span>
        <span>{energyOf(spectrum, c1).toFixed(0)} keV</span>
      </div>
    </div>
  );
}

export function TimeSeriesChart({
  values,
  times,
  height = 120,
  unit,
  color = "var(--accent)",
}: {
  values: number[];
  times?: number[];
  height?: number;
  unit?: string;
  color?: string;
}) {
  const container = useRef<HTMLDivElement>(null);
  const { view, setView, zoom, reset } = useViewWindow();
  useWheelZoom(container, zoom);
  const drag = useDragPan(container, view, setView);
  const dragging = useRef(false);
  const [hover, setHover] = useState<{ index: number; x: number } | null>(null);

  const lastIndex = Math.max(0, values.length - 1);
  const i0 = Math.floor(view.start * lastIndex);
  const i1 = Math.ceil(view.end * lastIndex);
  const slice = values.slice(i0, i1 + 1);

  const geometry = useMemo(() => {
    if (slice.length < 2) return null;
    const width = 600;
    const min = Math.min(...slice);
    const max = Math.max(...slice);
    const span = max - min || 1;
    const points = slice.map((value, index) => {
      const x = (index / (slice.length - 1)) * width;
      const y = height - 8 - ((value - min) / span) * (height - 18);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    return {
      width,
      line: `M${points.join(" L")}`,
      area: `M0,${height} L${points.join(" L")} L${width},${height} Z`,
      min,
      max,
      span,
    };
  }, [slice, height]);

  if (!geometry) {
    return <div className="flex h-28 items-center justify-center text-xs text-[var(--muted)]">waiting for samples…</div>;
  }

  const label = (index: number) => {
    if (times && times[index] != null) return new Date(times[index]).toLocaleTimeString();
    return `${index}`;
  };

  const hoverIndex = hover ? Math.min(i1, i0 + Math.round(hover.x * (i1 - i0))) : null;
  const hoverValue = hoverIndex != null ? values[hoverIndex] : null;
  const hoverX = hoverIndex != null ? ((hoverIndex - i0) / Math.max(1, i1 - i0)) * 100 : 0;

  return (
    <div>
      <div className="mb-1 flex items-center justify-between text-[10px] text-[var(--muted)]">
        <span className="num">
          {geometry.min.toFixed(2)}–{geometry.max.toFixed(2)} {unit}
        </span>
        <div className="flex items-center gap-2">
          <span className="hidden sm:inline">scroll to zoom · drag to pan</span>
          <ZoomControls onZoomIn={() => zoom(1 / 1.5)} onZoomOut={() => zoom(1.5)} onReset={reset} />
        </div>
      </div>
      <div
        ref={container}
        className="relative touch-none select-none"
        style={{ cursor: "crosshair" }}
        onPointerDown={(event) => {
          dragging.current = true;
          drag.onPointerDown(event);
        }}
        onPointerMove={(event) => {
          drag.onPointerMove(event);
          if (dragging.current || !container.current) return;
          const rect = container.current.getBoundingClientRect();
          setHover({ index: 0, x: clamp((event.clientX - rect.left) / rect.width, 0, 1) });
        }}
        onPointerUp={() => {
          dragging.current = false;
          drag.onPointerUp();
        }}
        onPointerCancel={() => {
          dragging.current = false;
          drag.onPointerCancel();
        }}
        onPointerLeave={() => setHover(null)}
      >
        <svg
          viewBox={`0 0 600 ${height}`}
          className="w-full"
          style={{ height }}
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <defs>
            <linearGradient id="seriesFill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity="0.35" />
              <stop offset="100%" stopColor={color} stopOpacity="0.02" />
            </linearGradient>
          </defs>
          {[0.25, 0.5, 0.75].map((fraction) => (
            <line
              key={fraction}
              x1="0"
              x2="600"
              y1={height * fraction}
              y2={height * fraction}
              stroke="var(--border)"
              strokeDasharray="3 4"
              strokeWidth="0.5"
              opacity="0.6"
            />
          ))}
          <path d={geometry.area} fill="url(#seriesFill)" />
          <path d={geometry.line} fill="none" stroke={color} strokeWidth="1.8" strokeLinejoin="round" />
          {hover && hoverIndex != null && (
            <>
              <line x1={(hoverX / 100) * 600} y1="0" x2={(hoverX / 100) * 600} y2={height} stroke="var(--amber)" strokeWidth="0.8" opacity="0.7" />
              <circle cx={(hoverX / 100) * 600} cy={height - 8 - ((hoverValue! - geometry.min) / geometry.span) * (height - 18)} r="3" fill="var(--amber)" />
            </>
          )}
        </svg>
        {hover && hoverIndex != null && hoverValue != null && (
          <div
            className="pointer-events-none absolute z-10 -translate-x-1/2 rounded border border-[var(--border)] bg-[var(--panel)]/95 px-2 py-1 text-[10px] shadow-lg"
            style={{ left: `${clamp(hoverX, 8, 92)}%`, top: 4 }}
          >
            <div className="num text-[var(--text)]">{hoverValue.toFixed(2)} {unit}</div>
            <div className="num text-[var(--muted)]">{label(hoverIndex)}</div>
          </div>
        )}
      </div>
      <div className="mt-1 flex justify-between text-[10px] text-[var(--muted)]">
        <span>{label(i0)}</span>
        <span>{label(i1)}</span>
      </div>
    </div>
  );
}

export function Histogram({
  values,
  height = 120,
  unit,
}: {
  values: number[];
  height?: number;
  unit?: string;
}) {
  const { bins, mean, sigma, min, max } = useMemo(() => {
    if (values.length === 0) return { bins: [] as number[], mean: 0, sigma: 0, min: 0, max: 0 };
    const min = Math.min(...values);
    const max = Math.max(...values);
    const mean = values.reduce((a, b) => a + b, 0) / values.length;
    const sigma = Math.sqrt(values.reduce((a, b) => a + (b - mean) ** 2, 0) / values.length);
    const count = 24;
    const span = max - min || 1;
    const bins = new Array(count).fill(0);
    for (const value of values) {
      const index = Math.min(count - 1, Math.floor(((value - min) / span) * count));
      bins[index] += 1;
    }
    return { bins, mean, sigma, min, max };
  }, [values]);

  if (bins.length === 0) return <p className="text-xs text-[var(--muted)]">no samples yet</p>;
  const peak = Math.max(...bins, 1);
  const meanX = ((mean - min) / Math.max(1e-9, max - min)) * 240;

  return (
    <div>
      <svg viewBox="0 0 240 80" className="w-full" style={{ height }} preserveAspectRatio="none" aria-hidden="true">
        <defs>
          <linearGradient id="histFill" x1="0" y1="1" x2="0" y2="0">
            <stop offset="0%" stopColor="var(--accent-dim)" />
            <stop offset="70%" stopColor="var(--accent)" />
            <stop offset="100%" stopColor="#f0b429" />
          </linearGradient>
        </defs>
        {bins.map((count, index) => (
          <rect
            key={index}
            x={index * 10}
            y={80 - (count / peak) * 76}
            width={9}
            height={(count / peak) * 76}
            fill="url(#histFill)"
            opacity={0.85}
          />
        ))}
        <line x1={meanX} y1="0" x2={meanX} y2="80" stroke="var(--amber)" strokeWidth="1" strokeDasharray="3 3" opacity="0.8" />
      </svg>
      <div className="mt-1 flex justify-between text-[10px] text-[var(--muted)]">
        <span className="num">{min.toFixed(2)} {unit}</span>
        <span className="num">μ {mean.toFixed(2)} · σ {sigma.toFixed(2)}</span>
        <span className="num">{max.toFixed(2)} {unit}</span>
      </div>
    </div>
  );
}
