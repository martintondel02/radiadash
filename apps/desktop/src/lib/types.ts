export interface FirmwareVersion {
  major: number;
  minor: number;
  date: string;
}

export interface DeviceIdentity {
  transport: string;
  serial: string;
  hw_serial: string;
  firmware: FirmwareVersion;
  boot: FirmwareVersion;
  model: string | null;
  spectrum_format: number;
  config: Record<string, string>;
}

export interface UnitFlags {
  dose_in_sievert: boolean;
  use_nsv_h: boolean;
  counts_per_minute: boolean;
}

export interface Telemetry {
  ts: number;
  cps: number;
  cps_err: number;
  dose_rate_usvh: number;
  dose_rate_err: number;
  dose_usv: number | null;
  temp_c: number | null;
  charge: number | null;
  flags: number;
  session_id: number;
}

export interface SpectrumPayload {
  ts: number;
  duration_s: number;
  a0: number;
  a1: number;
  a2: number;
  counts: number[];
}

export interface Snapshot {
  status: "disconnected" | "connecting" | "connected" | "error" | "demo";
  message: string | null;
  identity: DeviceIdentity | null;
  latest: Telemetry | null;
  session_id: number | null;
  started_at: number | null;
  spectrum_interval_s: number;
  dose_scale: "roentgen" | "micro_sievert" | null;
  unit_flags: UnitFlags | null;
  spectrum: SpectrumPayload | null;
  analysis: Analysis | null;
}

export interface Series {
  ts: number[];
  cps: number[];
  dose_rate_usvh: number[];
}

export interface Peak {
  channel: number;
  energy_kev: number;
  net: number;
  net_err: number;
  significance: number;
  fwhm_kev: number;
  gross: number;
  background: number;
}

export interface Evidence {
  line_energy_kev: number;
  intensity: number;
  matched: boolean;
  peak_energy_kev: number | null;
  delta_kev: number | null;
  significance: number | null;
}

export interface Candidate {
  id: string;
  name: string;
  half_life: string;
  category: string;
  confidence: "high" | "medium" | "low" | "none";
  score: number;
  coverage: number;
  matched_lines: number;
  total_lines: number;
  evidence: Evidence[];
}

export interface Analysis {
  peaks: Peak[];
  candidates: Candidate[];
  library_version: string;
  duration_s: number;
  total_counts: number;
}
