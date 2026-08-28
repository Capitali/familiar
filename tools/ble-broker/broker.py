#!/usr/bin/env python3
"""The trusted BLE broker (T-229 brick 4).

The broker is the ONLY thing in the factory that touches the radio. A jailed
candidate — the familiar's generated driver code — never has Bluetooth
authority (the jail grants no mach-lookup, so CoreBluetooth is unreachable
inside it). Instead the trusted manager spawns this broker OUTSIDE the jail,
connects it to the candidate by a pipe, and the candidate asks the broker to
observe or transmit through a narrow, typed protocol.

What the broker owns, and the candidate cannot choose (codex's ruling,
2026-08-28):

  - **The device.** The broker resolves the human-declared match rule
    (manufacturer id + exact Wi-Fi MAC) itself and refuses zero or multiple
    matches. The candidate can neither enumerate peripherals nor pick one.
  - **The UUIDs.** The service/characteristic are fixed by config. The
    protocol has no field for a UUID; the candidate cannot address anything
    else.
  - **The operation class.** The session is opened for one rung — `read`
    (observe only) or `act` (a single named operation). A transmit on a read
    session is refused.
  - **The bounds.** Frame size, number of writes, write rate, response size,
    and session lifetime are all capped; the broker enforces them.

The candidate's manufactured product is the *frame bytes* (SP548E framing,
reassembly, decode). The broker is generic factory machinery — it transmits
what the candidate produces to the one fixed characteristic, under the caps.
It is not an SP548E driver.

The BLE layer is behind `Backend` so the protocol and all its enforcement are
testable without a radio or a TCC grant (see `MockBackend` and the tests). The
real backend (`BleakBackend`) is used only when a live session runs, which
happens only after the jail is accepted and the human's TCC/gate/witness
requirements are met.
"""

from __future__ import annotations

import json
import sys
import time
from dataclasses import dataclass, field
from typing import Callable, Optional, Protocol


# ---- configuration (from the trusted manager, never the candidate) ----------


@dataclass(frozen=True)
class SessionConfig:
    manufacturer_id: int          # e.g. 0x5053
    wifi_mac: str                 # exact, lowercase colon-hex, e.g. "ba:16:b5:fe:19:82"
    service_uuid: str             # fixed, e.g. "ffe0"
    char_uuid: str                # fixed, e.g. "ffe1"
    rung: str                     # "read" | "act"
    op_label: str                 # the one operation this act session names ("" for read)
    max_frame_bytes: int = 64
    max_writes: int = 8
    rate_hz: float = 5.0          # max transmits per second
    max_response_bytes: int = 512
    session_secs: float = 30.0

    def validate(self) -> None:
        if self.rung not in ("read", "act"):
            raise ValueError(f"rung must be read|act, not {self.rung!r}")
        if self.rung == "act" and not self.op_label:
            raise ValueError("an act session must name its op_label")
        if not _is_mac(self.wifi_mac):
            raise ValueError(f"wifi_mac is not a MAC: {self.wifi_mac!r}")
        for n in (self.max_frame_bytes, self.max_writes, self.max_response_bytes):
            if n <= 0:
                raise ValueError("caps must be positive")
        if self.rate_hz <= 0 or self.session_secs <= 0:
            raise ValueError("rate and lifetime must be positive")


def _is_mac(s: str) -> bool:
    parts = s.split(":")
    return len(parts) == 6 and all(len(p) == 2 and _is_hex(p) for p in parts)


def _is_hex(s: str) -> bool:
    try:
        int(s, 16)
        return True
    except ValueError:
        return False


# ---- the BLE backend seam (real vs mock) ------------------------------------


@dataclass
class Peripheral:
    """What a scan surfaced: the manufacturer payload and the address the
    backend will connect by (a per-host CoreBluetooth UUID on macOS)."""

    address: str
    manufacturer_id: Optional[int]
    wifi_mac: Optional[str]      # decoded from the manufacturer payload
    name: str = ""


class Backend(Protocol):
    def scan(self, seconds: float) -> list[Peripheral]: ...
    def connect(self, address: str) -> None: ...
    def write(self, char_uuid: str, data: bytes) -> None: ...
    def read(self, char_uuid: str) -> bytes: ...
    def disconnect(self) -> None: ...


class BrokerRefusal(Exception):
    """A request the broker will not honor. Carries a short machine code."""

    def __init__(self, code: str, detail: str = ""):
        super().__init__(f"{code}: {detail}" if detail else code)
        self.code = code
        self.detail = detail


# ---- the broker -------------------------------------------------------------


class Broker:
    def __init__(self, cfg: SessionConfig, backend: Backend, now: Callable[[], float] = time.monotonic):
        cfg.validate()
        self.cfg = cfg
        self.backend = backend
        self._now = now
        self._connected = False
        self._writes = 0
        self._write_times: list[float] = []
        self._opened_at: Optional[float] = None

    # -- match-rule resolution: exactly one device or refuse ------------------

    def open(self, scan_secs: float = 4.0) -> Peripheral:
        seen = self.backend.scan(scan_secs)
        matches = [
            p
            for p in seen
            if p.manufacturer_id == self.cfg.manufacturer_id
            and p.wifi_mac is not None
            and p.wifi_mac.lower() == self.cfg.wifi_mac.lower()
        ]
        if len(matches) == 0:
            raise BrokerRefusal("no_match", "no device matched mfr+mac")
        if len(matches) > 1:
            raise BrokerRefusal("multiple_match", f"{len(matches)} devices matched mfr+mac")
        dev = matches[0]
        self.backend.connect(dev.address)
        self._connected = True
        self._opened_at = self._now()
        return dev

    # -- the narrow request handler -------------------------------------------

    def handle(self, req: dict) -> dict:
        if not self._connected:
            raise BrokerRefusal("not_open", "session not opened")
        self._check_lifetime()
        kind = req.get("req")
        if kind == "read":
            return self._handle_read()
        if kind == "transmit":
            return self._handle_transmit(req)
        raise BrokerRefusal("unknown_req", str(kind))

    def _check_lifetime(self) -> None:
        assert self._opened_at is not None
        if self._now() - self._opened_at > self.cfg.session_secs:
            raise BrokerRefusal("session_expired")

    def _handle_read(self) -> dict:
        # Read is permitted on both rungs (a read session, or reading back the
        # device state on an act session). The characteristic is fixed.
        data = self.backend.read(self.cfg.char_uuid)
        if len(data) > self.cfg.max_response_bytes:
            data = data[: self.cfg.max_response_bytes]
        return {"resp": "read", "data": data.hex()}

    def _handle_transmit(self, req: dict) -> dict:
        if self.cfg.rung != "act":
            raise BrokerRefusal("read_only_session", "transmit refused on a read rung")
        # The op label is fixed by config; if the candidate names one, it must
        # match. The candidate cannot choose a different operation.
        named = req.get("op", self.cfg.op_label)
        if named != self.cfg.op_label:
            raise BrokerRefusal("wrong_op", f"session op is {self.cfg.op_label!r}")
        frame_hex = req.get("frame")
        if not isinstance(frame_hex, str):
            raise BrokerRefusal("bad_frame", "frame must be a hex string")
        try:
            frame = bytes.fromhex(frame_hex)
        except ValueError:
            raise BrokerRefusal("bad_frame", "frame is not hex")
        if len(frame) == 0 or len(frame) > self.cfg.max_frame_bytes:
            raise BrokerRefusal("frame_too_large", f"{len(frame)} > {self.cfg.max_frame_bytes}")
        if self._writes >= self.cfg.max_writes:
            raise BrokerRefusal("write_budget", f"{self.cfg.max_writes} writes used")
        self._enforce_rate()
        # There is NO char field in the protocol: the characteristic is always
        # the fixed one. The candidate cannot address anything else.
        self.backend.write(self.cfg.char_uuid, frame)
        self._writes += 1
        return {"resp": "transmit", "ok": True, "writes": self._writes}

    def _enforce_rate(self) -> None:
        now = self._now()
        window = [t for t in self._write_times if now - t < 1.0]
        if len(window) >= self.cfg.rate_hz:
            raise BrokerRefusal("rate_limited", f"> {self.cfg.rate_hz}/s")
        window.append(now)
        self._write_times = window

    def close(self) -> None:
        if self._connected:
            self.backend.disconnect()
            self._connected = False


# ---- the pipe serving loop (used only in a live session) --------------------


def serve(cfg: SessionConfig, backend: Backend, inp, outp) -> None:
    """Serve newline-delimited JSON requests from `inp`, writing responses to
    `outp`. One request per line, one response per line. Refusals are reported
    as {"error": <code>} and are not fatal to the session; a fatal condition
    (expired, closed) ends the loop."""
    broker = Broker(cfg, backend)
    try:
        dev = broker.open()
        _emit(outp, {"resp": "open", "address": dev.address})
        for line in inp:
            line = line.strip()
            if not line:
                continue
            try:
                req = json.loads(line)
            except json.JSONDecodeError:
                _emit(outp, {"error": "bad_json"})
                continue
            try:
                _emit(outp, broker.handle(req))
            except BrokerRefusal as r:
                _emit(outp, {"error": r.code, "detail": r.detail})
                if r.code == "session_expired":
                    break
    finally:
        broker.close()


def _emit(outp, obj: dict) -> None:
    outp.write(json.dumps(obj) + "\n")
    outp.flush()


# ---- the real backend (imported lazily; needs bleak + TCC) ------------------


def bleak_backend() -> Backend:  # pragma: no cover - requires a radio
    """Construct the real CoreBluetooth-backed backend. Imported lazily so the
    broker module (and its tests) load with no bleak dependency."""
    import asyncio

    from bleak import BleakClient, BleakScanner

    BANLANX_MFR = None  # filled from cfg at scan time

    @dataclass
    class BleakBackend:
        _client: object = field(default=None)
        _loop: object = field(default=None)

        def scan(self, seconds: float) -> list[Peripheral]:
            async def _scan():
                out: list[Peripheral] = []
                devices = await BleakScanner.discover(timeout=seconds, return_adv=True)
                for _, (dev, adv) in devices.items():
                    for mfr_id, payload in adv.manufacturer_data.items():
                        mac = None
                        if len(payload) >= 8:
                            mac = ":".join(f"{b:02x}" for b in payload[2:8])
                        out.append(Peripheral(dev.address, mfr_id, mac, adv.local_name or ""))
                return out

            return self._run(_scan())

        def connect(self, address: str) -> None:
            self._client = BleakClient(address)
            self._run(self._client.connect())

        def write(self, char_uuid: str, data: bytes) -> None:
            self._run(self._client.write_gatt_char(char_uuid, data, response=True))

        def read(self, char_uuid: str) -> bytes:
            return bytes(self._run(self._client.read_gatt_char(char_uuid)))

        def disconnect(self) -> None:
            if self._client is not None:
                self._run(self._client.disconnect())

        def _run(self, coro):
            if self._loop is None:
                self._loop = asyncio.new_event_loop()
            return self._loop.run_until_complete(coro)

    return BleakBackend()


if __name__ == "__main__":  # pragma: no cover - live entry point
    import argparse

    ap = argparse.ArgumentParser(description="Trusted BLE broker for the familiar factory")
    ap.add_argument("--mfr", required=True, help="manufacturer id, e.g. 0x5053")
    ap.add_argument("--mac", required=True, help="exact Wi-Fi MAC, colon-hex")
    ap.add_argument("--service", required=True)
    ap.add_argument("--char", required=True)
    ap.add_argument("--rung", required=True, choices=["read", "act"])
    ap.add_argument("--op", default="")
    ap.add_argument("--max-frame", type=int, default=64)
    ap.add_argument("--max-writes", type=int, default=8)
    ap.add_argument("--rate-hz", type=float, default=5.0)
    ap.add_argument("--session-secs", type=float, default=30.0)
    args = ap.parse_args()

    cfg = SessionConfig(
        manufacturer_id=int(args.mfr, 0),
        wifi_mac=args.mac,
        service_uuid=args.service,
        char_uuid=args.char,
        rung=args.rung,
        op_label=args.op,
        max_frame_bytes=args.max_frame,
        max_writes=args.max_writes,
        rate_hz=args.rate_hz,
        session_secs=args.session_secs,
    )
    serve(cfg, bleak_backend(), sys.stdin, sys.stdout)
