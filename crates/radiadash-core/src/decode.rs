//! Decoders for the device's virtual strings.

use chrono::{DateTime, Duration, Local};

use crate::error::{Error, Result};
use crate::protocol::Cursor;
use crate::types::*;

/// Decode the mixed record stream returned by `VS::DATA_BUF`.
///
/// Timestamps are relative to `base` in 10 ms units.
pub fn decode_data_buf(data: &[u8], base: DateTime<Local>) -> Vec<Record> {
    let mut out = Vec::new();
    let mut cur = Cursor::new(data);
    let mut next_seq: Option<u8> = None;

    while cur.remaining() >= 7 {
        let seq = match cur.u8() {
            Ok(v) => v,
            Err(_) => break,
        };
        let eid = match cur.u8() {
            Ok(v) => v,
            Err(_) => break,
        };
        let gid = match cur.u8() {
            Ok(v) => v,
            Err(_) => break,
        };
        let ts_offset = match cur.i32() {
            Ok(v) => v,
            Err(_) => break,
        };
        let dt = base + Duration::milliseconds(ts_offset as i64 * 10);

        if let Some(expected) = next_seq
            && expected != seq
        {
            // Sequence jump: the buffer lost records, stop at the gap.
            break;
        }
        next_seq = Some(seq.wrapping_add(1));

        let record = match (eid, gid) {
            (0, 0) => {
                let count_rate = cur.f32();
                let dose_rate = cur.f32();
                let count_rate_err = cur.u16();
                let dose_rate_err = cur.u16();
                let flags = cur.u16();
                let real_time_flags = cur.u8();
                match (
                    count_rate,
                    dose_rate,
                    count_rate_err,
                    dose_rate_err,
                    flags,
                    real_time_flags,
                ) {
                    (
                        Ok(count_rate),
                        Ok(dose_rate),
                        Ok(count_rate_err),
                        Ok(dose_rate_err),
                        Ok(flags),
                        Ok(rt),
                    ) => Record::RealTime(RealTimeData {
                        dt,
                        count_rate,
                        count_rate_err: count_rate_err as f32 / 10.0,
                        dose_rate,
                        dose_rate_err: dose_rate_err as f32 / 10.0,
                        flags,
                        real_time_flags: rt,
                    }),
                    _ => break,
                }
            }
            (0, 1) => match (cur.f32(), cur.f32()) {
                (Ok(count_rate), Ok(dose_rate)) => Record::Raw(RawData {
                    dt,
                    count_rate,
                    dose_rate,
                }),
                _ => break,
            },
            (0, 2) => {
                let count = cur.u32();
                let count_rate = cur.f32();
                let dose_rate = cur.f32();
                let dose_rate_err = cur.u16();
                let flags = cur.u16();
                match (count, count_rate, dose_rate, dose_rate_err, flags) {
                    (Ok(count), Ok(count_rate), Ok(dose_rate), Ok(err), Ok(flags)) => {
                        Record::DoseRateDb(DoseRateDb {
                            dt,
                            count,
                            count_rate,
                            dose_rate,
                            dose_rate_err: err as f32 / 10.0,
                            flags,
                        })
                    }
                    _ => break,
                }
            }
            (0, 3) => {
                let duration = cur.u32();
                let dose = cur.u32();
                let temperature = cur.u16();
                let charge = cur.u16();
                let flags = cur.u16();
                match (duration, dose, temperature, charge, flags) {
                    (Ok(duration), Ok(dose), Ok(temperature), Ok(charge), Ok(flags)) => {
                        Record::Rare(RareData {
                            dt,
                            duration,
                            dose,
                            dose_float: f32::from_bits(dose),
                            temperature_c: (temperature as f32 - 2000.0) / 100.0,
                            charge_percent: charge as f32 / 100.0,
                            flags,
                        })
                    }
                    _ => break,
                }
            }
            (0, 7) => {
                let event = cur.u8();
                let param = cur.u8();
                let flags = cur.u16();
                match (event, param, flags) {
                    (Ok(event), Ok(event_param1), Ok(flags)) => Record::Event(Event {
                        dt,
                        event,
                        event_param1,
                        flags,
                    }),
                    _ => break,
                }
            }
            (0, 4) | (0, 5) => {
                // User/schedule data: same layout as DoseRateDB but not surfaced.
                if cur.skip(4 + 4 + 4 + 2 + 2).is_err() {
                    break;
                }
                continue;
            }
            (0, 6) => {
                if cur.skip(6).is_err() {
                    break;
                }
                continue;
            }
            (0, 8) => {
                if cur.skip(4 + 2).is_err() {
                    break;
                }
                continue;
            }
            (0, 9) => {
                if cur.skip(4 + 2).is_err() {
                    break;
                }
                continue;
            }
            (1, 1) | (1, 2) | (1, 3) => {
                let samples = cur.u16();
                let _sample_time = cur.u32();
                let Some(samples) = samples.ok() else { break };
                let stride = match gid {
                    1 => 8,
                    2 => 16,
                    _ => 14,
                };
                if cur.skip(samples as usize * stride).is_err() {
                    break;
                }
                continue;
            }
            _ => break,
        };
        out.push(record);
    }

    out
}

/// Decode a spectrum payload.
///
/// Format 0 stores plain `u32` counts. Format 1 uses a nibble-coded run-length
/// scheme: each `u16` says how many values follow and how they are encoded.
pub fn decode_spectrum(data: &[u8], format_version: u8) -> Result<Spectrum> {
    let mut cur = Cursor::new(data);
    let ts = cur.u32()?;
    let a0 = cur.f32()?;
    let a1 = cur.f32()?;
    let a2 = cur.f32()?;

    let counts = match format_version {
        0 => decode_counts_v0(&mut cur)?,
        1 => decode_counts_v1(&mut cur)?,
        other => {
            return Err(Error::Unsupported(format!("spectrum format {other}")));
        }
    };

    Ok(Spectrum {
        duration_s: ts as u64,
        a0,
        a1,
        a2,
        counts,
    })
}

fn decode_counts_v0(cur: &mut Cursor<'_>) -> Result<Vec<u32>> {
    let mut counts = Vec::new();
    while cur.remaining() >= 4 {
        counts.push(cur.u32()?);
    }
    Ok(counts)
}

fn decode_counts_v1(cur: &mut Cursor<'_>) -> Result<Vec<u32>> {
    let mut counts: Vec<u32> = Vec::new();
    let mut last: i64 = 0;

    while cur.remaining() >= 2 {
        let word = cur.u16()?;
        let run = ((word >> 4) & 0x0FFF) as usize;
        let vlen = (word & 0x000F) as u8;

        for _ in 0..run {
            let value = match vlen {
                0 => 0,
                1 => cur.u8()? as i64,
                2 => last + cur.i8()? as i64,
                3 => last + cur.i16()? as i64,
                4 => {
                    let a = cur.u8()? as i64;
                    let b = cur.u8()? as i64;
                    let c = cur.i8()? as i64;
                    last + ((c << 16) | (b << 8) | a)
                }
                5 => last + cur.i32()? as i64,
                other => {
                    return Err(Error::Protocol(format!(
                        "unsupported spectrum delta width {other}"
                    )));
                }
            };
            last = value;
            counts.push(value.max(0) as u32);
        }
    }

    Ok(counts)
}

/// Decode `GET_VERSION`: boot version, then target (application) version.
pub fn decode_fw_version(data: &[u8]) -> Result<(FirmwareVersion, FirmwareVersion)> {
    let mut cur = Cursor::new(data);
    let boot_minor = cur.u16()?;
    let boot_major = cur.u16()?;
    let boot_date = cur.string()?;
    let target_minor = cur.u16()?;
    let target_major = cur.u16()?;
    let target_date = cur.string()?;

    Ok((
        FirmwareVersion {
            major: boot_major,
            minor: boot_minor,
            date: boot_date,
        },
        FirmwareVersion {
            major: target_major,
            minor: target_minor,
            date: target_date,
        },
    ))
}

/// Decode `GET_SERIAL`: a length followed by 32-bit groups.
pub fn decode_hw_serial(data: &[u8]) -> Result<String> {
    let mut cur = Cursor::new(data);
    let len = cur.u32()? as usize;
    if !len.is_multiple_of(4) {
        return Err(Error::Protocol(format!("bad hardware serial length {len}")));
    }
    let mut parts = Vec::with_capacity(len / 4);
    for _ in 0..len / 4 {
        parts.push(format!("{:08X}", cur.u32()?));
    }
    Ok(parts.join("-"))
}

/// Decode the device's `key=value` configuration file (CP1251 encoded).
pub fn decode_configuration(data: &[u8]) -> String {
    let (text, _, _) = encoding_rs::WINDOWS_1251.decode(data);
    text.into_owned()
}

pub fn parse_configuration(text: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn base() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
    }

    #[test]
    fn decodes_real_time_record() {
        let mut buf = vec![1, 0, 0];
        buf.extend_from_slice(&100i32.to_le_bytes()); // 1.000 s offset
        buf.extend_from_slice(&12.5f32.to_le_bytes());
        buf.extend_from_slice(&58.0f32.to_le_bytes()); // 58 µR/h
        buf.extend_from_slice(&25u16.to_le_bytes()); // 2.5 %
        buf.extend_from_slice(&40u16.to_le_bytes()); // 4.0 %
        buf.extend_from_slice(&3u16.to_le_bytes());
        buf.push(0);

        let records = decode_data_buf(&buf, base());
        assert_eq!(records.len(), 1);
        match &records[0] {
            Record::RealTime(r) => {
                assert!((r.count_rate - 12.5).abs() < 1e-6);
                assert!((r.dose_rate - 58.0).abs() < 1e-6);
                assert!((r.count_rate_err - 2.5).abs() < 1e-6);
                assert_eq!(r.flags, 3);
                assert_eq!(r.dt, base() + Duration::seconds(1));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn decodes_rare_record() {
        let mut buf = vec![7, 0, 3];
        buf.extend_from_slice(&0i32.to_le_bytes());
        buf.extend_from_slice(&3600u32.to_le_bytes());
        buf.extend_from_slice(&1234u32.to_le_bytes());
        buf.extend_from_slice(&4450u16.to_le_bytes()); // 24.50 C
        buf.extend_from_slice(&8700u16.to_le_bytes()); // 87 %
        buf.extend_from_slice(&0u16.to_le_bytes());

        let records = decode_data_buf(&buf, base());
        match &records[0] {
            Record::Rare(r) => {
                assert!((r.temperature_c - 24.5).abs() < 1e-6);
                assert!((r.charge_percent - 87.0).abs() < 1e-6);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn stops_at_sequence_gap() {
        let mut buf = Vec::new();
        for seq in [0u8, 2] {
            buf.extend_from_slice(&[seq, 0, 0]);
            buf.extend_from_slice(&0i32.to_le_bytes());
            buf.extend_from_slice(&1.0f32.to_le_bytes());
            buf.extend_from_slice(&1.0f32.to_le_bytes());
            buf.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        }
        let records = decode_data_buf(&buf, base());
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn decodes_v1_spectrum() {
        // 6 values: 3 literal, one delta, then 2 zeros.
        let mut buf = Vec::new();
        buf.extend_from_slice(&10u32.to_le_bytes()); // duration
        buf.extend_from_slice(&0.5f32.to_le_bytes());
        buf.extend_from_slice(&2.0f32.to_le_bytes());
        buf.extend_from_slice(&0.0f32.to_le_bytes());
        buf.extend_from_slice(&((2u16 << 4) | 1).to_le_bytes()); // 2 values, 1 byte each
        buf.extend_from_slice(&[5, 9]);
        buf.extend_from_slice(&((1u16 << 4) | 2).to_le_bytes()); // 1 value, i8 delta
        buf.push(3u8); // last + 3 = 12
        buf.extend_from_slice(&(2u16 << 4).to_le_bytes()); // 2 zeros

        let s = decode_spectrum(&buf, 1).unwrap();
        assert_eq!(s.counts, vec![5, 9, 12, 0, 0]);
        assert_eq!(s.duration_s, 10);
        assert!((s.a1 - 2.0).abs() < 1e-6);
    }

    #[test]
    fn parses_configuration() {
        let cfg = "\u{feff}SpecFormatVersion=1\r\nDeviceName=RC-110\r\n\r\n";
        let map = parse_configuration(cfg);
        assert_eq!(map.get("SpecFormatVersion").unwrap(), "1");
        assert_eq!(map.get("DeviceName").unwrap(), "RC-110");
    }
}
