import { useEffect, useMemo, useRef, useState } from "react";
import { Map as MapLibreMap, Marker, NavigationControl, Popup } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useApp } from "../store";
import { addTrackPoint, exportTrack } from "../lib/ipc";

function doseColor(dose: number) {
  if (dose < 0) return "#8ea1ad";
  if (dose < 0.2) return "#4fd1c5";
  if (dose < 0.6) return "#f0b429";
  if (dose < 2) return "#ef8f5b";
  return "#ef5b5b";
}

export default function MapView() {
  const { track, latest, loadTrack, importTrack, clearTrack, message } = useApp();
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<MapLibreMap | null>(null);
  const fitted = useRef(false);
  const markers = useRef<Marker[]>([]);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [ready, setReady] = useState(false);
  const [filter, setFilter] = useState<[number, number]>([0, 1]);
  const [lat, setLat] = useState("59.9139");
  const [lon, setLon] = useState("10.7522");
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    void loadTrack();
  }, [loadTrack]);

  useEffect(() => {
    if (!container.current || map.current) return;
    try {
      const instance = new MapLibreMap({
        container: container.current,
        style: {
          version: 8,
          sources: {
            osm: {
              type: "raster",
              tiles: ["https://tile.openstreetmap.org/{z}/{x}/{y}.png"],
              tileSize: 256,
              attribution: "© OpenStreetMap contributors",
            },
          },
          layers: [
            { id: "background", type: "background", paint: { "background-color": "#0a0d10" } },
            { id: "osm", type: "raster", source: "osm", paint: { "raster-opacity": 0.55, "raster-saturation": -0.4 } },
          ],
        },
        center: [10.75, 59.91],
        zoom: 11,
        attributionControl: { compact: true },
      });
      instance.addControl(new NavigationControl({ showCompass: false }), "top-right");
      instance.on("load", () => {
        setReady(true);
      });

      map.current = instance;
      setStatus(null);
    } catch (error) {
      setStatus(`Map failed to start: ${String(error)}`);
    }

    return () => {
      map.current?.remove();
      map.current = null;
    };
  }, []);

  const timeBounds = useMemo(() => {
    const stamps = track.map((point) => point.ts).filter((ts) => ts > 0);
    return stamps.length > 1 ? { min: Math.min(...stamps), max: Math.max(...stamps) } : null;
  }, [track]);

  const visibleTrack = useMemo(() => {
    if (!timeBounds) return track;
    const from = timeBounds.min + (timeBounds.max - timeBounds.min) * filter[0];
    const to = timeBounds.min + (timeBounds.max - timeBounds.min) * filter[1];
    return track.filter((point) => point.ts === 0 || (point.ts >= from && point.ts <= to));
  }, [track, timeBounds, filter]);

  useEffect(() => {
    const instance = map.current;
    if (!instance || !ready) return;

    for (const marker of markers.current) marker.remove();
    markers.current = [];

    // DOM markers are used instead of a vector layer because they render on
    // every GL stack (including software rendering) and keep the app usable
    // on machines without GPU acceleration.
    const stride = Math.max(1, Math.ceil(visibleTrack.length / 500));
    for (let index = 0; index < visibleTrack.length; index += stride) {
      const point = visibleTrack[index];
      const element = document.createElement("div");
      element.style.width = "8px";
      element.style.height = "8px";
      element.style.borderRadius = "9999px";
      element.style.background = doseColor(point.dose_rate_usvh ?? -1);
      element.style.border = "1px solid #0a0d10";
      element.style.boxShadow = "0 0 0 1px rgba(79,209,197,0.25)";
      element.style.cursor = "pointer";
      element.title = `${point.dose_rate_usvh != null ? `${point.dose_rate_usvh.toFixed(3)} µSv/h` : "no dose"}${
        point.cps != null ? ` · ${point.cps.toFixed(1)} cps` : ""
      }`;

      element.addEventListener("click", (event) => {
        event.stopPropagation();
        const when = point.ts > 0 ? new Date(point.ts).toLocaleString() : "unknown time";
        new Popup({ closeButton: true, offset: 10 })
          .setLngLat([point.lon, point.lat])
          .setHTML(
            `<div style="font:12px Inter,system-ui;color:#e6edf2;background:#10151a;padding:8px 10px;border-radius:8px">
               <div style="color:#8ea1ad;font-size:10px;text-transform:uppercase;letter-spacing:.1em">measurement</div>
               <div style="margin-top:4px">${when}</div>
               <div style="margin-top:2px">${
                 point.dose_rate_usvh != null ? `${point.dose_rate_usvh.toFixed(3)} µSv/h` : "no dose"
               }${point.cps != null ? ` · ${point.cps.toFixed(1)} cps` : ""}</div>
             </div>`,
          )
          .addTo(instance);
      });

      markers.current.push(new Marker({ element }).setLngLat([point.lon, point.lat]).addTo(instance));
    }

    if (!fitted.current && visibleTrack.length > 0) {
      fitted.current = true;
      const lons = visibleTrack.map((point) => point.lon);
      const lats = visibleTrack.map((point) => point.lat);
      instance.fitBounds(
        [
          [Math.min(...lons), Math.min(...lats)],
          [Math.max(...lons), Math.max(...lats)],
        ],
        { padding: 70, maxZoom: 14, duration: 600 },
      );
    }
  }, [visibleTrack, ready]);

  const stats = useMemo(() => {
    const doses = visibleTrack.map((point) => point.dose_rate_usvh).filter((dose): dose is number => dose != null);
    return {
      count: visibleTrack.length,
      min: doses.length ? Math.min(...doses) : null,
      max: doses.length ? Math.max(...doses) : null,
    };
  }, [visibleTrack]);

  const handleImport = async (file: File) => {
    setBusy(true);
    setStatus(null);
    try {
      const format = file.name.toLowerCase().endsWith(".gpx") ? "gpx" : "rctrk";
      const count = await importTrack(await file.text(), format, file.name);
      fitted.current = false;
      setStatus(`Imported ${count} points from ${file.name}.`);
    } catch (error) {
      setStatus(String(error));
    } finally {
      setBusy(false);
    }
  };

  const handleTag = async () => {
    const latValue = Number(lat);
    const lonValue = Number(lon);
    if (!Number.isFinite(latValue) || !Number.isFinite(lonValue)) {
      setStatus("Enter valid coordinates first.");
      return;
    }
    try {
      await addTrackPoint(latValue, lonValue);
      await loadTrack();
      fitted.current = false;
      setStatus("Tagged the current reading at those coordinates.");
    } catch (error) {
      setStatus(String(error));
    }
  };

  const handleExport = async (format: "geojson" | "gpx" | "csv") => {
    setExporting(true);
    try {
      const path = await exportTrack(format);
      setStatus(`Saved ${path}`);
    } catch (error) {
      setStatus(String(error));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="panel overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--border)] p-3">
        <div>
          <h2 className="text-[11px] font-medium uppercase tracking-[0.16em] text-[var(--muted)]">Survey track</h2>
          <p className="mt-0.5 text-[11px] text-[var(--muted)]">
            {stats.count === 0
              ? "Import a .rctrk track from the phone app, or a GPX file."
              : `${stats.count.toLocaleString()} points${
                  stats.min != null ? ` · ${stats.min.toFixed(3)}–${stats.max?.toFixed(3)} µSv/h` : ""
                }`}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <label
            className={`cursor-pointer rounded-md border border-[var(--border)] px-2.5 py-1.5 text-[11px] ${
              busy ? "opacity-50" : "hover:border-[var(--accent-dim)]"
            }`}
          >
            {busy ? "Importing…" : "Import .rctrk / .gpx"}
            <input
              type="file"
              accept=".rctrk,.gpx,.json"
              className="hidden"
              disabled={busy}
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (file) void handleImport(file);
                event.target.value = "";
              }}
            />
          </label>
          <button
            onClick={() => {
              void clearTrack();
              fitted.current = false;
            }}
            disabled={stats.count === 0}
            className="rounded-md border border-[var(--border)] px-2.5 py-1.5 text-[11px] text-[var(--muted)] hover:text-[var(--text)] disabled:opacity-40"
          >
            Clear
          </button>
          <div className="flex items-center gap-2 rounded-md border border-[var(--border)] px-2 py-1 text-[10px] text-[var(--muted)]">
            <span className="flex items-center gap-1">
              <span className="h-2 w-2 rounded-full" style={{ background: doseColor(0.1) }} /> &lt;0.6
            </span>
            <span className="flex items-center gap-1">
              <span className="h-2 w-2 rounded-full" style={{ background: doseColor(0.8) }} /> 0.6–2
            </span>
            <span className="flex items-center gap-1">
              <span className="h-2 w-2 rounded-full" style={{ background: doseColor(3) }} /> &gt;2 µSv/h
            </span>
          </div>
        </div>
      </div>

      <div className="flex flex-wrap items-end gap-2 border-b border-[var(--border)] px-3 py-2 text-[11px]">
        <label className="flex flex-col gap-1 text-[var(--muted)]">
          Latitude
          <input
            value={lat}
            onChange={(event) => setLat(event.target.value)}
            className="w-24 rounded border border-[var(--border)] bg-[var(--panel-2)] px-2 py-1 text-[var(--text)]"
            inputMode="decimal"
          />
        </label>
        <label className="flex flex-col gap-1 text-[var(--muted)]">
          Longitude
          <input
            value={lon}
            onChange={(event) => setLon(event.target.value)}
            className="w-24 rounded border border-[var(--border)] bg-[var(--panel-2)] px-2 py-1 text-[var(--text)]"
            inputMode="decimal"
          />
        </label>
        <button
          onClick={() => {
            const center = map.current?.getCenter();
            if (center) {
              setLat(center.lat.toFixed(6));
              setLon(center.lng.toFixed(6));
            }
          }}
          className="rounded border border-[var(--border)] px-2 py-1 text-[var(--muted)] hover:text-[var(--text)]"
        >
          Use map centre
        </button>
        <button
          onClick={() => void handleTag()}
          disabled={!latest}
          className="rounded border border-[var(--accent-dim)] px-2 py-1 text-[var(--accent)] disabled:opacity-40"
          title={latest ? "Store the current dose/CPS at these coordinates" : "Connect a device first"}
        >
          Tag current reading
        </button>
        <div className="ml-auto flex items-center gap-1">
          <span className="text-[var(--muted)]">Export</span>
          {(["geojson", "gpx", "csv"] as const).map((format) => (
            <button
              key={format}
              onClick={() => void handleExport(format)}
              disabled={exporting || stats.count === 0}
              className="rounded border border-[var(--border)] px-2 py-1 uppercase text-[var(--muted)] hover:text-[var(--text)] disabled:opacity-40"
            >
              {format === "geojson" ? "GeoJSON" : format}
            </button>
          ))}
        </div>
      </div>

      {timeBounds && (
        <div className="flex flex-wrap items-center gap-3 border-b border-[var(--border)] px-3 py-2 text-[11px] text-[var(--muted)]">
          <span>Time filter</span>
          <input
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={filter[0]}
            onChange={(event) => setFilter([Math.min(Number(event.target.value), filter[1] - 0.01), filter[1]])}
            className="w-40"
            aria-label="Start of time filter"
          />
          <input
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={filter[1]}
            onChange={(event) => setFilter([filter[0], Math.max(Number(event.target.value), filter[0] + 0.01)])}
            className="w-40"
            aria-label="End of time filter"
          />
          <span className="num">
            {new Date(timeBounds.min + (timeBounds.max - timeBounds.min) * filter[0]).toLocaleString()} →{" "}
            {new Date(timeBounds.min + (timeBounds.max - timeBounds.min) * filter[1]).toLocaleString()}
          </span>
          <button
            onClick={() => setFilter([0, 1])}
            className="rounded border border-[var(--border)] px-2 py-0.5 hover:text-[var(--text)]"
          >
            reset
          </button>
        </div>
      )}

      <div className="relative">
        <div ref={container} className="h-[520px] w-full" />
        {track.length === 0 && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
            <div className="rounded-lg border border-[var(--border)] bg-[var(--panel)]/90 px-5 py-4 text-center">
              <p className="text-sm">No track imported yet</p>
              <p className="mt-1 max-w-sm text-[11px] leading-relaxed text-[var(--muted)]">
                The RC-110 has no GPS. Record a track in the Radiacode phone app, export it as .rctrk, then import it
                here — points are coloured by dose rate.
              </p>
            </div>
          </div>
        )}
        {status && (
          <div className="absolute bottom-3 left-3 rounded-md border border-[var(--border)] bg-[var(--panel)] px-3 py-2 text-[11px]">
            {status}
          </div>
        )}
        {message && (
          <div className="absolute bottom-3 right-3 rounded-md border border-[var(--danger)] bg-[var(--panel)] px-3 py-2 text-[11px] text-[var(--danger)]">
            {message}
          </div>
        )}
      </div>
    </div>
  );
}
