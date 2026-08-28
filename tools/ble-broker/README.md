# The trusted BLE broker (T-229 brick 4)

The broker is the only part of the familiar's factory that touches the radio.
A jailed candidate — the familiar's generated driver code — has no Bluetooth
authority at all (the containment jail grants no `mach-lookup`, so
CoreBluetooth is unreachable inside it). The trusted manager spawns this broker
**outside** the jail, connects it to the candidate by a pipe, and the candidate
observes or transmits only through a narrow, typed protocol.

## What the candidate cannot choose (codex's ruling, 2026-08-28)

- **The device** — the broker resolves the human-declared match rule
  (manufacturer id + exact Wi-Fi MAC) itself and refuses zero or multiple
  matches. No enumeration, no selection.
- **The UUIDs** — service and characteristic are fixed by config; the protocol
  has no UUID field.
- **The operation** — a session is opened for one rung (`read` or a single
  named `act` op). A transmit on a read session is refused; a transmit naming a
  different op is refused.
- **The bounds** — frame size, write count, write rate, response size, and
  session lifetime are all capped and enforced.

The candidate's product is the *frame bytes* (SP548E framing/reassembly/decode).
The broker transmits them to the one fixed characteristic under the caps. It is
generic factory machinery, not a driver.

## Protocol

Newline-delimited JSON on a pipe. The broker emits `{"resp":"open","address":…}`
once connected, then answers one line per request:

- `{"req":"read"}` → `{"resp":"read","data":"<hex>"}` (bounded)
- `{"req":"transmit","op":"<name>","frame":"<hex>"}` → `{"resp":"transmit","ok":true,"writes":N}`

Refusals come back as `{"error":"<code>","detail":"…"}` (codes: `no_match`,
`multiple_match`, `read_only_session`, `wrong_op`, `frame_too_large`,
`write_budget`, `rate_limited`, `session_expired`, `not_open`, `bad_frame`).

## Testing without a radio

The BLE layer is behind the `Backend` protocol, so the whole broker and every
cap is testable with a mock (`test_broker.py`, 14 hostile fixtures) — no radio,
no TCC grant. Run:

```
python3 tools/ble-broker/test_broker.py
```

The real `BleakBackend` is imported lazily and used only in a live session,
which begins only after the containment jail is accepted and the human's
TCC/gate/witness requirements are met.

## Live invocation (only when authorized)

```
python3 tools/ble-broker/broker.py \
  --mfr 0x5053 --mac ba:16:b5:fe:19:82 --service ffe0 --char ffe1 \
  --rung read
```

Requires `bleak` and a Bluetooth TCC grant for the broker process. The trusted
manager (a later brick) supplies the config from the work order, rechecks the
boundary gate before opening the session, and binds the candidate, broker,
gate-snapshot, and request/response evidence digests into the workshop ledger.
