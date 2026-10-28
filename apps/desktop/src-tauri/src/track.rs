//! Import of geotagged survey tracks (`.rctrk` from the RadiaCode app, GPX).
//!
//! The RC-110 has no GPS, so surveys are recorded on a phone and imported here.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TrackPoint {
    pub ts: i64,
    pub lat: f64,
    pub lon: f64,
    pub dose_rate_usvh: Option<f64>,
    pub cps: Option<f64>,
    pub source: String,
}

fn number(value: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(|v| v.as_f64()))
}

fn timestamp(value: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        let Some(raw) = value.get(*key) else { continue };
        if let Some(number) = raw.as_f64() {
            let number = number as i64;
            return Some(if number > 1_000_000_000_000 {
                number
            } else {
                number * 1000
            });
        }
        if let Some(text) = raw.as_str() {
            if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(text) {
                return Some(parsed.timestamp_millis());
            }
        }
    }
    None
}

/// Parse a `.rctrk` file (JSON with a `markers` array, or a bare array).
pub fn parse_rctrk(text: &str, source: &str) -> anyhow::Result<Vec<TrackPoint>> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let markers = value
        .get("markers")
        .and_then(|v| v.as_array())
        .or_else(|| value.as_array())
        .ok_or_else(|| anyhow::anyhow!("no markers array in .rctrk file"))?;

    let mut points = Vec::new();
    for marker in markers {
        let Some(lat) = number(marker, &["lat", "latitude"]) else {
            continue;
        };
        let Some(lon) = number(marker, &["lon", "lng", "longitude"]) else {
            continue;
        };
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            continue;
        }
        points.push(TrackPoint {
            ts: timestamp(marker, &["date", "timestamp", "time"]).unwrap_or(0),
            lat,
            lon,
            dose_rate_usvh: number(marker, &["doseRate", "dose_rate", "dose_rate_usvh"]),
            cps: number(marker, &["countRate", "count_rate", "cps"]),
            source: source.to_string(),
        });
    }
    Ok(points)
}

/// Minimal GPX parser: `trkpt` and `wpt` elements with lat/lon attributes.
pub fn parse_gpx(text: &str, source: &str) -> anyhow::Result<Vec<TrackPoint>> {
    let mut points = Vec::new();
    let mut rest = text;

    while let Some(start) = rest.find("<trkpt").or_else(|| rest.find("<wpt")) {
        let slice = &rest[start..];
        let end = slice.find('>').unwrap_or(slice.len());
        let tag = &slice[..end];
        rest = &slice[end..];

        let attribute = |name: &str| -> Option<f64> {
            let pattern = format!("{name}=\"");
            let index = tag.find(&pattern)? + pattern.len();
            let tail = &tag[index..];
            let end = tail.find('"')?;
            tail[..end].parse().ok()
        };

        let (Some(lat), Some(lon)) = (attribute("lat"), attribute("lon")) else {
            continue;
        };
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            continue;
        }

        let ts = rest
            .find("<time>")
            .and_then(|index| {
                let tail = &rest[index + 6..];
                let end = tail.find("</time>")?;
                chrono::DateTime::parse_from_rfc3339(tail[..end].trim()).ok()
            })
            .map(|time| time.timestamp_millis())
            .unwrap_or(0);

        points.push(TrackPoint {
            ts,
            lat,
            lon,
            dose_rate_usvh: None,
            cps: None,
            source: source.to_string(),
        });
    }
    Ok(points)
}

fn iso_time(ts: i64) -> String {
    if ts <= 0 {
        return String::new();
    }
    chrono::DateTime::from_timestamp_millis(ts)
        .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

/// GeoJSON FeatureCollection of the imported track.
pub fn to_geojson(points: &[TrackPoint]) -> String {
    let features: Vec<serde_json::Value> = points
        .iter()
        .map(|point| {
            serde_json::json!({
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [point.lon, point.lat] },
                "properties": {
                    "time": iso_time(point.ts),
                    "timestamp_ms": point.ts,
                    "dose_rate_usvh": point.dose_rate_usvh,
                    "cps": point.cps,
                    "source": point.source,
                }
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "type": "FeatureCollection",
        "features": features,
    }))
    .unwrap_or_else(|_| "{\"type\":\"FeatureCollection\",\"features\":[]}".to_string())
}

/// Minimal GPX track.
pub fn to_gpx(points: &[TrackPoint]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"Radiadash\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n  <trk>\n    <name>Radiadash survey</name>\n    <trkseg>\n",
    );
    for point in points {
        let time = iso_time(point.ts);
        if time.is_empty() {
            out.push_str(&format!(
                "      <trkpt lat=\"{:.7}\" lon=\"{:.7}\"></trkpt>\n",
                point.lat, point.lon
            ));
        } else {
            out.push_str(&format!(
                "      <trkpt lat=\"{:.7}\" lon=\"{:.7}\"><time>{}</time></trkpt>\n",
                point.lat, point.lon, time
            ));
        }
    }
    out.push_str("    </trkseg>\n  </trk>\n</gpx>\n");
    out
}

/// CSV with one row per point.
pub fn to_csv(points: &[TrackPoint]) -> String {
    let mut out = String::from("timestamp_ms,time,lat,lon,dose_rate_usvh,cps,source\n");
    for point in points {
        out.push_str(&format!(
            "{},{},{:.7},{:.7},{},{},{}\n",
            point.ts,
            iso_time(point.ts),
            point.lat,
            point.lon,
            point
                .dose_rate_usvh
                .map(|v| format!("{v:.6}"))
                .unwrap_or_default(),
            point.cps.map(|v| format!("{v:.3}")).unwrap_or_default(),
            point.source.replace(',', " "),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rctrk_markers() {
        let text = r#"{"markers":[
            {"lat":59.91,"lon":10.75,"doseRate":0.08,"countRate":6.1,"date":1793224021},
            {"lat":59.92,"lon":10.76,"doseRate":0.11,"countRate":8.4,"date":1793224031}
        ]}"#;
        let points = parse_rctrk(text, "test").unwrap();
        assert_eq!(points.len(), 2);
        assert!((points[0].dose_rate_usvh.unwrap() - 0.08).abs() < 1e-9);
        assert_eq!(points[0].ts, 1_793_224_021_000);
    }

    #[test]
    fn skips_invalid_coordinates() {
        let text = r#"[{"lat":999,"lon":10},{"lat":59.9,"lon":10.7}]"#;
        assert_eq!(parse_rctrk(text, "test").unwrap().len(), 1);
    }

    #[test]
    fn exports_geojson_gpx_and_csv() {
        let points = parse_rctrk(
            r#"[{"lat":59.9,"lon":10.7,"doseRate":0.1,"date":1793224021}]"#,
            "unit",
        )
        .unwrap();
        let geojson = to_geojson(&points);
        assert!(geojson.contains("FeatureCollection"));
        assert!(geojson.contains("\"dose_rate_usvh\": 0.1"));
        let gpx = to_gpx(&points);
        assert_eq!(gpx.matches("<trkpt").count(), 1);
        assert!(gpx.contains("2026-10-"));
        let csv = to_csv(&points);
        assert!(csv.starts_with("timestamp_ms,time,lat,lon"));
        assert_eq!(csv.lines().count(), 2);
    }

    #[test]
    fn parses_gpx_track() {
        let text = r#"<?xml version="1.0"?><gpx><trk><trkseg>
            <trkpt lat="59.91" lon="10.75"><time>2026-10-28T21:00:00Z</time></trkpt>
            <trkpt lat="59.92" lon="10.76"></trkpt>
        </trkseg></trk></gpx>"#;
        let points = parse_gpx(text, "test").unwrap();
        assert_eq!(points.len(), 2);
        assert!(points[0].ts > 0);
        assert_eq!(points[1].ts, 0);
    }
}
