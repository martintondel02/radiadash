import { Suspense, lazy, useEffect, useMemo, useRef, useState } from "react";
import {
  COUNT_UNITS,
  DOSE_RATE_UNITS,
  DOSE_UNITS,
  convertCount,
  convertDose,
  convertDoseRate,
  formatValue,
  type CountUnit,
  type DoseRateUnit,
  type DoseUnit,
} from "./lib/units";
import type { Candidate, Telemetry } from "./lib/types";
import { useApp } from "./store";
import { setEnergyCalibration } from "./lib/ipc";
import { Histogram, SpectrumChart, TimeSeriesChart } from "./components/charts";

const MapView = lazy(() => import("./components/MapView"));

type NavKey = "overview" | "spectrum" | "rate" | "map" | "history" | "device";

const NAV: { key: NavKey; label: string; hint: string }[] = [
  { key: "overview", label: "Overview", hint: "Everything at once" },
  { key: "spectrum", label: "Spectrum", hint: "Isotopes & fingerprint" },
  { key: "rate", label: "Rate & Dose", hint: "Time series" },
  { key: "map", label: "Map", hint: "Geotagged surveys" },
  { key: "history", label: "History", hint: "Sessions & export" },
  { key: "device", label: "Device", hint: "Connection & settings" },
];

function Logo() {
  return (
    <svg viewBox="0 0 32 32" className="h-8 w-8" aria-hidden="true">
      <circle cx="16" cy="16" r="15" fill="var(--panel-2)" stroke="var(--border)" />
      <circle cx="16" cy="16" r="2.4" fill="var(--accent)" />
      <path d="M16 4.5a11.5 11.5 0 0 1 9.96 5.75" stroke="var(--accent)" strokeWidth="2" fill="none" strokeLinecap="round" />
      <path d="M6.04 10.25A11.5 11.5 0 0 0 6.7 25" stroke="var(--accent)" strokeWidth="2" fill="none" strokeLinecap="round" opacity="0.75" />
      <path d="M25.3 25a11.5 11.5 0 0 0 .66-14.75" stroke="var(--accent)" strokeWidth="2" fill="none" strokeLinecap="round" opacity="0.45" />
    </svg>
  );
}

function Card({
  title,
  children,
  className = "",
  action,
}: {
  title: string;
  children: React.ReactNode;
  className?: string;
  action?: React.ReactNode;
}) {
  return (
    <section className={`panel p-4 ${className}`}>
      <header className="mb-3 flex items-center justify-between gap-2">
        <h2 className="text-[11px] font-medium uppercase tracking-[0.16em] text-[var(--muted)]">{title}</h2>
        {action}
      </header>
      {children}
    </section>
  );
}

function Stat({ label, value, unit, sub }: { label: string; value: string; unit: string; sub?: string }) {
  return (
    <div>
      <div className="text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">{label}</div>
      <div className="mt-1 flex items-baseline gap-1.5">
        <span className="num text-3xl font-medium">{value}</span>
        <span className="text-xs text-[var(--muted)]">{unit}</span>
      </div>
      {sub && <div className="mt-1 text-[11px] text-[var(--muted)]">{sub}</div>}
    </div>
  );
}

const CATEGORY_COLORS: Record<string, string> = {
  norm: "#4fd1c5",
  medical: "#7dd3fc",
  industrial: "#f0b429",
  fallout: "#fca5a5",
  calibration: "#c4b5fd",
  activation: "#fdba74",
};

function categoryColor(category: string) {
  return CATEGORY_COLORS[category] ?? "var(--muted)";
}

function StatusPill({ status }: { status: string }) {
  const style =
    status === "connected"
      ? "border-[var(--accent-dim)] text-[var(--accent)]"
      : status === "connecting"
        ? "border-[var(--amber)] text-[var(--amber)]"
        : status === "error"
          ? "border-[var(--danger)] text-[var(--danger)]"
          : status === "demo"
            ? "border-[var(--border)] text-[var(--muted)]"
            : "border-[var(--border)] text-[var(--muted)]";
  return (
    <span className={`rounded-full border px-2.5 py-1 text-[10px] uppercase tracking-[0.14em] ${style}`}>
      {status}
    </span>
  );
}

function elapsedText(startedAt: number | null, now: number) {
  if (!startedAt) return "—";
  const seconds = Math.max(0, Math.floor((now - startedAt) / 1000));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return h > 0 ? `${h}h ${m}m` : m > 0 ? `${m}m ${s}s` : `${s}s`;
}

function ConnectionPanel() {
  const { status, identity, message, connecting, connect, disconnect } = useApp();
  const [open, setOpen] = useState(false);

  return (
    <div className="rounded-lg border border-[var(--border)] bg-[var(--panel-2)] p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-xs">
          <span
            className={`h-2 w-2 rounded-full ${
              status === "connected" ? "bg-[var(--accent)]" : status === "error" ? "bg-[var(--danger)]" : "bg-[var(--amber)]"
            }`}
          />
          <span className="font-medium capitalize">{status}</span>
        </div>
        {status === "connected" && (
          <button onClick={() => void disconnect()} className="text-[10px] text-[var(--muted)] hover:text-[var(--text)]">
            disconnect
          </button>
        )}
      </div>

      {identity ? (
        <div className="mt-2 space-y-0.5 text-[11px] text-[var(--muted)]">
          <div className="num text-[var(--text)]">{identity.serial}</div>
          <div>
            fw {identity.firmware.major}.{identity.firmware.minor} · {identity.transport}
          </div>
        </div>
      ) : (
        <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--muted)]">
          {message ?? "Connect over Bluetooth (first) or USB."}
        </p>
      )}

      {status !== "connected" && (
        <>
          <button
            disabled={connecting}
            onClick={() => void connect({ kind: "auto" })}
            className="mt-3 w-full rounded-md border border-[var(--border)] px-2 py-1.5 text-[11px] hover:border-[var(--accent-dim)] disabled:opacity-50"
          >
            {connecting ? "Connecting…" : "Connect"}
          </button>
          <button onClick={() => setOpen(!open)} className="mt-1 w-full text-[10px] text-[var(--muted)] hover:text-[var(--text)]">
            {open ? "hide options" : "transport options"}
          </button>
          {open && (
            <div className="mt-1 grid grid-cols-2 gap-1">
              <button
                onClick={() => void connect({ kind: "usb" })}
                className="rounded border border-[var(--border)] px-1.5 py-1 text-[10px] text-[var(--muted)] hover:text-[var(--text)]"
              >
                USB
              </button>
              <button
                onClick={() => void connect({ kind: "ble" })}
                className="rounded border border-[var(--border)] px-1.5 py-1 text-[10px] text-[var(--muted)] hover:text-[var(--text)]"
              >
                Bluetooth
              </button>
            </div>
          )}
        </>
      )}
      <div className="mt-2 px-1 text-[10px] text-[var(--muted)]">v0.1.0 · M3</div>
    </div>
  );
}

function useGeiger(cps: number, enabled: boolean, volume: number) {
  const cpsRef = useRef(cps);
  const volumeRef = useRef(volume);
  cpsRef.current = cps;
  volumeRef.current = volume;

  useEffect(() => {
    if (!enabled) return;
    const AudioCtor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtor) return;
    const context = new AudioCtor();
    const buffer = context.createBuffer(1, Math.floor(context.sampleRate * 0.004), context.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < data.length; i += 1) data[i] = (Math.random() * 2 - 1) * (1 - i / data.length);

    const timer = window.setInterval(() => {
      const expected = Math.min(cpsRef.current, 500) * 0.08;
      let clicks = Math.floor(expected);
      if (Math.random() < expected - clicks) clicks += 1;
      clicks = Math.min(clicks, 6);
      for (let i = 0; i < clicks; i += 1) {
        const source = context.createBufferSource();
        source.buffer = buffer;
        const gain = context.createGain();
        const level = volumeRef.current * (0.6 + Math.random() * 0.4);
        gain.gain.setValueAtTime(level, context.currentTime + Math.random() * 0.02);
        gain.gain.exponentialRampToValueAtTime(0.001, context.currentTime + 0.03);
        source.connect(gain).connect(context.destination);
        source.start(context.currentTime + Math.random() * 0.02);
      }
    }, 80);

    return () => {
      window.clearInterval(timer);
      void context.close();
    };
  }, [enabled]);
}

interface CalibrationPoint {
  channel: number;
  energy: number;
  isotope: string;
  intensity: number;
}

function calibrationPoints(analysis: ReturnType<typeof useApp.getState>["analysis"]): CalibrationPoint[] {
  if (!analysis) return [];
  const byChannel = new Map<number, CalibrationPoint>();
  for (const candidate of analysis.candidates) {
    for (const evidence of candidate.evidence) {
      if (!evidence.matched || evidence.peak_channel == null) continue;
      if (evidence.intensity < 10) continue;
      if (Math.abs(evidence.delta_kev ?? 999) > 60) continue;
      const point: CalibrationPoint = {
        channel: evidence.peak_channel,
        energy: evidence.line_energy_kev,
        isotope: candidate.id,
        intensity: evidence.intensity,
      };
      const existing = byChannel.get(point.channel);
      if (!existing || existing.intensity < point.intensity) byChannel.set(point.channel, point);
    }
  }
  return [...byChannel.values()].sort((a, b) => a.channel - b.channel);
}

function solve(matrix: number[][], vector: number[]): number[] | null {
  const n = vector.length;
  const a = matrix.map((row, index) => [...row, vector[index]]);
  for (let column = 0; column < n; column += 1) {
    let pivot = column;
    for (let row = column + 1; row < n; row += 1) {
      if (Math.abs(a[row][column]) > Math.abs(a[pivot][column])) pivot = row;
    }
    if (Math.abs(a[pivot][column]) < 1e-12) return null;
    [a[column], a[pivot]] = [a[pivot], a[column]];
    for (let row = 0; row < n; row += 1) {
      if (row === column) continue;
      const factor = a[row][column] / a[column][column];
      for (let k = column; k <= n; k += 1) a[row][k] -= factor * a[column][k];
    }
  }
  return a.map((row, index) => row[n] / row[index]);
}

function fitCalibration(points: CalibrationPoint[], quadratic: boolean): [number, number, number] | null {
  if (points.length < 2) return null;
  const terms = quadratic && points.length >= 3 ? 3 : 2;
  const ata = Array.from({ length: terms }, () => new Array(terms).fill(0));
  const atb = new Array(terms).fill(0);
  for (const point of points) {
    const x = point.channel;
    const basis = terms === 3 ? [1, x, x * x] : [1, x];
    for (let i = 0; i < terms; i += 1) {
      atb[i] += basis[i] * point.energy;
      for (let j = 0; j < terms; j += 1) ata[i][j] += basis[i] * basis[j];
    }
  }
  const solved = solve(ata, atb);
  if (!solved) return null;
  return [solved[0], solved[1], terms === 3 ? solved[2] : 0];
}

function fitRobust(points: CalibrationPoint[]): { fit: [number, number, number]; inliers: CalibrationPoint[] } | null {
  let inliers = points;
  let fit = fitCalibration(inliers, inliers.length >= 4);
  if (!fit) return null;

  for (let iteration = 0; iteration < 3; iteration += 1) {
    const residuals = inliers.map((point) => fit![0] + fit![1] * point.channel + fit![2] * point.channel * point.channel - point.energy);
    const sorted = [...residuals].sort((a, b) => a - b);
    const median = sorted[Math.floor(sorted.length / 2)] ?? 0;
    const mad = sorted.map((value) => Math.abs(value - median)).sort((a, b) => a - b)[Math.floor(sorted.length / 2)] ?? 0;
    const threshold = Math.max(4, 3 * 1.4826 * mad);
    const next = inliers.filter((_, index) => Math.abs(residuals[index]) <= threshold);
    if (next.length < 2 || next.length === inliers.length) break;
    const refit = fitCalibration(next, next.length >= 4);
    if (!refit) break;
    inliers = next;
    fit = refit;
  }
  return fit ? { fit, inliers } : null;
}

function CalibrationCard() {
  const { analysis, calibration } = useApp();
  const [fit, setFit] = useState<[number, number, number] | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [points, setPoints] = useState<CalibrationPoint[]>([]);
  const [excluded, setExcluded] = useState(0);
  const [rms, setRms] = useState(0);

  const current = calibration;

  const runFit = () => {
    const collected = calibrationPoints(analysis);
    setPoints(collected);
    if (collected.length < 2) {
      setFit(null);
      setMessage("Need at least two identified peaks (e.g. K-40 and Tl-208/Bi-214). Collect more spectrum first.");
      return;
    }
    const result = fitRobust(collected);
    setFit(result?.fit ?? null);
    setExcluded(result ? collected.length - result.inliers.length : 0);
    if (result) setPoints(result.inliers);
    setMessage(result ? null : "Fit failed — points are degenerate.");
    if (result) {
      const residuals = result.inliers.map(
        (point) => result.fit[0] + result.fit[1] * point.channel + result.fit[2] * point.channel * point.channel - point.energy,
      );
      setRms(Math.sqrt(residuals.reduce((a, b) => a + b * b, 0) / residuals.length));
    } else {
      setRms(0);
    }
  };

  const apply = async () => {
    if (!fit) return;
    const text = `Write calibration to the device?

a0=${fit[0].toFixed(4)}  a1=${fit[1].toFixed(6)}  a2=${fit[2].toExponential(3)}

This replaces the factory calibration. You can restore it from a backup if you recorded the old values.`;
    if (!window.confirm(text)) return;
    try {
      await setEnergyCalibration(fit);
      setMessage("Calibration written. New spectra will use it.");
    } catch (error) {
      setMessage(String(error));
    }
  };

  const residual = (coefficients: [number, number, number], channel: number, target: number) =>
    coefficients[0] + coefficients[1] * channel + coefficients[2] * channel * channel - target;

  return (
    <Card title="Energy calibration">
      {current ? (
        <div className="num text-[11px] text-[var(--muted)]">
          a0={current[0].toFixed(4)} · a1={current[1].toFixed(6)} · a2={current[2].toExponential(3)}
        </div>
      ) : (
        <p className="text-[11px] text-[var(--muted)]">Not connected.</p>
      )}

      <div className="mt-3 flex flex-wrap gap-2">
        <button
          onClick={runFit}
          className="rounded border border-[var(--border)] px-2 py-1 text-[11px] text-[var(--muted)] hover:text-[var(--text)]"
        >
          Fit from identified peaks
        </button>
        <button
          onClick={() => void apply()}
          disabled={!fit || rms > 12}
          className="rounded border border-[var(--accent-dim)] px-2 py-1 text-[11px] text-[var(--accent)] disabled:opacity-40"
        >
          Apply to device
        </button>
      </div>

      {points.length > 0 && (
        <table className="mt-3 w-full text-[10px]">
          <thead>
            <tr className="text-left text-[var(--muted)]">
              <th className="font-normal">Isotope</th>
              <th className="text-right font-normal">Channel</th>
              <th className="text-right font-normal">Line</th>
              <th className="text-right font-normal">Fit Δ</th>
            </tr>
          </thead>
          <tbody className="num">
            {points.map((point) => (
              <tr key={point.channel}>
                <td>{point.isotope}</td>
                <td className="text-right">{point.channel}</td>
                <td className="text-right">{point.energy.toFixed(1)}</td>
                <td className="text-right">
                  {fit ? `${residual(fit, point.channel, point.energy).toFixed(1)} keV` : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {fit && (
        <p className={`mt-2 text-[10px] ${rms > 8 ? "text-[var(--amber)]" : "text-[var(--muted)]"}`}>
          Fit RMS {rms.toFixed(1)} keV over {points.length} anchors{rms > 8 ? " — poor agreement, check peak identities" : ""}.
        </p>
      )}
      {excluded > 0 && (
        <p className="mt-2 text-[10px] text-[var(--muted)]">
          {excluded} point{excluded === 1 ? "" : "s"} rejected as outliers (residual &gt; 3×MAD).
        </p>
      )}
      {message && <p className="mt-2 text-[10px] leading-relaxed text-[var(--amber)]">{message}</p>}
      <p className="mt-2 text-[10px] leading-relaxed text-[var(--muted)]">
        Fits a0/a1 (and a2 with three or more anchors) to the identified peaks. Only use this if the peaks are
        confidently identified; a bad calibration is worse than a slightly offset one.
      </p>
    </Card>
  );
}

function Overview({ now }: { now: number }) {
  const { latest, history, spectrum, identity, units, startedAt, analysis, density } = useApp();
  const wide = density === "comfortable" ? "lg:col-span-2" : "";
  const grid =
    density === "compact"
      ? "grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4"
      : density === "focus"
        ? "grid grid-cols-1 gap-4"
        : "grid grid-cols-1 gap-4 lg:grid-cols-3";
  const candidates = analysis?.candidates ?? [];
  const cpsValues = history.slice(-360).map((t) => t.cps);

  const doseRate = latest ? convertDoseRate(latest.dose_rate_usvh, units.doseRate) : null;
  const rateDisplay = doseRate === null ? null : formatValue(doseRate);
  const dose = latest?.dose_usv != null ? convertDose(latest.dose_usv, units.dose) : null;
  const doseDisplay = dose === null ? null : formatValue(dose);
  const cps = latest ? convertCount(latest.cps, units.count) : null;
  const cpsDisplay = cps === null ? null : formatValue(cps);

  return (
    <div className={grid}>
      <Card
        title="Count rate"
        className={wide}
        action={<span className="num text-[11px] text-[var(--muted)]">last {cpsValues.length} s</span>}
      >
        <div className="flex flex-wrap items-end justify-between gap-3">
          <Stat
            label="Live"
            value={cpsDisplay?.text ?? "—"}
            unit={units.count}
            sub={latest ? `± ${latest.cps_err.toFixed(1)} % · ${Math.round(latest.cps * 60).toLocaleString()} cpm` : "no data"}
          />
          <div className="text-right text-[10px] text-[var(--muted)]">
            <div className="num">{latest?.flags ? `flags 0x${latest.flags.toString(16).padStart(4, "0")}` : ""}</div>
            <div>{history.length} samples in session</div>
          </div>
        </div>
        <div className="mt-3">
          <TimeSeriesChart
            values={cpsValues}
            times={history.slice(-360).map((sample) => sample.ts)}
            unit={units.count}
            height={130}
          />
        </div>
      </Card>

      <Card title="Dose rate" action={
          <span className="text-[10px] text-[var(--muted)]">{identity ? `device: ${identity.config?.DS_AlarmMax_R ? "R" : "Sv"}` : ""}</span>
        }>
        <Stat
          label="Current"
          value={rateDisplay?.text ?? "—"}
          unit={units.doseRate}
          sub={latest ? `± ${latest.dose_rate_err.toFixed(1)} %` : "no data"}
        />
        <div className="mt-4 h-1.5 w-full overflow-hidden rounded-full bg-[var(--panel-2)]">
          <div
            className="h-full rounded-full"
            style={{
              width: `${Math.min(100, ((latest?.dose_rate_usvh ?? 0) / 5) * 100)}%`,
              background:
                (latest?.dose_rate_usvh ?? 0) > 3.75
                  ? "var(--danger)"
                  : (latest?.dose_rate_usvh ?? 0) > 2
                    ? "var(--amber)"
                    : "linear-gradient(90deg, var(--accent-dim), var(--accent))",
            }}
          />
        </div>
        <div className="mt-1 flex justify-between text-[10px] text-[var(--muted)]">
          <span>0</span>
          <span>5 µSv/h</span>
        </div>
      </Card>

      <Card title="Spectrum · 1024 ch">
        <SpectrumChart spectrum={spectrum} height={130} compact />
        <div className="mt-2 flex items-center justify-between text-[10px] text-[var(--muted)]">
          <span>{spectrum ? `${spectrum.counts.reduce((a, b) => a + b, 0).toLocaleString()} counts` : "—"}</span>
          <span>{spectrum ? `${spectrum.duration_s.toLocaleString()} s live` : ""}</span>
        </div>
      </Card>

      <Card title="Accumulated dose">
        <Stat
          label="Session total"
          value={doseDisplay?.text ?? "—"}
          unit={units.dose}
          sub={startedAt ? `since ${new Date(startedAt).toLocaleTimeString()} · ${elapsedText(startedAt, now)}` : "not connected"}
        />
        <button
          onClick={() => void useApp.getState().resetDose()}
          disabled={!latest}
          className="mt-3 rounded border border-[var(--border)] px-1.5 py-0.5 text-[10px] text-[var(--muted)] hover:text-[var(--text)] disabled:opacity-40"
        >
          reset device counter
        </button>
      </Card>

      <Card title="Device">
        <dl className="grid grid-cols-2 gap-y-2 text-xs">
          {[
            ["Model", identity?.model ?? "RC-110"],
            ["Serial", identity?.serial ?? "—"],
            ["Firmware", identity ? `${identity.firmware.major}.${identity.firmware.minor}` : "—"],
            ["Battery", latest?.charge != null ? `${latest.charge.toFixed(0)} %` : "—"],
            ["Temperature", latest?.temp_c != null ? `${latest.temp_c.toFixed(1)} °C` : "—"],
            ["Transport", identity?.transport ?? "not connected"],
          ].map(([key, value]) => (
            <div key={key} className="contents">
              <dt className="text-[var(--muted)]">{key}</dt>
              <dd className="num truncate text-right" title={String(value)}>
                {value}
              </dd>
            </div>
          ))}
        </dl>
      </Card>

      <Card
        title="Isotope candidates"
        className="lg:col-span-2"
        action={<span className="num text-[10px] text-[var(--muted)]">library v{analysis?.library_version ?? "—"}</span>}
      >
        {candidates.length === 0 ? (
          <div className="rounded-lg border border-dashed border-[var(--border)] p-4 text-center text-xs text-[var(--muted)]">
            Collecting spectrum — candidates appear after the first analysis.
          </div>
        ) : (
          <div className="flex flex-wrap gap-2">
            {candidates.slice(0, 4).map((candidate) => (
              <div key={candidate.id} className="rounded-lg border border-[var(--border)] bg-[var(--panel-2)] px-3 py-2">
                <div className="flex items-center gap-2">
                  <span className="h-1.5 w-1.5 rounded-full" style={{ background: categoryColor(candidate.category) }} />
                  <span className="num text-sm font-medium">{candidate.id}</span>
                  <span className={`rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider ${confidenceClass(candidate.confidence)}`}>
                    {candidate.confidence}
                  </span>
                </div>
                <div className="num mt-1 text-[10px] text-[var(--muted)]">
                  {candidate.matched_lines}/{candidate.total_lines} lines · score {candidate.score.toFixed(2)}
                </div>
              </div>
            ))}
          </div>
        )}
        <p className="mt-3 text-[10px] leading-relaxed text-[var(--muted)]">
          Candidate identification from peak matching. Not a certified measurement — verify with a calibrated instrument.
        </p>
      </Card>
    </div>
  );
}

function confidenceClass(confidence: Candidate["confidence"]) {
  switch (confidence) {
    case "high":
      return "bg-[var(--accent-dim)] text-[var(--text)]";
    case "medium":
      return "border border-[var(--amber)] text-[var(--amber)]";
    case "low":
      return "border border-[var(--border)] text-[var(--muted)]";
    default:
      return "border border-[var(--border)] text-[var(--muted)]";
  }
}

function calibrationHint(candidate: Candidate): string | null {
  const deltas = candidate.evidence
    .filter((item) => item.matched && item.delta_kev != null && item.intensity >= 5)
    .map((item) => item.delta_kev as number)
    .sort((a, b) => a - b);
  if (deltas.length === 0) return null;
  const median = deltas[Math.floor(deltas.length / 2)];
  if (Math.abs(median) < 4) return null;
  return `Median energy offset ${median > 0 ? "+" : ""}${median.toFixed(1)} keV on matched lines — the device calibration may need adjusting.`;
}

function SpectrumView() {
  const { spectrum, analysis } = useApp();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const candidates = analysis?.candidates ?? [];
  const selected = candidates.find((candidate) => candidate.id === selectedId) ?? candidates[0] ?? null;

  const overlay = selected
    ? {
        lines: selected.evidence.map((item) => ({ energy: item.line_energy_kev, matched: item.matched })),
      }
    : undefined;

  return (
    <div className="grid gap-4 lg:grid-cols-3">
      <Card
        title={`Energy spectrum${selected ? ` · ${selected.id}` : ""}`}
        className="lg:col-span-2"
        action={
          <span className="text-[10px] text-[var(--muted)]">
            {analysis ? `${analysis.peaks.length} peaks · ${analysis.total_counts.toLocaleString()} counts` : "analysing…"}
          </span>
        }
      >
        <SpectrumChart spectrum={spectrum} height={320} overlay={overlay} />
        <p className="mt-2 text-[10px] text-[var(--muted)]">
          Amber sticks are library lines matched to a peak; dashed grey sticks are expected but not observed.
        </p>
      </Card>

      <div className="space-y-4">
        <Card title="Candidates">
          {candidates.length === 0 && <p className="text-xs text-[var(--muted)]">No candidates yet — waiting for a spectrum.</p>}
          <div className="space-y-1.5">
            {candidates.map((candidate) => (
              <button
                key={candidate.id}
                onClick={() => setSelectedId(candidate.id)}
                className={`w-full rounded-lg border px-3 py-2 text-left transition-colors ${
                  selected?.id === candidate.id
                    ? "border-[var(--accent-dim)] bg-[var(--panel-2)]"
                    : "border-[var(--border)] hover:bg-[var(--panel-2)]"
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="flex items-center gap-2">
                    <span className="h-1.5 w-1.5 rounded-full" style={{ background: categoryColor(candidate.category) }} />
                    <span className="num text-sm">{candidate.id}</span>
                  </span>
                  <span className={`rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider ${confidenceClass(candidate.confidence)}`}>
                    {candidate.confidence}
                  </span>
                </div>
                <div className="num mt-0.5 text-[10px] text-[var(--muted)]">
                  {candidate.matched_lines}/{candidate.total_lines} lines · coverage {(candidate.coverage * 100).toFixed(0)}% · score{" "}
                  {candidate.score.toFixed(2)}
                </div>
              </button>
            ))}
          </div>
        </Card>

        <Card title="Evidence">
          {selected ? (
            <table className="w-full text-[11px]">
              <thead>
                <tr className="text-left text-[var(--muted)]">
                  <th className="pb-1 font-normal">Line</th>
                  <th className="pb-1 text-right font-normal">I</th>
                  <th className="pb-1 text-right font-normal">Peak</th>
                  <th className="pb-1 text-right font-normal">Δ</th>
                </tr>
              </thead>
              <tbody className="num">
                {selected.evidence.map((item) => (
                  <tr key={`${item.line_energy_kev}-${item.intensity}`} className={item.matched ? "" : "text-[var(--muted)]"}>
                    <td className="py-0.5">
                      {item.matched ? "✔" : "·"} {item.line_energy_kev.toFixed(1)}
                    </td>
                    <td className="py-0.5 text-right">{item.intensity.toFixed(0)}%</td>
                    <td className="py-0.5 text-right">{item.peak_energy_kev?.toFixed(1) ?? "—"}</td>
                    <td className="py-0.5 text-right">
                      {item.delta_kev != null ? `${item.delta_kev > 0 ? "+" : ""}${item.delta_kev.toFixed(1)}` : "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="text-xs text-[var(--muted)]">Select a candidate.</p>
          )}
          {selected && (
            <p className="mt-2 text-[10px] leading-relaxed text-[var(--muted)]">
              {selected.name} · {selected.half_life} · {selected.category}. A single line can belong to several nuclides.
            </p>
          )}
          {selected && calibrationHint(selected) && (
            <p className="mt-1 text-[10px] leading-relaxed text-[var(--amber)]">{calibrationHint(selected)}</p>
          )}
        </Card>
      </div>

      <Card title="Detected peaks" className="lg:col-span-3">
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
          {(analysis?.peaks ?? []).slice(0, 12).map((peak) => (
            <div key={peak.channel} className="rounded border border-[var(--border)] px-2 py-1.5">
              <div className="num text-sm text-[var(--accent)]">{peak.energy_kev.toFixed(1)} keV</div>
              <div className="num text-[10px] text-[var(--muted)]">
                net {Math.round(peak.net).toLocaleString()} · {peak.significance.toFixed(1)}σ
              </div>
            </div>
          ))}
        </div>
      </Card>
    </div>
  );
}

function RateView({ now }: { now: number }) {
  const { series, history, units, latest, refreshSeries } = useApp();
  const [range, setRange] = useState(3600);

  const data = useMemo(() => {
    if (series && series.cps.length > 2) return series.cps;
    return history.slice(-600).map((t) => t.cps);
  }, [series, history]);

  return (
    <div className="space-y-4">
      <Card
        title="Count rate"
        action={
          <div className="flex gap-1">
            {[
              [300, "5m"],
              [3600, "1h"],
              [21600, "6h"],
              [86400, "24h"],
            ].map(([seconds, label]) => (
              <button
                key={String(label)}
                onClick={() => {
                  setRange(seconds as number);
                  void refreshSeries(seconds as number);
                }}
                className={`rounded border px-1.5 py-0.5 text-[10px] ${
                  range === seconds
                    ? "border-[var(--accent-dim)] text-[var(--accent)]"
                    : "border-[var(--border)] text-[var(--muted)]"
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        }
      >
        <TimeSeriesChart
          values={data}
          times={series && series.ts.length > 2 ? series.ts : history.slice(-600).map((sample) => sample.ts)}
          unit={units.count}
          height={170}
        />
        <div className="mt-2 flex justify-between text-[10px] text-[var(--muted)]">
          <span>{data.length} points</span>
          <span>now {latest ? `${convertCount(latest.cps, units.count).toFixed(1)} ${units.count}` : ""}</span>
          <span>{elapsedText(useApp.getState().startedAt, now)}</span>
        </div>
      </Card>
      <Card title="Dose rate series">
        <TimeSeriesChart
          values={(series?.dose_rate_usvh ?? history.map((t) => t.dose_rate_usvh)).slice(-600)}
          times={series?.ts}
          unit="µSv/h"
          color="var(--amber)"
          height={170}
        />
      </Card>
      <div className="grid gap-4 lg:grid-cols-2">
        <Card title="CPS distribution">
          <Histogram values={history.slice(-600).map((t) => t.cps)} unit={units.count} />
        </Card>
        <Card title="Dose rate distribution">
          <Histogram values={history.slice(-600).map((t) => t.dose_rate_usvh)} unit="µSv/h" />
        </Card>
      </div>
    </div>
  );
}

function Placeholder({ title, milestone, body }: { title: string; milestone: string; body: string }) {
  return (
    <Card title={title}>
      <div className="rounded-lg border border-dashed border-[var(--border)] p-6 text-center">
        <div className="text-[10px] uppercase tracking-[0.16em] text-[var(--muted)]">{milestone}</div>
        <p className="mt-2 text-sm">{body}</p>
      </div>
    </Card>
  );
}

export default function App() {
  const {
    theme,
    toggleTheme,
    units,
    setDoseRateUnit,
    setDoseUnit,
    setCountUnit,
    status,
    init,
    density,
    setDensity,
    sound,
    setSound,
    volume,
    setVolume,
    latest,
  } = useApp();
  useGeiger(latest?.cps ?? 0, sound, volume);
  const [nav, setNav] = useState<NavKey>("overview");
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    void init();
  }, [init]);

  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    document.documentElement.classList.toggle("light", theme === "light");
    document.documentElement.style.colorScheme = theme;
  }, [theme]);

  const page = NAV.find((item) => item.key === nav);

  return (
    <div className="flex h-full">
      <aside className="flex w-60 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--panel)]">
        <div className="flex items-center gap-2.5 px-4 py-5">
          <Logo />
          <div>
            <div className="text-sm font-semibold tracking-tight">Radiadash</div>
            <div className="text-[10px] uppercase tracking-[0.18em] text-[var(--muted)]">RC-110 console</div>
          </div>
        </div>

        <nav className="flex flex-col gap-0.5 px-2" aria-label="Primary">
          {NAV.map((item) => (
            <button
              key={item.key}
              onClick={() => setNav(item.key)}
              aria-current={nav === item.key ? "page" : undefined}
              className={`flex items-center justify-between rounded-md px-3 py-2 text-left text-sm transition-colors ${
                nav === item.key
                  ? "bg-[var(--panel-2)] text-[var(--text)]"
                  : "text-[var(--muted)] hover:bg-[var(--panel-2)] hover:text-[var(--text)]"
              }`}
            >
              <span>{item.label}</span>
              {nav === item.key && <span className="h-1.5 w-1.5 rounded-full bg-[var(--accent)]" />}
            </button>
          ))}
        </nav>

        <div className="mt-auto p-3">
          <ConnectionPanel />
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border)] px-6 py-3">
          <div className="min-w-0">
            <h1 className="truncate text-sm font-medium">{page?.label}</h1>
            <p className="truncate text-[11px] text-[var(--muted)]">{page?.hint}</p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <StatusPill status={status} />
            <select
              value={units.doseRate}
              onChange={(event) => setDoseRateUnit(event.target.value as DoseRateUnit)}
              className="rounded-md border border-[var(--border)] bg-[var(--panel)] px-2 py-1.5 text-xs"
              aria-label="Dose rate unit"
            >
              {DOSE_RATE_UNITS.map((unit) => (
                <option key={unit}>{unit}</option>
              ))}
            </select>
            <select
              value={units.dose}
              onChange={(event) => setDoseUnit(event.target.value as DoseUnit)}
              className="rounded-md border border-[var(--border)] bg-[var(--panel)] px-2 py-1.5 text-xs"
              aria-label="Dose unit"
            >
              {DOSE_UNITS.map((unit) => (
                <option key={unit}>{unit}</option>
              ))}
            </select>
            <select
              value={units.count}
              onChange={(event) => setCountUnit(event.target.value as CountUnit)}
              className="rounded-md border border-[var(--border)] bg-[var(--panel)] px-2 py-1.5 text-xs"
              aria-label="Count rate unit"
            >
              {COUNT_UNITS.map((unit) => (
                <option key={unit}>{unit}</option>
              ))}
            </select>
            <select
              value={density}
              onChange={(event) => setDensity(event.target.value as "comfortable" | "compact" | "focus")}
              className="rounded-md border border-[var(--border)] bg-[var(--panel)] px-2 py-1.5 text-xs"
              aria-label="Layout density"
            >
              <option value="comfortable">Comfortable</option>
              <option value="compact">Compact</option>
              <option value="focus">Focus</option>
            </select>
            <button
              onClick={() => setSound(!sound)}
              className={`rounded-md border px-2.5 py-1.5 text-xs ${
                sound ? "border-[var(--accent-dim)] text-[var(--accent)]" : "border-[var(--border)] text-[var(--muted)]"
              }`}
              aria-pressed={sound}
            >
              {sound ? "Sound" : "Muted"}
            </button>
            <button
              onClick={toggleTheme}
              className="rounded-md border border-[var(--border)] px-2.5 py-1.5 text-xs text-[var(--muted)] hover:text-[var(--text)]"
              aria-label="Toggle theme"
            >
              {theme === "dark" ? "Light" : "Dark"}
            </button>
          </div>
        </header>

        <div className="flex-1 overflow-auto p-6">
          {nav === "overview" && <Overview now={now} />}
          {nav === "spectrum" && <SpectrumView />}
          {nav === "rate" && <RateView now={now} />}
          {nav === "map" && (
            <Suspense fallback={<div className="panel p-6 text-xs text-[var(--muted)]">Loading map…</div>}>
              <MapView />
            </Suspense>
          )}
          {nav === "history" && (
            <Placeholder title="History" milestone="M2 storage / M6 export" body="Sessions are stored in SQLite; browsing and export arrive next." />
          )}
          {nav === "device" && (
            <div className="grid gap-4 lg:grid-cols-2">
              <Card title="Device">
                <DeviceDetails />
              </Card>
              <CalibrationCard />
              <Card title="Geiger audio">
                <label className="flex items-center gap-2 text-xs">
                  <input type="checkbox" checked={sound} onChange={(event) => setSound(event.target.checked)} />
                  Click audio from measured count rate
                </label>
                <div className="mt-3 flex items-center gap-3">
                  <span className="text-[11px] text-[var(--muted)]">Volume</span>
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={volume}
                    onChange={(event) => setVolume(Number(event.target.value))}
                    className="flex-1"
                    aria-label="Geiger volume"
                  />
                  <span className="num text-[11px] text-[var(--muted)]">{Math.round(volume * 100)}%</span>
                </div>
                <p className="mt-2 text-[10px] leading-relaxed text-[var(--muted)]">
                  Clicks are generated from the measured CPS with Poisson jitter — the audio is a sonification, not an
                  independent measurement.
                </p>
              </Card>
            </div>
          )}
          <p className="mt-6 text-center text-[10px] text-[var(--muted)]">
            M5 · live data · isotope candidates · calibration wizard · survey map · no cloud
          </p>
        </div>
      </main>
    </div>
  );
}

function DeviceDetails() {
  const { identity, latest } = useApp();
  const rows: [string, string][] = [
    ["Model", identity?.model ?? "RC-110 (reported by firmware: DType=4)"],
    ["Serial", identity?.serial ?? "—"],
    ["Hardware serial", identity?.hw_serial ?? "—"],
    ["Firmware", identity ? `${identity.firmware.major}.${identity.firmware.minor} · ${identity.firmware.date}` : "—"],
    ["Spectrum format", identity ? String(identity.spectrum_format) : "—"],
    ["Transport", identity?.transport ?? "—"],
    ["Battery", latest?.charge != null ? `${latest.charge.toFixed(1)} %` : "—"],
    ["Temperature", latest?.temp_c != null ? `${latest.temp_c.toFixed(2)} °C` : "—"],
  ];
  return (
    <dl className="grid grid-cols-2 gap-y-2 text-xs">
      {rows.map(([key, value]) => (
        <div key={key} className="contents">
          <dt className="text-[var(--muted)]">{key}</dt>
          <dd className="num truncate text-right" title={value}>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

export type { Telemetry };
