# SP548E BLE protocol — research for production order #1

**This is research, not a driver.** It is the sourced reference material the
familiar's factory is handed for order #1 (manufacture an SP548E driver). The
factory writes its *own* code and proves it against the oracle; this document
only informs. A claim without a source is a TODO.

## Sources

1. **Household record** — `~/Projects/CLAUDE.md`, "river.io Network →
   SP548E LED controller" section. Reverse-engineered and verified live on the
   physical device on firmware `V3.0.10`, 2026-07-28. This is the primary
   source; every framing claim below is from it.
2. **Reference implementation (informational only)** — `banlanx_6xx.py` in
   [monty68/uniled](https://github.com/monty68/uniled). The household record
   notes this was byte-accurate for the BanlanX 6xx protocol at `key=0x00`;
   model `0x94` (SP548E) falls outside uniled's documented `0x1F`–`0x34`
   range, which is why no public repo lists it. **Reading this as reference is
   research; copying it in as the driver is not** — the oracle decides, the
   reference only informs.
3. **Live re-verification, 2026-08-28** — a passive BLE scan from MacOnStick
   saw the device: `name='motorlight' rssi=-56 model=0x94 ver=0xf0
   wifi_mac=ba:16:b5:fe:19:82`. Note `ver=0xf0` here vs `0x10` in the July
   record — a possible firmware change since July; the factory's read-oracle
   rung should treat the version byte as observed-not-assumed.

## Identity / discovery

- BLE advertises as name `SP548E` (July) / `motorlight` (Aug), service `ffe1`,
  **manufacturer ID `0x5053`** ("SP"), payload `[model 0x94][ver][WiFi MAC ×6]`.
- **Match rule for the broker:** manufacturer `0x5053` **and** WiFi MAC
  `ba:16:b5:fe:19:82`. Never the CoreBluetooth peripheral UUID — that is
  per-host (`507DF5FA-…` on MacOnStick) and not portable.
- GATT: service `ffe0`, characteristic `ffe1` (write + notify). Use
  **acknowledged** writes.

## Framing

```
53 | type | key | total_frags | frag_idx | payload_len | payload
```

`key = 0x00` (unencrypted). Commands (`type`):

| type | meaning | payload |
|---|---|---|
| `0x02` | state query | — (replies in 18 fragments / 245 bytes) |
| `0x50` | power | on/off |
| `0x51` | brightness | `[which, level]` |
| `0x52` | static RGB | `[r, g, b, level]` |
| `0x53` | mode | `[mode, effect]` |
| `0x57` | RGB in dynamic mode | — |

## State decode (from the `0x02` reply)

- offset `[30]` = light mode
- offset `[33]` = brightness
- **Colour is never echoed back** — only mode and brightness are readable.
  This is the reason order #1's oracle has a **witness rung**: a colour claim
  can only be confirmed by human eyes. The bench and read rungs can prove
  framing, reassembly, mode, and brightness; they cannot prove colour.

## Known hazards the factory must respect

- The strip renders true RGB (`red = ff0000`) despite WS2811/WS2812B being
  natively GRB. A controller reconfiguration could silently invert red/green
  while every command still reports success — so the witness rung is not
  optional for colour.
- Bluetooth TCC: a launchd daemon's child is denied CoreBluetooth on first
  headless run (the 2026-08-08 wildhorse incident; the 60s tool budget in
  `cycle` exists because of it). The broker owns the radio and must acquire
  the grant through a human-triggered prompt before the daemon path works.
- The colour/flash/dynamic-mode commands (`0x52`/`0x53`/`0x57`) move real
  light. Order #1's capability surface is `state, on, off, brightness, color`;
  the act rung transmits only what the order names, one operation at a time.
