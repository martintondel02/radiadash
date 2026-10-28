import { useEffect, useMemo, useState } from "react";

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

function Sparkline({ data, width = 260, height = 56 }: { data: number[]; width?: number; height?: number }) {
  const path = useMemo(() => {
    const max = Math.max(...data);
    const min = Math.min(...data);
    const span = max - min || 1;
    return data
      .map((v, i) => {
        const x = (i / (data.length - 1)) * width;
        const y = height - ((v - min) / span) * (height - 6) - 3;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  }, [data, width, height]);

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full" preserveAspectRatio="none" aria-hidden="true">
      <path d={path} fill="none" stroke="var(--accent)" strokeWidth="1.6" strokeLinejoin="round" />
    </svg>
  );
}

function SpectrumPreview() {
  const bars = useMemo(() => {
    const out: number[] = [];
    for (let i = 0; i < 128; i++) {
      const x = i / 128;
      let v = 12 * Math.exp(-x * 9) + 2.5;
      v += 46 * Math.exp(-Math.pow((x - 0.215) / 0.012, 2)); // 661.7 keV
      v += 14 * Math.exp(-Math.pow((x - 0.475) / 0.014, 2)); // 1460.8 keV
      v += 10 * Math.exp(-Math.pow((x - 0.115) / 0.01, 2)); // 351.9 keV
      out.push(v);
    }
    return out;
  }, []);
  const max = Math.max(...bars);

  return (
    <svg viewBox="0 0 128 64" className="w-full" preserveAspectRatio="none" aria-hidden="true">
      {bars.map((v, i) => (
        <rect key={i} x={i} y={64 - (v / max) * 60} width={0.72} height={(v / max) * 60} fill="var(--accent)" opacity={0.75} />
      ))}
      <line x1="0" y1="63.5" x2="128" y2="63.5" stroke="var(--border)" strokeWidth="0.5" />
    </svg>
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

function Card({ title, children, className = "", action }: { title: string; children: React.ReactNode; className?: string; action?: React.ReactNode }) {
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

function useDemoSeries(n: number, seed = 1) {
  return useMemo(() => {
    let s = seed;
    const rand = () => {
      s = (s * 1664525 + 1013904223) % 4294967296;
      return s / 4294967296;
    };
    return Array.from({ length: n }, (_, i) => 70 + 18 * Math.sin(i / 9) + 8 * (rand() - 0.5));
  }, [n, seed]);
}

export default function App() {
  const [theme, setTheme] = useState<"dark" | "light">("dark");
  const [nav, setNav] = useState<NavKey>("overview");
  const [unit, setUnit] = useState("µSv/h");
  const cps = useDemoSeries(96, 7);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    document.documentElement.classList.toggle("light", theme === "light");
    document.documentElement.style.colorScheme = theme;
  }, [theme]);

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
              className={`group flex items-center justify-between rounded-md px-3 py-2 text-left text-sm transition-colors ${
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
          <div className="rounded-lg border border-[var(--border)] bg-[var(--panel-2)] p-3">
            <div className="flex items-center gap-2 text-xs">
              <span className="h-2 w-2 rounded-full bg-[var(--amber)]" />
              <span className="font-medium">No device</span>
            </div>
            <p className="mt-1.5 text-[11px] leading-relaxed text-[var(--muted)]">
              Bluetooth and USB transports land in M1/M2. Until then the panel shows demo data.
            </p>
            <button className="mt-3 w-full rounded-md border border-[var(--border)] px-2 py-1.5 text-[11px] text-[var(--text)] hover:border-[var(--accent-dim)]">
              Connect device
            </button>
          </div>
          <div className="mt-2 px-1 text-[10px] text-[var(--muted)]">v0.1.0 · M0 scaffold</div>
        </div>
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between gap-4 border-b border-[var(--border)] px-6 py-3">
          <div className="min-w-0">
            <h1 className="truncate text-sm font-medium">{NAV.find((n) => n.key === nav)?.label}</h1>
            <p className="truncate text-[11px] text-[var(--muted)]">{NAV.find((n) => n.key === nav)?.hint}</p>
          </div>
          <div className="flex items-center gap-2">
            <select
              value={unit}
              onChange={(e) => setUnit(e.target.value)}
              className="rounded-md border border-[var(--border)] bg-[var(--panel)] px-2 py-1.5 text-xs text-[var(--text)]"
              aria-label="Dose rate unit"
            >
              <option>µSv/h</option>
              <option>mSv/h</option>
              <option>µR/h</option>
              <option>mrem/h</option>
            </select>
            <button
              onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
              className="rounded-md border border-[var(--border)] px-2.5 py-1.5 text-xs text-[var(--muted)] hover:text-[var(--text)]"
              aria-label="Toggle theme"
            >
              {theme === "dark" ? "Light" : "Dark"}
            </button>
            <span className="rounded-full border border-[var(--border)] px-2.5 py-1 text-[10px] uppercase tracking-wider text-[var(--muted)]">
              demo
            </span>
          </div>
        </header>

        <div className="flex-1 overflow-auto p-6">
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
            <Card
              title="Count rate"
              className="lg:col-span-2"
              action={<span className="num text-[11px] text-[var(--muted)]">last 96 s</span>}
            >
              <Stat label="Live" value="84.2" unit="cps" sub="± 1.1 % · 5 052 cpm" />
              <div className="mt-3">
                <Sparkline data={cps} />
              </div>
            </Card>

            <Card title="Dose rate">
              <Stat label="Current" value="1.094" unit={unit} sub="device: Sv · alarm at 5 µSv/h" />
              <div className="mt-4 h-1.5 w-full overflow-hidden rounded-full bg-[var(--panel-2)]">
                <div className="h-full w-[22%] rounded-full bg-[var(--accent)]" />
              </div>
              <div className="mt-1 flex justify-between text-[10px] text-[var(--muted)]">
                <span>0</span>
                <span>5 µSv/h</span>
              </div>
            </Card>

            <Card title="Spectrum · 1024 ch">
              <SpectrumPreview />
              <div className="mt-2 flex items-center justify-between text-[11px] text-[var(--muted)]">
                <span>0 keV</span>
                <span className="text-[var(--accent)]">Cs-137 · 661.7 keV · 92 %</span>
                <span>3 MeV</span>
              </div>
            </Card>

            <Card title="Accumulated dose">
              <Stat label="Session total" value="0.412" unit="µSv" sub="since 22:04 · 18 min" />
              <div className="mt-3 flex items-center gap-2 text-[11px] text-[var(--muted)]">
                <span className="rounded border border-[var(--border)] px-1.5 py-0.5">reset</span>
                <span>device counter synced</span>
              </div>
            </Card>

            <Card title="Device">
              <dl className="grid grid-cols-2 gap-y-2 text-xs">
                {[
                  ["Model", "RC-110"],
                  ["Serial", "—"],
                  ["Firmware", "—"],
                  ["Battery", "87 %"],
                  ["Temperature", "24.5 °C"],
                  ["Transport", "not connected"],
                ].map(([k, v]) => (
                  <div key={k} className="contents">
                    <dt className="text-[var(--muted)]">{k}</dt>
                    <dd className="num text-right">{v}</dd>
                  </div>
                ))}
              </dl>
            </Card>

            <Card title="Isotope candidates" className="lg:col-span-2">
              <div className="flex flex-wrap gap-2">
                {[
                  { name: "Cs-137", conf: "high", lines: "661.7 · 32 keV X-ray" },
                  { name: "K-40", conf: "high", lines: "1460.8 keV" },
                  { name: "Bi-214", conf: "medium", lines: "609.3 · 1120.3 · 1764.5" },
                  { name: "Pb-214", conf: "medium", lines: "295.2 · 351.9" },
                ].map((iso) => (
                  <div key={iso.name} className="rounded-lg border border-[var(--border)] bg-[var(--panel-2)] px-3 py-2">
                    <div className="flex items-center gap-2">
                      <span className="num text-sm font-medium">{iso.name}</span>
                      <span
                        className={`rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider ${
                          iso.conf === "high" ? "bg-[var(--accent-dim)] text-[var(--text)]" : "border border-[var(--border)] text-[var(--muted)]"
                        }`}
                      >
                        {iso.conf}
                      </span>
                    </div>
                    <div className="mt-1 num text-[10px] text-[var(--muted)]">{iso.lines}</div>
                  </div>
                ))}
              </div>
              <p className="mt-3 text-[10px] leading-relaxed text-[var(--muted)]">
                Candidate identification from peak matching. Not a certified measurement — always verify with a calibrated instrument.
              </p>
            </Card>
          </div>

          <p className="mt-6 text-center text-[10px] text-[var(--muted)]">
            M0 scaffold · charts and widgets are placeholders until the protocol core is wired to the UI (M2).
          </p>
        </div>
      </main>
    </div>
  );
}
