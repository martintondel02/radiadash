//! `radiadash-probe` — inspect and record a RadiaCode device from the terminal.
//!
//! Examples:
//!   radiadash-probe list
//!   radiadash-probe info
//!   radiadash-probe monitor --duration 60
//!   radiadash-probe spectrum --out spectrum.json
//!   radiadash-probe record --duration 300 --out session.jsonl
//!   radiadash-probe sim

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use clap::{Args, Parser, Subcommand};
use radiadash_core::spectrum::{ascii_plot, find_peaks};
use radiadash_core::types::{Record, Spectrum};
use radiadash_core::{OpenOptions, RadiaCode, Transport, ble, usb};
use radiadash_sim::{demo_records, demo_spectrum};

#[derive(Parser, Debug)]
#[command(
    name = "radiadash-probe",
    version,
    about = "Probe and record RadiaCode 10x devices (BLE first, USB fallback)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List USB devices and scan for Bluetooth devices.
    List {
        #[arg(long, default_value_t = 8)]
        scan_secs: u64,
    },
    /// Print device identity, configuration and unit registers.
    Info(Conn),
    /// Print individual registers and a sample of raw records.
    Regs(Conn),
    /// Stream live measurements.
    Monitor(MonitorArgs),
    /// Fetch a spectrum, list peaks and optionally save it as JSON.
    Spectrum(SpectrumArgs),
    /// Analyse a spectrum and print isotope candidates with evidence.
    Analyze(AnalyzeArgs),
    /// Record records and spectra to a JSONL file.
    Record(RecordArgs),
    /// Show a synthetic spectrum without a device.
    Sim {
        #[arg(long, default_value_t = 120)]
        duration: u64,
        #[arg(long, default_value_t = 7)]
        seed: u64,
    },
}

#[derive(Args, Debug, Clone)]
struct Conn {
    /// Bluetooth address (e.g. AA:BB:CC:DD:EE:FF).
    #[arg(long, value_name = "MAC")]
    ble: Option<String>,
    /// Use USB only.
    #[arg(long)]
    usb: bool,
    /// Select a USB device by serial number.
    #[arg(long)]
    serial: Option<String>,
    /// Continue even if firmware < 4.8.
    #[arg(long)]
    ignore_fw: bool,
    /// How long to scan for Bluetooth devices.
    #[arg(long, default_value_t = 10)]
    scan_secs: u64,
}

#[derive(Args, Debug)]
struct MonitorArgs {
    #[command(flatten)]
    conn: Conn,
    /// Seconds to stream (0 = forever).
    #[arg(long, default_value_t = 30)]
    duration: u64,
    /// Poll interval in seconds.
    #[arg(long, default_value_t = 1.0)]
    interval: f64,
    /// Append records as JSON lines.
    #[arg(long)]
    out: Option<String>,
}

#[derive(Args, Debug)]
struct SpectrumArgs {
    #[command(flatten)]
    conn: Conn,
    /// Use the accumulated spectrum instead of the live one.
    #[arg(long)]
    accum: bool,
    /// Number of peaks to print.
    #[arg(long, default_value_t = 8)]
    top: usize,
    /// Save the spectrum as JSON.
    #[arg(long)]
    out: Option<String>,
    /// Print an ASCII plot.
    #[arg(long)]
    plot: bool,
}

#[derive(Args, Debug)]
struct AnalyzeArgs {
    #[command(flatten)]
    conn: Conn,
    /// Maximum candidates to print.
    #[arg(long, default_value_t = 6)]
    top: usize,
    /// Use the accumulated spectrum.
    #[arg(long)]
    accum: bool,
}

#[derive(Args, Debug)]
struct RecordArgs {
    #[command(flatten)]
    conn: Conn,
    /// Seconds to record (0 = forever).
    #[arg(long, default_value_t = 120)]
    duration: u64,
    /// Output JSONL file.
    #[arg(long)]
    out: String,
    /// Save a spectrum every N seconds (0 disables).
    #[arg(long, default_value_t = 10)]
    spectrum_every: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "radiadash_core=warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().command {
        Command::List { scan_secs } => list(scan_secs).await,
        Command::Info(args) => info(args).await,
        Command::Regs(args) => regs(args).await,
        Command::Monitor(args) => monitor(args).await,
        Command::Spectrum(args) => spectrum(args).await,
        Command::Analyze(args) => analyze(args).await,
        Command::Record(args) => record(args).await,
        Command::Sim { duration, seed } => {
            sim(duration, seed);
            Ok(())
        }
    }
}

async fn connect(args: &Conn) -> Result<RadiaCode> {
    let options = OpenOptions {
        allow_old_firmware: args.ignore_fw,
    };

    if let Some(address) = &args.ble {
        let transport = ble::BleTransport::open(Some(address), "radia", args.scan_secs)
            .await
            .with_context(|| format!("connecting to Bluetooth device {address}"))?;
        println!("transport: ble {address}");
        return Ok(RadiaCode::open_with(Box::new(transport), options).await?);
    }

    if !args.usb {
        match ble::BleTransport::open(None, "radia", args.scan_secs).await {
            Ok(transport) => {
                println!("transport: {}", transport.label());
                match RadiaCode::open_with(Box::new(transport), options).await {
                    Ok(device) => return Ok(device),
                    Err(err) => eprintln!("bluetooth session failed ({err}), falling back to USB"),
                }
            }
            Err(err) => eprintln!("bluetooth scan failed ({err}), falling back to USB"),
        }
    }

    let transport = usb::UsbTransport::open(args.serial.as_deref())
        .await
        .context("opening USB device (is the udev rule installed?)")?;
    println!("transport: {}", transport.label());
    Ok(RadiaCode::open_with(Box::new(transport), options).await?)
}

async fn list(scan_secs: u64) -> Result<()> {
    println!("USB devices (looking for 0483:f123):");
    let devices = usb::list().await?;
    if devices.is_empty() {
        println!("  (none connected)");
    }
    for info in &devices {
        println!(
            "  ✔ {:04x}:{:04x}  serial={}  bus={}",
            info.vendor_id,
            info.product_id,
            info.serial.as_deref().unwrap_or("?"),
            info.bus
        );
    }

    println!("\nBluetooth scan ({scan_secs}s):");
    match ble::scan(scan_secs).await {
        Ok(devices) if devices.is_empty() => println!("  (no devices found)"),
        Ok(devices) => {
            for (address, name) in devices {
                println!("  {address}  {name}");
            }
        }
        Err(err) => println!("  scan unavailable: {err}"),
    }
    Ok(())
}

fn fmt_opt(v: Option<u32>) -> String {
    v.map(|v| v.to_string())
        .unwrap_or_else(|| "invalid/unavailable".into())
}

/// Dump individual registers plus a raw record sample. Used during bring-up to
/// discover which registers a firmware actually implements.
async fn regs(conn: Conn) -> Result<()> {
    let mut device = connect(&conn).await?;
    let units = device.read_units().await?;
    println!(
        "units: dose={} nSv_mode={} count={}",
        if units.dose_in_sievert { "Sv" } else { "R" },
        units.use_nsv_h,
        if units.counts_per_minute { "cpm" } else { "cps" }
    );

    let candidates = [
        ("CPS", 0x8020u32),
        ("DR_uR_h", 0x8021),
        ("DS_uR", 0x8022),
        ("TEMP_degC", 0x8024),
        ("DS_UNITS", 0x8004),
        ("USE_nSv_h", 0x800C),
        ("CR_UNITS", 0x8013),
        ("CPS_FILTER", 0x8005),
        ("VBIAS_mV", 0xC000),
    ];
    for (name, id) in candidates {
        match device.read_register_opt(id).await {
            Ok(Some(raw)) => {
                let f = f32::from_bits(raw);
                println!(
                    "  {name:<10} 0x{id:04x} = {raw:>12}  (u32 {} / f32 {:.4})",
                    raw, f
                );
            }
            Ok(None) => println!("  {name:<10} 0x{id:04x} = invalid"),
            Err(err) => println!("  {name:<10} 0x{id:04x} = error: {err}"),
        }
    }

    let records = device.data_buf().await?;
    println!("\nbuffered records: {}", records.len());
    let mut shown = 0;
    for record in records.iter().rev().take(400) {
        match record {
            Record::RealTime(r) => {
                println!(
                    "  realtime cps={:.4} err={:.2}% dose_rate={:.9} raw_bits={:#010x} err={:.2}% flags=0x{:04x}",
                    r.count_rate,
                    r.count_rate_err,
                    r.dose_rate,
                    r.dose_rate.to_bits(),
                    r.dose_rate_err,
                    r.flags
                );
                shown += 1;
            }
            Record::Rare(r) => {
                println!(
                    "  rare     dose={} temp={:.3}C charge={:.2}% duration={}s flags=0x{:04x}",
                    r.dose, r.temperature_c, r.charge_percent, r.duration, r.flags
                );
                shown += 1;
            }
            _ => {}
        }
        if shown >= 6 {
            break;
        }
    }
    Ok(())
}

async fn info(conn: Conn) -> Result<()> {
    let mut device = connect(&conn).await?;
    let identity = device.identify().await?;
    let units = device.read_units().await?;
    let live = device.live_registers().await?;
    let calibration = device.energy_calibration().await?;
    let status = device.status_flags().await?;

    println!("\nidentity");
    println!("  transport        {}", identity.transport);
    println!("  serial           {}", identity.serial);
    println!("  hardware serial  {}", identity.hw_serial);
    println!(
        "  model            {}",
        identity.model.as_deref().unwrap_or("(not reported)")
    );
    println!(
        "  firmware         {}.{} ({})",
        identity.firmware.major, identity.firmware.minor, identity.firmware.date
    );
    println!(
        "  bootloader       {}.{} ({})",
        identity.boot.major, identity.boot.minor, identity.boot.date
    );
    println!("  spectrum format  {}", identity.spectrum_format);
    println!("  status flags     0x{status:08x}");

    println!("\ncalibration (keV = a0 + a1*ch + a2*ch^2)");
    println!(
        "  a0={:.6}  a1={:.6}  a2={:.9}",
        calibration[0], calibration[1], calibration[2]
    );

    println!("\nunit configuration");
    println!(
        "  dose unit        {}",
        if units.dose_in_sievert { "Sv" } else { "R" }
    );
    println!("  nSv/h mode       {}", units.use_nsv_h);
    println!(
        "  count unit       {}",
        if units.counts_per_minute { "cpm" } else { "cps" }
    );

    println!("\nlive registers");
    println!("  CPS              {}", fmt_opt(live.cps));
    println!("  DR (raw)         {}", fmt_opt(live.dose_rate_ur_h));
    println!("  DS (raw)         {}", fmt_opt(live.dose_ur));
    println!(
        "  temperature      {}",
        live.temperature_c
            .map(|t| format!("{t:.2} °C"))
            .unwrap_or_else(|| "unavailable".into())
    );

    if let (Some(dr), Some(ds)) = (live.dose_rate_ur_h, live.dose_ur) {
        println!("\ninterpretation (compare with the device display)");
        println!(
            "  if raw is µR:    {:.4} µSv/h   {:.4} µSv accumulated",
            dr as f64 / 100.0,
            ds as f64 / 100.0
        );
        println!(
            "  if raw is µSv:   {:.4} µSv/h   {:.4} µSv accumulated",
            dr as f64, ds as f64
        );
        println!(
            "  if raw is nSv:   {:.4} µSv/h   {:.4} µSv accumulated",
            dr as f64 / 1000.0,
            ds as f64 / 1000.0
        );
    }

    if !identity.config.is_empty() {
        println!("\nconfiguration");
        for (key, value) in &identity.config {
            println!("  {key} = {value}");
        }
    }

    Ok(())
}

fn format_record(record: &Record) -> Option<String> {
    match record {
        Record::RealTime(r) => Some(format!(
            "{}  cps={:8.2} ±{:5.1}%  dose={:e} R/h (≈{:7.4} µSv/h)  flags=0x{:04x}",
            r.dt.format("%H:%M:%S"),
            r.count_rate,
            r.count_rate_err,
            r.dose_rate,
            r.dose_rate as f64 * 1e4,
            r.flags
        )),
        Record::Rare(r) => Some(format!(
            "{}  RARE   dose={:.6} R ({:.2} µSv as f32)  word={}  T={:.2} °C  battery={:.0}%  uptime={}s",
            r.dt.format("%H:%M:%S"),
            r.dose_float,
            r.dose_float as f64 * 1e4,
            r.dose,
            r.temperature_c,
            r.charge_percent,
            r.duration
        )),
        Record::DoseRateDb(r) => Some(format!(
            "{}  HISTORY count={} cps={:.2} raw={:.2}",
            r.dt.format("%H:%M:%S"),
            r.count,
            r.count_rate,
            r.dose_rate
        )),
        Record::Event(e) => Some(format!(
            "{}  EVENT  id={} param={}",
            e.dt.format("%H:%M:%S"),
            e.event,
            e.event_param1
        )),
        Record::Raw(_) => None,
    }
}

async fn monitor(args: MonitorArgs) -> Result<()> {
    let mut device = connect(&args.conn).await?;
    let mut out = match &args.out {
        Some(path) => Some(std::fs::File::create(path).with_context(|| format!("create {path}"))?),
        None => None,
    };

    // Drain whatever the device buffered while disconnected.
    let mut drained = 0usize;
    for _ in 0..50 {
        match device.data_buf().await {
            Ok(records) if !records.is_empty() => {
                drained += records.len();
                if drained > 20_000 {
                    break;
                }
            }
            _ => break,
        }
    }
    if drained > 0 {
        eprintln!("discarded {drained} buffered records");
    }
    let fresh_after = Local::now() - chrono::Duration::seconds(15);

    let start = Instant::now();
    let interval = Duration::from_secs_f64(args.interval.max(0.2));
    println!("time      measurement");
    loop {
        for record in device.data_buf().await? {
            if record.dt() < fresh_after {
                continue;
            }
            if let Some(line) = format_record(&record) {
                println!("{line}");
            }
            if let Some(file) = out.as_mut() {
                use std::io::Write;
                writeln!(file, "{}", serde_json::to_string(&record)?)?;
            }
        }

        if args.duration > 0 && start.elapsed() >= Duration::from_secs(args.duration) {
            break;
        }
        tokio::time::sleep(interval).await;
    }

    Ok(())
}

fn print_spectrum(spectrum: &Spectrum, top: usize, plot: bool) {
    println!(
        "spectrum: {} channels, {:.0}s live time, {} counts ({:.2} cps)",
        spectrum.counts.len(),
        spectrum.duration_s,
        spectrum.total_counts(),
        spectrum.cps()
    );
    println!(
        "calibration: a0={:.4} a1={:.6} a2={:.9}",
        spectrum.a0, spectrum.a1, spectrum.a2
    );
    let peaks = find_peaks(spectrum, 15.0, top);
    if !peaks.is_empty() {
        println!("\npeaks (naive search)");
        for peak in &peaks {
            println!(
                "  ch {:4}  {:8.2} keV  counts={:6}  prominence={:7.1}  fwhm≈{:.1} ch",
                peak.channel, peak.energy_kev, peak.counts, peak.prominence, peak.fwhm_channels
            );
        }
    }
    if plot {
        println!("\n{}", ascii_plot(spectrum, 96, 16));
    }
}

async fn spectrum(args: SpectrumArgs) -> Result<()> {
    let mut device = connect(&args.conn).await?;
    let spectrum = if args.accum {
        device.spectrum_accumulated().await?
    } else {
        device.spectrum().await?
    };

    print_spectrum(&spectrum, args.top, args.plot || args.out.is_none());

    if let Some(path) = &args.out {
        std::fs::write(path, serde_json::to_vec_pretty(&spectrum)?)
            .with_context(|| format!("write {path}"))?;
        println!("\nsaved {path}");
    }
    Ok(())
}

async fn analyze(args: AnalyzeArgs) -> Result<()> {
    let mut device = connect(&args.conn).await?;
    let spectrum = if args.accum {
        device.spectrum_accumulated().await?
    } else {
        device.spectrum().await?
    };
    let analysis = radiadash_core::isotope::analyze(&spectrum);

    println!(
        "spectrum: {} counts, {:.0}s live, library {}",
        analysis.total_counts, analysis.duration_s, analysis.library_version
    );
    println!("\npeaks ({} above threshold)", analysis.peaks.len());
    for peak in analysis.peaks.iter().take(12) {
        println!(
            "  {:8.2} keV  net={:8.0} ±{:.0}  significance={:5.1}  fwhm={:.1} keV",
            peak.energy_kev, peak.net, peak.net_err, peak.significance, peak.fwhm_kev
        );
    }

    println!("\ncandidates");
    for candidate in analysis.candidates.iter().take(args.top) {
        println!(
            "  {:<8} {:<8} score={:.2} coverage={:.0}%  lines {}/{}  {}",
            candidate.id,
            candidate.confidence,
            candidate.score,
            candidate.coverage * 100.0,
            candidate.matched_lines,
            candidate.total_lines,
            candidate.half_life
        );
        for evidence in &candidate.evidence {
            if evidence.matched {
                println!(
                    "      ✔ {:8.2} keV ({:5.1}%) → peak {:8.2} keV  Δ={:+.2}  sig={:.1}",
                    evidence.line_energy_kev,
                    evidence.intensity,
                    evidence.peak_energy_kev.unwrap_or(0.0),
                    evidence.delta_kev.unwrap_or(0.0),
                    evidence.significance.unwrap_or(0.0)
                );
            } else if evidence.intensity >= 5.0 {
                println!(
                    "      ✘ {:8.2} keV ({:5.1}%) not seen",
                    evidence.line_energy_kev, evidence.intensity
                );
            }
        }
    }
    Ok(())
}

async fn record(args: RecordArgs) -> Result<()> {
    use std::io::Write;

    let mut device = connect(&args.conn).await?;
    let mut file = std::fs::File::create(&args.out).with_context(|| format!("create {}", args.out))?;
    let _ = device.data_buf().await;

    let start = Instant::now();
    let mut last_spectrum = Instant::now();
    let mut records_written = 0usize;

    loop {
        for record in device.data_buf().await? {
            writeln!(file, "{}", serde_json::to_string(&record)?)?;
            records_written += 1;
        }

        if args.spectrum_every > 0 && last_spectrum.elapsed() >= Duration::from_secs(args.spectrum_every) {
            last_spectrum = Instant::now();
            match device.spectrum().await {
                Ok(spectrum) => {
                    let envelope = serde_json::json!({
                        "kind": "spectrum",
                        "saved_at": Local::now().to_rfc3339(),
                        "spectrum": spectrum,
                    });
                    writeln!(file, "{envelope}")?;
                }
                Err(err) => eprintln!("spectrum read failed: {err}"),
            }
        }

        if args.duration > 0 && start.elapsed() >= Duration::from_secs(args.duration) {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    file.flush()?;
    println!("wrote {records_written} records to {}", args.out);
    Ok(())
}

fn sim(duration: u64, seed: u64) {
    let start = Local::now();
    for record in demo_records(start, 5, 84.0, 1.09, seed) {
        if let Some(line) = format_record(&record) {
            println!("{line}");
        }
    }
    let spectrum = demo_spectrum(duration, seed);
    println!();
    print_spectrum(&spectrum, 8, true);
}
