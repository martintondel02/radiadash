import { create } from "zustand";
import * as ipc from "./lib/ipc";
import type { Analysis, Series, Snapshot, SpectrumPayload, Telemetry, TrackPoint } from "./lib/types";
import type { CountUnit, DoseRateUnit, DoseUnit } from "./lib/units";

const HISTORY_LIMIT = 900;

interface Units {
  doseRate: DoseRateUnit;
  dose: DoseUnit;
  count: CountUnit;
}

interface AppStore {
  status: Snapshot["status"];
  message: string | null;
  identity: Snapshot["identity"];
  latest: Telemetry | null;
  history: Telemetry[];
  spectrum: SpectrumPayload | null;
  analysis: Analysis | null;
  track: TrackPoint[];
  calibration: [number, number, number] | null;
  density: "comfortable" | "compact" | "focus";
  sound: boolean;
  volume: number;
  series: Series | null;
  startedAt: number | null;
  units: Units;
  theme: "dark" | "light";
  connecting: boolean;
  init: () => Promise<void>;
  connect: (transport: ipc.TransportChoice) => Promise<void>;
  disconnect: () => Promise<void>;
  refreshSeries: (rangeSeconds: number) => Promise<void>;
  resetDose: () => Promise<void>;
  loadTrack: () => Promise<void>;
  importTrack: (text: string, format: "rctrk" | "gpx", source?: string) => Promise<number>;
  clearTrack: () => Promise<void>;
  setDensity: (density: "comfortable" | "compact" | "focus") => void;
  setSound: (sound: boolean) => void;
  setVolume: (volume: number) => void;
  setDoseRateUnit: (unit: DoseRateUnit) => void;
  setDoseUnit: (unit: DoseUnit) => void;
  setCountUnit: (unit: CountUnit) => void;
  toggleTheme: () => void;
}

function demoTelemetry(elapsed: number): Telemetry {
  const noise = (Math.random() - 0.5) * 2;
  const cps = Math.max(0, 7.8 + noise * 0.6 + Math.sin(elapsed / 40) * 0.4);
  return {
    ts: Date.now(),
    cps,
    cps_err: 4.4,
    dose_rate_usvh: 0.0395 + noise * 0.002,
    dose_rate_err: 15.1,
    dose_usv: 56 + elapsed * 0.04,
    temp_c: 22.1,
    charge: 83,
    flags: 0x4040,
    session_id: -1,
  };
}

function demoSpectrum(): SpectrumPayload {
  const counts = new Array(1024).fill(0).map((_, ch) => {
    const base = 60 * Math.exp(-ch / 90) + 3;
    const peak = (center: number, amp: number, sigma: number) =>
      amp * Math.exp(-Math.pow((ch - center) / sigma, 2) / 2);
    return Math.round(base + peak(28, 4200, 6) + peak(120, 380, 7) + peak(277, 1800, 9) + peak(616, 420, 12));
  });
  return { ts: Date.now(), duration_s: 900, a0: 4.282, a1: 2.367718, a2: 0.000342982, counts };
}

function demoAnalysis(): Analysis {
  const evidence = (lines: [number, number][], matched: boolean) =>
    lines.map(([energy, intensity], index) => ({
      line_energy_kev: energy,
      intensity,
      matched: matched && index < 2,
      peak_channel: matched && index < 2 ? Math.round(energy / 3) : null,
      peak_energy_kev: matched && index < 2 ? energy - 2 : null,
      delta_kev: matched && index < 2 ? -2 : null,
      significance: matched && index < 2 ? 24 : null,
    }));
  return {
    peaks: [],
    candidates: [
      { id: "Cs-137", name: "Cesium-137", half_life: "30.17 y", category: "fallout", confidence: "high", score: 0.92, coverage: 1, matched_lines: 1, total_lines: 1, evidence: evidence([[661.657, 85.1]], true) },
      { id: "K-40", name: "Potassium-40", half_life: "1.25e9 y", category: "norm", confidence: "medium", score: 0.64, coverage: 1, matched_lines: 1, total_lines: 1, evidence: evidence([[1460.822, 10.66]], true) },
      { id: "Bi-214", name: "Bismuth-214", half_life: "19.9 min", category: "norm", confidence: "medium", score: 0.41, coverage: 0.4, matched_lines: 3, total_lines: 6, evidence: evidence([[609.312, 45.5], [1764.494, 15.3], [1120.294, 14.9]], true) },
      { id: "Tl-208", name: "Thallium-208", half_life: "3.05 min", category: "norm", confidence: "medium", score: 0.36, coverage: 0.5, matched_lines: 2, total_lines: 4, evidence: evidence([[2614.511, 99.8], [583.191, 85.0]], true) },
    ],
    library_version: "2026.10-r1",
    duration_s: 900,
    total_counts: 80157,
  };
}

export const useApp = create<AppStore>((set, get) => ({
  status: "disconnected",
  message: null,
  identity: null,
  latest: null,
  history: [],
  spectrum: null,
  analysis: null,
  track: [],
  calibration: null,
  density: (localStorage.getItem("radiadash-density") as "comfortable" | "compact" | "focus") ?? "comfortable",
  sound: localStorage.getItem("radiadash-sound") === "1",
  volume: Number(localStorage.getItem("radiadash-volume") ?? "0.35"),
  series: null,
  startedAt: null,
  units: { doseRate: "µSv/h", dose: "µSv", count: "cps" },
  theme: (localStorage.getItem("radiadash-theme") as "dark" | "light") ?? "dark",
  connecting: false,

  init: async () => {
    if (!ipc.isDesktop) {
      let elapsed = 0;
      const seedHistory = Array.from({ length: 300 }, () => {
        elapsed += 1;
        return demoTelemetry(elapsed);
      });
      set({
        status: "demo",
        startedAt: Date.now() - 1000 * 60 * 18,
        latest: { ...seedHistory[seedHistory.length - 1], session_id: 0 },
        history: seedHistory,
        spectrum: demoSpectrum(),
        analysis: demoAnalysis(),
      });
      setInterval(() => {
        elapsed += 1;
        const telemetry = demoTelemetry(elapsed);
        set((state) => ({
          latest: { ...telemetry, session_id: 0 },
          history: [...state.history, telemetry].slice(-HISTORY_LIMIT),
        }));
      }, 1000);
      return;
    }

    const unlisten = await Promise.all([
      ipc.onTelemetry((telemetry) =>
        set((state) => ({
          latest: telemetry,
          history: [...state.history, telemetry].slice(-HISTORY_LIMIT),
        })),
      ),
      ipc.onConnection((snapshot) => {
        set({
          status: snapshot.status,
          message: snapshot.message,
          identity: snapshot.identity,
          startedAt: snapshot.started_at,
          spectrum: snapshot.spectrum ?? get().spectrum,
          latest: snapshot.latest ?? get().latest,
          calibration: snapshot.calibration ?? get().calibration,
        });
        if (snapshot.status === "connected") {
          void get().refreshSeries(3600);
        }
      }),
      ipc.onSpectrum((spectrum) => set({ spectrum })),
      ipc.onAnalysis((analysis) => set({ analysis })),
      ipc.onCalibration((calibration) => set({ calibration })),
      ipc.onDeviceError((message) => set({ message })),
    ]);
    void unlisten;

    try {
      const snapshot = await ipc.getSnapshot();
      set({
        status: snapshot.status,
        message: snapshot.message,
        identity: snapshot.identity,
        latest: snapshot.latest,
        startedAt: snapshot.started_at,
        spectrum: snapshot.spectrum,
      });
      const spectrum = await ipc.getLatestSpectrum();
      if (spectrum) set({ spectrum });
      const analysis = await ipc.getLatestAnalysis();
      if (analysis) set({ analysis });
      if (snapshot.calibration) set({ calibration: snapshot.calibration });
      if (snapshot.status === "connected") await get().refreshSeries(3600);
    } catch (error) {
      set({ status: "error", message: String(error) });
    }
  },

  connect: async (transport) => {
    set({ connecting: true, message: null });
    try {
      await ipc.connectDevice(transport);
    } catch (error) {
      set({ connecting: false, status: "error", message: String(error) });
    } finally {
      set({ connecting: false });
    }
  },

  disconnect: async () => {
    try {
      await ipc.disconnectDevice();
      set({ status: "disconnected", identity: null, latest: null, startedAt: null });
    } catch (error) {
      set({ message: String(error) });
    }
  },

  refreshSeries: async (rangeSeconds) => {
    try {
      const series = await ipc.getSeries(rangeSeconds);
      set({ series });
    } catch (error) {
      set({ message: String(error) });
    }
  },

  resetDose: async () => {
    try {
      await ipc.resetDose();
    } catch (error) {
      set({ message: String(error) });
    }
  },

  loadTrack: async () => {
    if (!ipc.isDesktop) return;
    try {
      set({ track: await ipc.getTrack() });
    } catch (error) {
      set({ message: String(error) });
    }
  },

  importTrack: async (text, format, source) => {
    const count = await ipc.importTrack(text, format, source);
    await get().loadTrack();
    return count;
  },

  clearTrack: async () => {
    await ipc.clearTrack();
    set({ track: [] });
  },

  setDensity: (density) =>
    set(() => {
      localStorage.setItem("radiadash-density", density);
      return { density };
    }),
  setSound: (sound) =>
    set(() => {
      localStorage.setItem("radiadash-sound", sound ? "1" : "0");
      return { sound };
    }),
  setVolume: (volume) =>
    set(() => {
      localStorage.setItem("radiadash-volume", String(volume));
      return { volume };
    }),
  setDoseRateUnit: (doseRate) => set((state) => ({ units: { ...state.units, doseRate } })),
  setDoseUnit: (dose) => set((state) => ({ units: { ...state.units, dose } })),
  setCountUnit: (count) => set((state) => ({ units: { ...state.units, count } })),
  toggleTheme: () =>
    set((state) => {
      const theme = state.theme === "dark" ? "light" : "dark";
      localStorage.setItem("radiadash-theme", theme);
      return { theme };
    }),
}));
