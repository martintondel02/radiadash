import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Series, Snapshot, SpectrumPayload, Telemetry, DeviceIdentity } from "./types";

export type TransportChoice =
  | { kind: "auto" }
  | { kind: "ble"; address?: string | null }
  | { kind: "usb"; serial?: string | null };

export interface DeviceList {
  usb: { vendor_id: number; product_id: number; serial: string | null; bus: string }[];
  ble: { address: string; name: string }[];
}

export const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function getSnapshot(): Promise<Snapshot> {
  return invoke<Snapshot>("get_snapshot");
}

export async function connectDevice(transport: TransportChoice, scanSecs = 8): Promise<void> {
  return invoke("connect_device", { request: { transport, scan_secs: scanSecs } });
}

export async function disconnectDevice(): Promise<void> {
  return invoke("disconnect_device");
}

export async function getSeries(rangeSeconds: number): Promise<Series> {
  return invoke<Series>("get_series", { rangeSeconds });
}

export async function getLatestSpectrum(): Promise<SpectrumPayload | null> {
  return invoke<SpectrumPayload | null>("get_latest_spectrum");
}

export async function listDevices(): Promise<DeviceList> {
  return invoke<DeviceList>("list_devices");
}

export async function resetDose(): Promise<void> {
  return invoke("reset_dose");
}

export async function setSpectrumInterval(seconds: number): Promise<void> {
  return invoke("set_spectrum_interval", { seconds });
}

export function onTelemetry(cb: (value: Telemetry) => void): Promise<UnlistenFn> {
  return listen<Telemetry>("telemetry", (event) => cb(event.payload));
}

export function onConnection(cb: (value: Snapshot) => void): Promise<UnlistenFn> {
  return listen<Snapshot>("connection", (event) => cb(event.payload));
}

export function onSpectrum(cb: (value: SpectrumPayload) => void): Promise<UnlistenFn> {
  return listen<SpectrumPayload>("spectrum", (event) => cb(event.payload));
}

export function onDeviceError(cb: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("device-error", (event) => cb(event.payload));
}

export type { DeviceIdentity, Series, Snapshot, SpectrumPayload, Telemetry };
