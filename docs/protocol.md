# RadiaCode protocol notes

Everything below was derived from the public MIT-licensed
[`cdump/radiacode`](https://github.com/cdump/radiacode) implementation and validated
against a RadiaCode 110. It is a working reference, not an official specification.

## Transports

**USB** — vendor interface, *not* CDC serial:

| | |
| --- | --- |
| VID:PID | `0483:f123` (STMicroelectronics) |
| Bulk OUT | `0x01` |
| Bulk IN | `0x81` |
| Framing | `u32` little-endian payload length, then payload |

**Bluetooth LE** — vendor GATT service:

| Characteristic | UUID |
| --- | --- |
| Write | `e63215e6-7003-49d8-96b0-b024798fb901` |
| Notify | `e63215e7-7003-49d8-96b0-b024798fb901` |

Requests are written in 18-byte chunks (default ATT MTU 23). Replies arrive as one
or more notifications; the first four bytes are the reply length.

## Frame

```
[u32 payload_len][u16 command][u8 0][u8 0x80 | seq][args...]
```

`seq` cycles 0–31; the device echoes the four header bytes. A reply looks like
`[u32 len][echoed header][body]`.

## Initialization

1. `SET_EXCHANGE (0x0007)` with `01 ff 12 ff`
2. `SET_TIME (0x0A04)` with `[day, month, year-2000, 0, sec, min, hour, 0]`
3. Write VSFR `DEVICE_TIME (0x0504)` = `0`
4. Base time for buffered records = host now + 128 s
5. Read `GET_VERSION`; firmware ≥ 4.8 required
6. Read the configuration string (CP1251); `SpecFormatVersion` selects the
   spectrum decoder

## Commands

| Command | Id | Notes |
| --- | --- | --- |
| GET_STATUS | `0x0005` | `u32` flags |
| SET_EXCHANGE | `0x0007` | handshake |
| GET_VERSION | `0x000A` | boot + target versions, length-prefixed strings |
| GET_SERIAL | `0x000B` | `u32` byte length, then `u32` groups |
| FW_SIGNATURE | `0x0101` | signature, filename, id |
| RD_VIRT_STRING | `0x0826` | read block (`VS`) |
| WR_VIRT_STRING | `0x0827` | e.g. spectrum reset |
| RD_VIRT_SFR | `0x0824` | read one register |
| WR_VIRT_SFR | `0x0825` | write one register |
| RD_VIRT_SFR_BATCH | `0x082A` | `[n][ids...]` → `[valid_mask][values...]` |
| SET_TIME | `0x0A04` | clock |

### Virtual strings

| Id | Content |
| --- | --- |
| `0x0002` | configuration (CP1251) |
| `0x0008` | serial number |
| `0x0100` | data buffer (mixed records) |
| `0x0200` | live spectrum |
| `0x0205` | accumulated spectrum |
| `0x0202` | energy calibration, three `f32` |

### Registers used by Radiadash

| Register | Address | Type |
| --- | --- | --- |
| `DS_UNITS` | `0x8004` | bool: Sv vs R |
| `USE_NSV_H` | `0x800C` | bool |
| `CR_UNITS` | `0x8013` | bool: cpm vs cps |
| `CPS` | `0x8020` | u32 |
| `DR_UR_H` | `0x8021` | u32, µR/h |
| `DS_UR` | `0x8022` | u32, µR |
| `TEMP_DEGC` | `0x8024` | f32 |
| `CHN_TO_KEV_A0..A2` | `0x8010..0x8012` | f32 |
| `DISP_BRT` | `0x0511` | u8 |
| `DISP_OFF_TIME` | `0x0513` | encoded 5/10/15/30 s |
| `SOUND_ON` / `VIBRO_ON` | `0x0522` / `0x0531` | bool |
| `DOSE_RESET` | `0x8007` | write-only |

## Data buffer records

Each record starts with `seq u8`, `event_id u8`, `group_id u8`, `timestamp i32`
(10 ms units, relative to the base time). Known groups:

| `(event, group)` | Payload |
| --- | --- |
| `(0,0)` RealTimeData | `f32 cps`, `f32 dose_rate`, `u16 cps_err/10`, `u16 dose_err/10`, `u16 flags`, `u8 rt_flags` |
| `(0,1)` RawData | `f32 cps`, `f32 dose_rate` |
| `(0,2)` DoseRateDB | `u32 count`, `f32 cps`, `f32 dose_rate`, `u16 err/10`, `u16 flags` |
| `(0,3)` RareData | `u32 duration`, `u32 dose`, `u16 temp (0.01 °C + 2000)`, `u16 charge (0.01 %)`, `u16 flags` |
| `(0,7)` Event | `u8 event`, `u8 param`, `u16 flags` |

A sequence jump means records were dropped; decoding stops at the gap.

## Spectrum

Payload: `u32 live_time_s`, `f32 a0`, `f32 a1`, `f32 a2`, then counts.

- **Format 0**: plain `u32` counts.
- **Format 1**: `u16` words; high 12 bits = run length, low 4 bits = value encoding
  (0 = zeros, 1 = u8, 2 = i8 delta, 3 = i16 delta, 4 = 24-bit delta, 5 = i32 delta).

Energy: `keV = a0 + a1*ch + a2*ch²`.

## Units caveat

The protocol does not carry an explicit unit for measurement values. What we have
observed on a RadiaCode 110 (firmware 4.14, device configured for roentgen):

- `RealTimeData.dose_rate` is **R/h** (`4.1e-6` ≈ 0.041 µSv/h).
- `RareData.dose` is a **little-endian f32 in R**, not an integer count.
- The classic `CPS` / `DR_uR_h` / `DS_uR` registers are absent; live values come
  from the data buffer.

Radiadash stores canonical units (µSv, µSv/h) and converts only at the display
boundary. Older RC-10x firmware uses different scaling (`raw/100` for Sv mode);
always confirm against the device's own display and record the result in
[devices.md](devices.md).

## Firmware

Firmware ≥ 4.8 is required (the reference library refuses older versions). The
device reports `SpecFormatVersion` in its configuration; Radiadash supports 0 and 1.
