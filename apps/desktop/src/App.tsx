import { useEffect, useMemo, useState } from "react";
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
import type { SpectrumPayload, Telemetry } from "./lib/types";
import { useApp } from "./store";

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

function Sparkline({ data }: { data: number[] }) {
  const path = useMemo(() => {
    if (data.length < 2) return "";
    const width = 600;
    const height = 120;
    const max = Math.max(...data);
    const min = Math.min(...data);
    const span = max - min || 1;
    return data
      .map((value, index) => {
        const x = (index / (data.length - 1)) * width;
        const y = height - ((value - min) / span) * (height - 8) - 4;
        return `${index === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  }, [data]);

  if (!path) {
    return <div className="flex h-28 items-center justify-center text-xs text-[var(--muted)]">waiting for samples…</div>;
  }

  return (
    <svg viewBox="0 0 600 120" className="h-28 w-full" preserveAspectRatio="none" aria-hidden="true">
      <path d={path} fill="none" stroke="var(--accent)" strokeWidth="1.8" strokeLinejoin="round" />
    </svg>
  );
}

function energyOf(spectrum: SpectrumPayload, channel: number) {
  return spectrum.a0 + spectrum.a1 * channel + spectrum.a2 * channel * channel;
}

function SpectrumChart({ spectrum, height = 180 }: { spectrum: SpectrumPayload | null; height?: number }) {
  const bars = useMemo(() => {
    if (!spectrum) return [] as number[];
    const counts = spectrum.counts;
    const max = Math.max(...counts, 1);
    return counts.map((value) => Math.log10(1 + value) / Math.log10(1 + max));
  }, [spectrum]);

  if (!spectrum || bars.length === 0) {
    return <div className="flex h-44 items-center justify-center text-xs text-[var(--muted)]">no spectrum yet</div>;
  }

  const width = 1024;
  return (
    <div>
      <svg viewBox={`0 0 ${width} ${height}`} className="w-full" preserveAspectRatio="none" aria-hidden="true">
        {bars.map((value, index) => (
          <rect
            key={index}
            x={index}
            y={height - value * (height - 4)}
            width={0.8}
            height={value * (height - 4)}
            fill="var(--accent)"
            opacity={0.78}
          />
        ))}
        <line x1="0" y1={height - 0.5} x2={width} y2={height - 0.5} stroke="var(--border)" strokeWidth="0.6" />
      </svg>
      <div className="mt-1 flex justify-between text-[10px] text-[var(--muted)]">
        <span>0 keV</span>
        <span>{Math.round(energyOf(spectrum, 512))} keV</span>
        <span>{Math.round(energyOf(spectrum, spectrum.counts.length - 1))} keV</span>
      </div>
    </div>
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
      <div className="mt-2 px-1 text-[10px] text-[var(--muted)]">v0.1.0 · M2 live</div>
    </div>
  );
}

function Overview({ now }: { now: number }) {
  const { latest, history, spectrum, identity, units, startedAt } = useApp();
  const cpsValues = history.slice(-360).map((t) => t.cps);

  const doseRate = latest ? convertDoseRate(latest.dose_rate_usvh, units.doseRate) : null;
  const rateDisplay = doseRate === null ? null : formatValue(doseRate);
  const dose = latest?.dose_usv != null ? convertDose(latest.dose_usv, units.dose) : null;
  const doseDisplay = dose === null ? null : formatValue(dose);
  const cps = latest ? convertCount(latest.cps, units.count) : null;
  const cpsDisplay = cps === null ? null : formatValue(cps);

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <Card
        title="Count rate"
        className="lg:col-span-2"
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
          <Sparkline data={cpsValues} />
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
            className="h-full rounded-full bg-[var(--accent)]"
            style={{ width: `${Math.min(100, ((latest?.dose_rate_usvh ?? 0) / 5) * 100)}%` }}
          />
        </div>
        <div className="mt-1 flex justify-between text-[10px] text-[var(--muted)]">
          <span>0</span>
          <span>5 µSv/h</span>
        </div>
      </Card>

      <Card title="Spectrum · 1024 ch">
        <SpectrumChart spectrum={spectrum} height={130} />
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

      <Card title="Isotope candidates" className="lg:col-span-2">
        <div className="flex flex-wrap gap-2">
          {[
            { name: "Cs-137", conf: "pending", lines: "661.7 keV · M3" },
            { name: "K-40", conf: "pending", lines: "1460.8 keV · M3" },
            { name: "Bi-214", conf: "pending", lines: "609.3 · 1120.3 · 1764.5" },
            { name: "Pb-214", conf: "pending", lines: "295.2 · 351.9" },
          ].map((iso) => (
            <div key={iso.name} className="rounded-lg border border-[var(--border)] bg-[var(--panel-2)] px-3 py-2">
              <div className="flex items-center gap-2">
                <span className="num text-sm font-medium">{iso.name}</span>
                <span className="rounded-full border border-[var(--border)] px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-[var(--muted)]">
                  {iso.conf}
                </span>
              </div>
              <div className="num mt-1 text-[10px] text-[var(--muted)]">{iso.lines}</div>
            </div>
          ))}
        </div>
        <p className="mt-3 text-[10px] leading-relaxed text-[var(--muted)]">
          Peak matching and the isotope library land in M3. The spectrum above is live from the device.
        </p>
      </Card>
    </div>
  );
}

function SpectrumView() {
  const { spectrum } = useApp();
  const peaks = useMemo(() => {
    if (!spectrum) return [];
    const counts = spectrum.counts;
    const out: { channel: number; energy: number; counts: number }[] = [];
    for (let i = 2; i < counts.length - 2; i++) {
      const value = counts[i];
      if (value > 50 && value > counts[i - 1] && value >= counts[i + 1] && value > counts[i - 2] && value >= counts[i + 2]) {
        out.push({ channel: i, energy: energyOf(spectrum, i), counts: value });
      }
    }
    return out.sort((a, b) => b.counts - a.counts).slice(0, 8).sort((a, b) => a.channel - b.channel);
  }, [spectrum]);

  return (
    <div className="grid gap-4 lg:grid-cols-3">
      <Card title="Energy spectrum" className="lg:col-span-2">
        <SpectrumChart spectrum={spectrum} height={320} />
      </Card>
      <Card title="Detected peaks">
        {peaks.length === 0 && <p className="text-xs text-[var(--muted)]">No peaks above threshold yet.</p>}
        <ul className="space-y-1.5 text-xs">
          {peaks.map((peak) => (
            <li key={peak.channel} className="flex items-center justify-between gap-2">
              <span className="num text-[var(--accent)]">{peak.energy.toFixed(1)} keV</span>
              <span className="num text-[var(--muted)]">ch {peak.channel} · {peak.counts.toLocaleString()}</span>
            </li>
          ))}
        </ul>
        {spectrum && (
          <p className="num mt-3 text-[10px] leading-relaxed text-[var(--muted)]">
            a0={spectrum.a0.toFixed(3)} a1={spectrum.a1.toFixed(5)} a2={spectrum.a2.toExponential(2)}
          </p>
        )}
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
        <Sparkline data={data} />
        <div className="mt-2 flex justify-between text-[10px] text-[var(--muted)]">
          <span>{data.length} points</span>
          <span>now {latest ? `${convertCount(latest.cps, units.count).toFixed(1)} ${units.count}` : ""}</span>
          <span>{elapsedText(useApp.getState().startedAt, now)}</span>
        </div>
      </Card>
      <Card title="Dose rate series">
        <Sparkline data={(series?.dose_rate_usvh ?? history.map((t) => t.dose_rate_usvh)).slice(-600)} />
      </Card>
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
  const { theme, toggleTheme, units, setDoseRateUnit, setDoseUnit, setCountUnit, status, init } = useApp();
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
            <Placeholder title="Map" milestone="M5" body="Geotag surveys from manual pins, gpsd, GPX or .rctrk imports." />
          )}
          {nav === "history" && (
            <Placeholder title="History" milestone="M2 storage / M6 export" body="Sessions are stored in SQLite; browsing and export arrive next." />
          )}
          {nav === "device" && (
            <div className="grid gap-4 lg:grid-cols-2">
              <Card title="Device">
                <DeviceDetails />
              </Card>
              <Placeholder title="Settings" milestone="M6" body="Brightness, sound, alarms and calibration editing land in M6." />
            </div>
          )}
          <p className="mt-6 text-center text-[10px] text-[var(--muted)]">
            M2 · live data over BLE/USB · stored locally in SQLite · no cloud
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
