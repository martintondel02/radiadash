import { useEffect, useState } from "react";

type PageKey = "overview" | "spectrum" | "rate" | "map" | "history" | "device" | "general";

interface Section {
  heading: string;
  body: string;
  steps?: string[];
}

interface Guide {
  title: string;
  intro: string;
  sections: Section[];
}

const GUIDES: Record<PageKey, Guide> = {
  general: {
    title: "Getting started",
    intro: "Radiadash connects to a RadiaCode 110 over Bluetooth or USB and keeps everything on this machine.",
    sections: [
      {
        heading: "Connect a device",
        body: "The app tries Bluetooth first, then USB, when it starts. You can also connect manually from the sidebar.",
        steps: [
          "Turn the detector on and keep it within a metre of the computer.",
          "Click Connect in the sidebar for auto mode, or open transport options to force USB or Bluetooth.",
          "The first spectrum appears within a minute; the status pill turns teal when connected.",
        ],
      },
      {
        heading: "If the device is not found",
        body: "USB needs a udev rule so the app can claim the vendor interface without root; Bluetooth needs the device free (close the phone app).",
        steps: [
          "Check the cable and that the detector shows the charging/data screen.",
          "Run `radiadash-probe list` in a terminal to see what the app can see.",
          "For USB permission errors, install `packaging/60-radiacode.rules` under /etc/udev/rules.d and reload udev.",
        ],
      },
      {
        heading: "Privacy and limits",
        body: "All measurements, spectra and tracks are stored locally in SQLite under ~/.local/share/io.github.martintondel.radiadash. Map tiles are fetched from OpenStreetMap when the map is open.",
        steps: [
          "Isotope results are candidates from peak matching, not a certified identification.",
          "Dose values are converted from the device's roentgen-based output; treat them as indicative.",
          "The Geiger sound is a sonification of the measured count rate, not a second measurement.",
        ],
      },
    ],
  },
  overview: {
    title: "Overview",
    intro: "The default workspace: live numbers, a rolling count-rate chart, the current spectrum and isotope candidates.",
    sections: [
      {
        heading: "Reading the cards",
        body: "Count rate shows CPS with its percentage error; dose rate is converted into the unit you pick; accumulated dose comes from the device's own counter. Device shows battery, temperature and transport.",
      },
      {
        heading: "Zooming the charts",
        body: "Every chart is interactive. The visible window is independent per chart.",
        steps: [
          "Scroll or pinch with the trackpad to zoom around the cursor.",
          "Drag left/right to pan when zoomed in.",
          "Use − and + for stepped zoom, and ⟲ to reset.",
        ],
      },
      {
        heading: "Units, density and sound",
        body: "The header switches dose-rate, dose and count-rate units independently; the layout selector changes grid density; the Sound button toggles the Geiger audio (volume is on the Device page).",
      },
    ],
  },
  spectrum: {
    title: "Spectrum and isotopes",
    intro: "The spectrum view shows the measured energy histogram and ranks isotope candidates against a local gamma-line library.",
    sections: [
      {
        heading: "Reading the plot",
        body: "Bars are counts on a log scale. Colour emphasises stronger peaks (teal → amber). The energy axis follows the zoom window, and detected peaks are listed below the chart.",
        steps: [
          "Scroll to zoom into a region, drag to pan, ⟲ to reset.",
          "Hover the candidate list to switch which library lines are drawn on the spectrum.",
        ],
      },
      {
        heading: "Fingerprint overlay",
        body: "Amber sticks are library lines that matched a detected peak; dashed grey sticks are lines the library expects but the spectrum does not show. A nuclide with many missing strong lines should be treated sceptically.",
      },
      {
        heading: "Candidates and confidence",
        body: "Score combines intensity-weighted line coverage with energy agreement. High confidence needs two or more matched lines with good coverage, or one very strong line close to its expected energy. Single-line nuclides (K-40, Cs-137) can only ever reach medium unless the line is strong and exact.",
      },
      {
        heading: "Calibration hint",
        body: "The Δ column is the measured minus expected energy. A median offset larger than a few keV means the device calibration is drifting — the Device page can fit a replacement, but verify the anchors first.",
      },
    ],
  },
  rate: {
    title: "Rate and dose",
    intro: "Time series from the live buffer and the local database, plus distributions.",
    sections: [
      {
        heading: "Ranges",
        body: "5m and 1h read the current session; 6h and 24h query SQLite so they survive restarts. The chart downsamples automatically for long ranges.",
      },
      {
        heading: "Distributions",
        body: "Histograms show the spread of CPS and dose rate with mean and standard deviation. A Poisson-like spread is expected at low count rates; unusually wide distributions often mean the detector moved or the source varied.",
      },
      {
        heading: "Zoom",
        body: "Wheel/pinch zooms inside the loaded data, drag pans, and the ± buttons step the window. Zoom does not re-query the database.",
      },
    ],
  },
  map: {
    title: "Survey map",
    intro: "The RC-110 has no GPS, so locations come from imported tracks or from tagging the current reading.",
    sections: [
      {
        heading: "Import a track",
        body: "Record a survey in the Radiacode phone app, export the .rctrk file, then import it here. GPX files from phones or GPS loggers also work.",
        steps: [
          "Open Map and click Import .rctrk / .gpx.",
          "Pick the exported file; points appear coloured by dose rate.",
          "Click a point to see time, dose rate and count rate.",
        ],
      },
      {
        heading: "Tag the current reading",
        body: "Move the map to a location, use “Use map centre” or type coordinates, then click Tag current reading. The point stores the live dose rate and CPS with a timestamp.",
      },
      {
        heading: "Time filter and export",
        body: "The two sliders limit which points are shown and counted. Export writes GeoJSON, GPX or CSV to ~/Downloads (or the app data folder if Downloads is missing) and reports the full path.",
      },
      {
        heading: "Colours and accuracy",
        body: "Teal below 0.2 µSv/h, amber below 0.6, orange below 2, red above; grey means the source had no dose value. Imported .rctrk dose rates are assumed to be µSv/h.",
      },
    ],
  },
  device: {
    title: "Device and calibration",
    intro: "Identity, calibration and audio settings.",
    sections: [
      {
        heading: "Calibration wizard",
        body: "The fit uses identified peaks as anchors. It is a diagnostic first and a write second — a bad calibration is worse than a small offset.",
        steps: [
          "Collect a spectrum with several known lines (Cs-137, K-40, Co-60 are ideal).",
          "Click Fit from identified peaks and check the table plus the RMS.",
          "If RMS is high (>8 keV), do not apply — some lines were misidentified.",
          "Record the current coefficients, then Apply to device and confirm. New spectra use the new calibration.",
        ],
      },
      {
        heading: "Geiger audio",
        body: "Clicks are generated from the measured CPS with Poisson jitter. Volume and mute are here; the header Sound button is a quick toggle.",
      },
      {
        heading: "Device facts",
        body: "Battery readings on the RC-110 are noisy and may jump; treat them as approximate. Rare records (battery, temperature, accumulated dose) arrive infrequently, so values are seeded from the last stored record at connect.",
      },
    ],
  },
  history: {
    title: "History",
    intro: "Sessions and samples are stored in SQLite; browsing and export arrive in a later milestone.",
    sections: [
      {
        heading: "What is stored",
        body: "Each connection creates a session. Samples (1 Hz), rare records, spectra and events are written to ~/.local/share/io.github.martintondel.radiadash/radiadash.db.",
      },
      {
        heading: "Using the data now",
        body: "Rate & Dose ranges above 5 minutes already read from the database. For raw access, open the SQLite file with any client; the schema is documented in docs/architecture.md.",
      },
    ],
  },
};

const ORDER: PageKey[] = ["general", "overview", "spectrum", "rate", "map", "device", "history"];

const SHORT: Record<PageKey, string> = {
  general: "Start",
  overview: "Overview",
  spectrum: "Spectrum",
  rate: "Rate",
  map: "Map",
  device: "Device",
  history: "History",
};

export default function GuideDrawer({
  page,
  open,
  onClose,
}: {
  page: PageKey;
  open: boolean;
  onClose: () => void;
}) {
  const [active, setActive] = useState<PageKey>(page);

  useEffect(() => {
    if (open) setActive(page);
  }, [open, page]);

  useEffect(() => {
    if (!open) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;
  const guide = GUIDES[active];

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button className="absolute inset-0 bg-black/45" aria-label="Close guide" onClick={onClose} />
      <aside className="relative z-10 flex h-full w-full max-w-lg flex-col border-l border-[var(--border)] bg-[var(--panel)] shadow-2xl">
        <header className="flex items-start justify-between gap-3 border-b border-[var(--border)] px-5 py-4">
          <div>
            <div className="text-[10px] uppercase tracking-[0.16em] text-[var(--muted)]">Guide</div>
            <h2 className="mt-0.5 text-base font-medium">{guide.title}</h2>
          </div>
          <button
            onClick={onClose}
            className="rounded border border-[var(--border)] px-2 py-1 text-xs text-[var(--muted)] hover:text-[var(--text)]"
          >
            close
          </button>
        </header>

        <nav className="flex flex-wrap gap-1 border-b border-[var(--border)] px-4 py-2">
          {ORDER.map((key) => (
            <button
              key={key}
              onClick={() => setActive(key)}
              className={`rounded-full px-2.5 py-1 text-[10px] uppercase tracking-wider ${
                active === key
                  ? "bg-[var(--panel-2)] text-[var(--text)]"
                  : "text-[var(--muted)] hover:text-[var(--text)]"
              }`}
            >
              {SHORT[key]}
            </button>
          ))}
        </nav>

        <div className="flex-1 overflow-auto px-5 py-4">
          <p className="text-sm leading-relaxed text-[var(--text)]">{guide.intro}</p>
          <div className="mt-4 space-y-5">
            {guide.sections.map((section) => (
              <section key={section.heading}>
                <h3 className="text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--accent)]">
                  {section.heading}
                </h3>
                <p className="mt-1.5 text-[12px] leading-relaxed text-[var(--muted)]">{section.body}</p>
                {section.steps && (
                  <ol className="mt-2 list-decimal space-y-1 pl-5 text-[12px] leading-relaxed text-[var(--muted)]">
                    {section.steps.map((step) => (
                      <li key={step}>{step}</li>
                    ))}
                  </ol>
                )}
              </section>
            ))}
          </div>
          <p className="mt-6 text-[10px] leading-relaxed text-[var(--muted)]">
            The same guides live in <span className="num">docs/user-guide.md</span> in the repository.
          </p>
        </div>
      </aside>
    </div>
  );
}
