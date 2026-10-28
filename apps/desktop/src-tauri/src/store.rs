//! SQLite persistence for sessions, samples, rare records and spectra.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS sessions (
    id           INTEGER PRIMARY KEY,
    started_at   INTEGER NOT NULL,
    ended_at     INTEGER,
    device       TEXT NOT NULL,
    transport    TEXT NOT NULL,
    name         TEXT
);

CREATE TABLE IF NOT EXISTS samples (
    id             INTEGER PRIMARY KEY,
    session_id     INTEGER NOT NULL,
    ts             INTEGER NOT NULL,
    cps            REAL NOT NULL,
    cps_err        REAL,
    dose_rate_usvh REAL NOT NULL,
    dose_rate_err  REAL,
    flags          INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS samples_ts ON samples(ts);
CREATE INDEX IF NOT EXISTS samples_session_ts ON samples(session_id, ts);

CREATE TABLE IF NOT EXISTS rare (
    id         INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL,
    ts         INTEGER NOT NULL,
    dose_usv   REAL NOT NULL,
    temp_c     REAL,
    charge     REAL,
    flags      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS rare_ts ON rare(ts);

CREATE TABLE IF NOT EXISTS spectra (
    id         INTEGER PRIMARY KEY,
    session_id INTEGER,
    ts         INTEGER NOT NULL,
    duration_s INTEGER NOT NULL,
    a0         REAL NOT NULL,
    a1         REAL NOT NULL,
    a2         REAL NOT NULL,
    counts     BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS spectra_ts ON spectra(ts);

CREATE TABLE IF NOT EXISTS track_points (
    id             INTEGER PRIMARY KEY,
    imported_at    INTEGER NOT NULL,
    ts             INTEGER NOT NULL,
    lat            REAL NOT NULL,
    lon            REAL NOT NULL,
    dose_rate_usvh REAL,
    cps            REAL,
    source         TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS track_points_ts ON track_points(ts);

CREATE TABLE IF NOT EXISTS events (
    id         INTEGER PRIMARY KEY,
    session_id INTEGER,
    ts         INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    detail     TEXT
);
"#;

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct StoredSpectrum {
    pub ts: i64,
    pub duration_s: u64,
    pub a0: f32,
    pub a1: f32,
    pub a2: f32,
    pub counts: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub session_id: i64,
    pub ts: i64,
    pub cps: f64,
    pub cps_err: f64,
    pub dose_rate_usvh: f64,
    pub dose_rate_err: f64,
    pub flags: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct RareSnapshot {
    #[allow(dead_code)]
    pub ts: i64,
    pub dose_usv: f64,
    pub temp_c: f32,
    pub charge: f32,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct Series {
    pub ts: Vec<i64>,
    pub cps: Vec<f64>,
    pub dose_rate_usvh: Vec<f64>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Close sessions left open by a crash or forced shutdown.
    pub fn close_stale_sessions(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET ended_at = ?1 WHERE ended_at IS NULL",
            params![chrono::Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn start_session(&self, device: &str, transport: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (started_at, device, transport) VALUES (?1, ?2, ?3)",
            params![chrono::Utc::now().timestamp_millis(), device, transport],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn end_session(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET ended_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().timestamp_millis(), id],
        )?;
        Ok(())
    }

    pub fn insert_sample(&self, sample: Sample) -> Result<()> {
        self.conn.execute(
            "INSERT INTO samples (session_id, ts, cps, cps_err, dose_rate_usvh, dose_rate_err, flags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                sample.session_id,
                sample.ts,
                sample.cps,
                sample.cps_err,
                sample.dose_rate_usvh,
                sample.dose_rate_err,
                sample.flags as i64
            ],
        )?;
        Ok(())
    }

    pub fn insert_rare(
        &self,
        session_id: i64,
        ts: i64,
        dose_usv: f64,
        temp_c: f32,
        charge: f32,
        flags: u16,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO rare (session_id, ts, dose_usv, temp_c, charge, flags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![session_id, ts, dose_usv, temp_c, charge, flags as i64],
        )?;
        Ok(())
    }

    pub fn insert_spectrum(
        &self,
        session_id: Option<i64>,
        ts: i64,
        spectrum: &radiadash_core::types::Spectrum,
    ) -> Result<()> {
        let mut blob = Vec::with_capacity(spectrum.counts.len() * 4);
        for count in &spectrum.counts {
            blob.extend_from_slice(&count.to_le_bytes());
        }
        self.conn.execute(
            "INSERT INTO spectra (session_id, ts, duration_s, a0, a1, a2, counts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session_id,
                ts,
                spectrum.duration_s as i64,
                spectrum.a0 as f64,
                spectrum.a1 as f64,
                spectrum.a2 as f64,
                blob
            ],
        )?;
        Ok(())
    }

    pub fn insert_track(&self, points: &[crate::track::TrackPoint]) -> Result<usize> {
        let imported_at = chrono::Utc::now().timestamp_millis();
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut statement = tx.prepare(
                "INSERT INTO track_points (imported_at, ts, lat, lon, dose_rate_usvh, cps, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for point in points {
                statement.execute(params![
                    imported_at,
                    point.ts,
                    point.lat,
                    point.lon,
                    point.dose_rate_usvh,
                    point.cps,
                    point.source
                ])?;
            }
        }
        tx.commit()?;
        Ok(points.len())
    }

    pub fn track(&self) -> Result<Vec<crate::track::TrackPoint>> {
        let mut statement = self
            .conn
            .prepare("SELECT ts, lat, lon, dose_rate_usvh, cps, source FROM track_points ORDER BY ts ASC")?;
        let mut rows = statement.query([])?;
        let mut points = Vec::new();
        while let Some(row) = rows.next()? {
            points.push(crate::track::TrackPoint {
                ts: row.get(0)?,
                lat: row.get(1)?,
                lon: row.get(2)?,
                dose_rate_usvh: row.get(3)?,
                cps: row.get(4)?,
                source: row.get(5)?,
            });
        }
        Ok(points)
    }

    pub fn clear_track(&self) -> Result<()> {
        self.conn.execute("DELETE FROM track_points", [])?;
        Ok(())
    }

    pub fn latest_rare(&self) -> Result<Option<RareSnapshot>> {
        let mut stmt = self
            .conn
            .prepare("SELECT ts, dose_usv, temp_c, charge FROM rare ORDER BY ts DESC LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(RareSnapshot {
                ts: row.get(0)?,
                dose_usv: row.get(1)?,
                temp_c: row.get(2)?,
                charge: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn latest_spectrum(&self) -> Result<Option<StoredSpectrum>> {
        let mut stmt = self
            .conn
            .prepare("SELECT ts, duration_s, a0, a1, a2, counts FROM spectra ORDER BY ts DESC LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let blob: Vec<u8> = row.get(5)?;
            let counts = blob
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            Ok(Some(StoredSpectrum {
                ts: row.get(0)?,
                duration_s: row.get::<_, i64>(1)? as u64,
                a0: row.get::<_, f64>(2)? as f32,
                a1: row.get::<_, f64>(3)? as f32,
                a2: row.get::<_, f64>(4)? as f32,
                counts,
            }))
        } else {
            Ok(None)
        }
    }

    /// Downsampled series for the last `range_ms`, bucketed to about 600 points.
    pub fn series(&self, range_ms: i64) -> Result<Series> {
        let since = chrono::Utc::now().timestamp_millis() - range_ms;
        let mut stmt = self
            .conn
            .prepare("SELECT ts, cps, dose_rate_usvh FROM samples WHERE ts >= ?1 ORDER BY ts ASC")?;
        let mut rows = stmt.query(params![since])?;

        let mut raw: Vec<(i64, f64, f64)> = Vec::new();
        while let Some(row) = rows.next()? {
            raw.push((row.get(0)?, row.get(1)?, row.get(2)?));
        }
        if raw.is_empty() {
            return Ok(Series::default());
        }

        // Bucket size: aim for <= 600 points, at least 1 s.
        let span = (raw.last().unwrap().0 - raw[0].0).max(1);
        let bucket = (span / 600).max(1000);
        let mut series = Series::default();
        let mut current_bucket = i64::MIN;
        let mut n = 0.0;
        let mut cps_sum = 0.0;
        let mut dose_sum = 0.0;
        for (ts, cps, dose) in raw {
            let b = ts / bucket * bucket;
            if b != current_bucket {
                if n > 0.0 {
                    series.ts.push(current_bucket);
                    series.cps.push(cps_sum / n);
                    series.dose_rate_usvh.push(dose_sum / n);
                }
                current_bucket = b;
                n = 0.0;
                cps_sum = 0.0;
                dose_sum = 0.0;
            }
            n += 1.0;
            cps_sum += cps;
            dose_sum += dose;
        }
        if n > 0.0 {
            series.ts.push(current_bucket);
            series.cps.push(cps_sum / n);
            series.dose_rate_usvh.push(dose_sum / n);
        }
        Ok(series)
    }

    #[allow(dead_code)]
    pub fn session_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?)
    }

    #[allow(dead_code)]
    pub fn sample_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM samples", [], |r| r.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radiadash_core::types::Spectrum;

    fn temp_db() -> Db {
        Db::open(Path::new(":memory:")).expect("in-memory db")
    }

    #[test]
    fn session_and_samples_roundtrip() {
        let db = temp_db();
        let sid = db.start_session("RC-110-TEST", "usb").unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        for i in 0..10 {
            db.insert_sample(Sample {
                session_id: sid,
                ts: now - 9_000 + 1_000 * i,
                cps: 7.5,
                cps_err: 4.4,
                dose_rate_usvh: 0.04,
                dose_rate_err: 15.0,
                flags: 0,
            })
            .unwrap();
        }
        assert_eq!(db.sample_count().unwrap(), 10);
        let series = db.series(3_600_000).unwrap();
        assert!(!series.ts.is_empty());
        assert!(series.cps.iter().all(|c| (*c - 7.5).abs() < 1e-9));
        db.end_session(sid).unwrap();
    }

    #[test]
    fn spectrum_blob_roundtrip() {
        let db = temp_db();
        let spectrum = Spectrum {
            duration_s: 60,
            a0: 1.0,
            a1: 2.0,
            a2: 0.0,
            counts: (0..1024).collect(),
        };
        db.insert_spectrum(None, 42, &spectrum).unwrap();
        let stored = db.latest_spectrum().unwrap().unwrap();
        assert_eq!(stored.counts.len(), 1024);
        assert_eq!(stored.counts[1023], 1023);
        assert_eq!(stored.duration_s, 60);
    }
}
