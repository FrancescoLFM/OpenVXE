# VXE R1 SE+ HID protocol

The VXE R1 SE+ uses the Compx / ATK vendor protocol shared by many VXE, ATK, VGN and
MCHOSE mice. This document collects what is known, **where it comes from and how sure we
are**. Please keep the confidence column honest when you update it.

Confidence levels:

- ✅ **vendor**: read in the official ATK HUB web driver bundle (WebHID JavaScript)
- ✅ **hw**: verified on real hardware by a public project (source given)
- 🟡 **consistent**: inferred, and consistent with the packets OpenVXE already sends
- ❓ **unverified**: single source or guess — needs a capture / `vxe-probe` check

## Device identification

| VID:PID | Device | Confidence |
| --- | --- | --- |
| `373b:1085` | "Wireless mouse 1k dongle" — generic receiver, vendor usage page `0xFF04`, usage 2 | ✅ vendor |
| `3554:f58f` | R1 SE / SE+ wired | ❓ (OpenMouse) |
| `3554:f58e`, `3554:fb12` | VXE Mouse 1K Dongle | ✅ vendor (`fb12`), ❓ (`f58e`) |
| `373b:12dc`, `373b:1319` | R1 SE+ CORE26K (NearLink) | ❓ (OpenMouse, from HUB) |

Since `1085` is a generic receiver, the actual mouse model must be identified at runtime with
`GetMouseCIDMID` (R1 SE+ reported as CID/MID 2,32; HUB also lists 2,31 and 2,116 — ❓).
Run `vxe-probe --list` to see the interfaces and usage pages of your device.

## Frame

Output report ID `0x08`, 16-byte payload:

| Byte | Meaning |
| --- | --- |
| `[0]` | command |
| `[1]` | status in replies (0 = ok, non-zero = rejected / unsupported) |
| `[2..3]` | EEPROM address, 16-bit big-endian |
| `[4]` | data length (max 10) |
| `[5..14]` | data |
| `[15]` | checksum: `0x55 - (0x08 + sum([0..14]))`, i.e. the payload sums to `0x4D` |

Replies arrive as input reports with the same report ID and echo command and address.
Confidence: ✅ vendor; checksum verified against a published raw frame (unit test
`eeprom_read_matches_published_frame`).

## Commands

| Code | Name | Notes |
| --- | --- | --- |
| `0x01` | DownLoadData | |
| `0x02` | DownLoadDriverStatus | |
| `0x03` | GetWirelessMouseOnline | |
| `0x04` | **GetBatteryLevel** | see below |
| `0x05` / `0x06` | SetWirelessDonglePair / GetWirelessDonglePairResult | |
| `0x07` | **SetEEPROM** | write configuration memory |
| `0x08` | **GetEEPROM** | read configuration memory (some firmware replies with `0x07`) |
| `0x09` | RestoreFactory | ⚠️ |
| `0x0a` | ReportMouseStatus | |
| `0x0d` | EnterUSBUpgradeMode | ⚠️ puts the mouse in bootloader mode |
| `0x0e` / `0x0f` | GetCurrentConfig / SetCurrentConfig | profile bank 0–3 |
| `0x10` | **GetMouseCIDMID** | CID at data `[0]`, MID at data `[1]` |
| `0x12` | **GetMouseVersion** | BCD, e.g. `03 15` = 3.15 |
| `0x13` | DongleExitPair | |
| `0x14` / `0x15` | Set / Get4KRGBMode | |
| `0x16` / `0x17` | Set / **GetFarDistanceMode** | long range mode flag |
| `0x18` / `0x19` | Set / GetDongleLightMode | |
| `0x1d` | GetDongleVersion | |

Confidence: ✅ vendor (enum read in the HUB bundle).

> ⚠️ **OpenVXE currently uses `0x17` as battery query.** According to the vendor code
> `0x17` is GetFarDistanceMode, and the battery is `0x04`. To verify with `vxe-probe --info`.

### Battery (`0x04`)

Request `04 00 … 00`. Reply data (payload offsets): `[5]` percent 0–100 (no ×2),
`[6]` charging (0 = no, 1 = yes), `[7..8]` voltage in mV, big-endian.
Example reply payload `04 00 00 00 02 5F 00 10 00 …` = 95 %, not charging, 4096 mV.
Confidence: ✅ hw on `373b:1085` (vxe-battery-tray), ❓ charging flag mapping.

## EEPROM map

Most single-byte settings are stored as **value / complement pairs** (`v`, `0x55 - v`);
records of 4 bytes end with `0x55 - (b0 + b1 + b2)`.

| Address | Field | Encoding | Confidence |
| --- | --- | --- | --- |
| `0x00` | report rate | pair: 1 = 1000, 2 = 500, 4 = 250, 8 = 125 Hz | ✅ vendor |
| `0x02` | DPI stage count | pair | ✅ vendor |
| `0x04` | active DPI stage (0-based) | pair | ✅ vendor |
| `0x0a` | lift-off distance | pair: 1 = 1 mm, 2 = 2 mm | ✅ vendor |
| `0x0c`–`0x2b` | DPI stages 1–8 | records `[x, y, ex, check]` | ✅ vendor, 🟡 consistent |
| `0x2c`–`0x4b` | DPI stage colours | | ❓ |
| `0x4c` | DPI LED settings | | ❓ |
| `0x60`–`0x9f` | button / key slots | records `[class, v1, v2, check]`; `0x60` left, `0x64` right, `0x68` middle, `0x6c` back, `0x70` forward, `0x74` bottom | ✅ vendor |
| `0xa9` | debounce (ms) | pair | ✅ vendor, 🟡 consistent |
| `0xab` | motion sync | pair | ✅ vendor, 🟡 consistent |
| `0xad` | sleep time | pair, **units of 10 s** | ✅ vendor (`raw * 10`) |
| `0xaf` | straight-line correction (angle snapping?) | pair | ❓ |
| `0xb1` | ripple control | pair | ❓ |
| `0x100`– | shortcuts, 32-byte slots | | ❓ |
| `0x300`– | macros, 384-byte slots | | ❓ |

Button classes: 0 disabled, 1 mouse (v1: 1 L, 2 R, 4 M, 8 back, 0x10 forward), 2 DPI
(1 cycle, 2 up, 3 down), 3 h-scroll, 4 fire, 5 shortcut, 6 macro, 7 report rate, 8 light,
9 profile, 0x0a DPI lock, 0x0b wheel. Confidence: ❓ (libatk-rs, MouseMod).

### DPI encoding

Each stage is `[x, y, ex, 0x55 - (x + y + ex)]`. `ex` holds extra bits: low nibble for X,
high nibble for Y. The meaning of `x`/`y` **depends on the sensor**:

- **PAW3395SE** variant: `x`/`y` are an index into a code table (not `dpi / 50`), e.g.
  800 → `0x12`, 1600 → `0x25`, 3200 → `0x4b`; above 10,000 DPI half the value is written
  and an `ex` flag is set. ❓ (OpenMouse, hw on the wired `f58f`).
- **CORE26K** variant (up to 26,000 DPI): count = `dpi / 50 - 1`, low byte in `x`/`y`,
  bits 8–9 in `ex` bits 2–3 (Y: 6–7), i.e. `ex = high * 0x44` for X = Y. ❓ (OpenMouse,
  Linux-ATK).

OpenVXE currently writes `dpi / 50` with `ex = 0`, which matches neither exactly. The
right encoding must be chosen from `GetMouseCIDMID` and confirmed with a capture.

## How OpenVXE's current packets map onto this

Re-reading the packets the TUI sends with the map above (`vxe-decode` does this
automatically):

- **DPI** `07 00 00 0c 08 | 04 04 00 4d | X Y 00 C` writes **two stages**: stage 1 fixed
  to code 4 and stage 2 = the chosen value. 🟡 consistent (both records have valid checks).
- **Motion sync** `07 00 00 a9 0a | 08 4d 01 54 05 50 00 00 00 00` writes 10 bytes from
  `0xa9`: **debounce 8 ms, motion sync, sleep 50 s**, then straight-line correction and
  ripple control with *invalid* complements (`00 00`). Toggling motion sync therefore also
  resets debounce and sleep. 🟡 consistent.
- **Polling rate, LOD, debounce, angle snapping** are written as 4 bytes at `0x70`
  (`[register, value, 0, 0x55 - value]`). In the vendor map `0x70` is the **forward
  button** record, and these writes don't have a valid record checksum. No source other
  than OpenVXE describes this scheme: it may be a real "live settings" register on this
  dongle, or it may be corrupting the forward button. ❓ — **check with
  `vxe-probe --dump --from 60 --to 78` before and after changing those settings.**

## Sources

- ATK HUB official web driver, bundle v3.2.27 — <https://hub.atk.pro/>
- OpenMouse-Project/mouse-protocol — <https://github.com/OpenMouse-Project/mouse-protocol>
  (`src/atk/index.ts`, `docs/atk-testing.md`)
- cyberphantom52/libatk-rs — <https://github.com/cyberphantom52/libatk-rs> (`src/types.rs`);
  VoideUI/Linux-ATK — <https://github.com/VoideUI/Linux-ATK>
- LCournollet/MouseMod — <https://github.com/LCournollet/MouseMod/blob/main/PROTOCOL.md>
- ivancacic/vxe-battery-tray — <https://github.com/ivancacic/vxe-battery-tray/blob/main/PROTOCOL.md>
- SpookyyQ ATK Fierce X notes — <https://github.com/SpookyyQ/IPI-Stay-Fly-Driver/blob/main/docs/atk-fierce-x-protocol-notes.md>
- TnZzZHlp/mouse (ATK X1 Pro) — <https://github.com/TnZzZHlp/mouse>
