/** Display-only unit conversion. Canonical values are µSv/h and µSv. */

export const DOSE_RATE_UNITS = ["µSv/h", "mSv/h", "nSv/h", "µR/h", "mR/h", "mrem/h"] as const;
export type DoseRateUnit = (typeof DOSE_RATE_UNITS)[number];

export const DOSE_UNITS = ["µSv", "mSv", "µR", "mrem"] as const;
export type DoseUnit = (typeof DOSE_UNITS)[number];

export const COUNT_UNITS = ["cps", "cpm"] as const;
export type CountUnit = (typeof COUNT_UNITS)[number];

export function convertDoseRate(usvh: number, unit: DoseRateUnit): number {
  switch (unit) {
    case "µSv/h":
      return usvh;
    case "mSv/h":
      return usvh / 1000;
    case "nSv/h":
      return usvh * 1000;
    case "µR/h":
      return usvh * 100;
    case "mR/h":
      return usvh / 10;
    case "mrem/h":
      return usvh / 10;
  }
}

export function convertDose(usv: number, unit: DoseUnit): number {
  switch (unit) {
    case "µSv":
      return usv;
    case "mSv":
      return usv / 1000;
    case "µR":
      return usv * 100;
    case "mrem":
      return usv / 10;
  }
}

export function convertCount(cps: number, unit: CountUnit): number {
  return unit === "cps" ? cps : cps * 60;
}

/** Sensible precision for a value whose display unit is already chosen. */
export function formatValue(value: number): { text: string; decimals: number } {
  const abs = Math.abs(value);
  if (abs === 0) return { text: "0.000", decimals: 3 };
  if (abs >= 1000) return { text: value.toFixed(0), decimals: 0 };
  if (abs >= 100) return { text: value.toFixed(1), decimals: 1 };
  if (abs >= 1) return { text: value.toFixed(2), decimals: 2 };
  if (abs >= 0.01) return { text: value.toFixed(3), decimals: 3 };
  return { text: value.toExponential(2), decimals: 3 };
}
