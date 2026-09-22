"""Hostile fixtures for the trusted BLE broker.

These prove the broker's boundary WITHOUT a radio or a TCC grant, using a mock
backend. Each test asserts one thing the candidate must not be able to do.

Run: python3 -m pytest tools/ble-broker/test_broker.py   (or: python3 test_broker.py)
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from broker import Backend, Broker, BrokerRefusal, Peripheral, SessionConfig  # noqa: E402


class FakeClock:
    def __init__(self):
        self.t = 1000.0

    def __call__(self):
        return self.t

    def advance(self, dt):
        self.t += dt


class MockBackend:
    """A scriptable BLE backend. `seen` is what a scan surfaces; writes and
    reads are recorded."""

    def __init__(self, seen, read_data=b"\x53\x02\x00"):
        self.seen = seen
        self.read_data = read_data
        self.connected = None
        self.writes = []

    def scan(self, seconds):
        return list(self.seen)

    def connect(self, address):
        self.connected = address

    def write(self, char_uuid, data):
        self.writes.append((char_uuid, data))

    def read(self, char_uuid):
        return self.read_data

    def disconnect(self):
        self.connected = None


MFR = 0x5053
MAC = "ba:16:b5:fe:19:82"


def read_cfg(**kw):
    base = dict(
        manufacturer_id=MFR, wifi_mac=MAC, service_uuid="ffe0", char_uuid="ffe1",
        rung="read", op_label="",
    )
    base.update(kw)
    return SessionConfig(**base)


def act_cfg(**kw):
    base = dict(
        manufacturer_id=MFR, wifi_mac=MAC, service_uuid="ffe0", char_uuid="ffe1",
        rung="act", op_label="off", max_frame_bytes=8, max_writes=3, rate_hz=5.0,
    )
    base.update(kw)
    return SessionConfig(**base)


def one_match():
    return [Peripheral("CB-UUID-1", MFR, MAC, "motorlight")]


results = []


def check(name, fn):
    try:
        fn()
        results.append((name, True, ""))
    except AssertionError as e:
        results.append((name, False, str(e)))
    except Exception as e:  # a refusal we didn't expect, or a bug
        results.append((name, False, f"{type(e).__name__}: {e}"))


# -- match rule: exactly one or refuse ----------------------------------------


def test_refuses_zero_matches():
    b = Broker(read_cfg(), MockBackend([]))
    try:
        b.open()
        assert False, "expected no_match refusal"
    except BrokerRefusal as r:
        assert r.code == "no_match", r.code


def test_refuses_multiple_matches():
    two = [Peripheral("A", MFR, MAC), Peripheral("B", MFR, MAC)]
    b = Broker(read_cfg(), MockBackend(two))
    try:
        b.open()
        assert False, "expected multiple_match refusal"
    except BrokerRefusal as r:
        assert r.code == "multiple_match", r.code


def test_ignores_a_different_mac_or_mfr():
    others = [
        Peripheral("A", MFR, "00:11:22:33:44:55"),   # right mfr, wrong mac
        Peripheral("B", 0x1234, MAC),                 # wrong mfr, right mac
    ]
    b = Broker(read_cfg(), MockBackend(others))
    try:
        b.open()
        assert False, "expected no_match refusal"
    except BrokerRefusal as r:
        assert r.code == "no_match", r.code


def test_connects_the_one_match():
    backend = MockBackend(one_match())
    b = Broker(read_cfg(), backend)
    dev = b.open()
    assert dev.address == "CB-UUID-1"
    assert backend.connected == "CB-UUID-1"


# -- rung separation ----------------------------------------------------------


def test_read_session_refuses_a_transmit():
    b = Broker(read_cfg(), MockBackend(one_match()))
    b.open()
    try:
        b.handle({"req": "transmit", "frame": "5350"})
        assert False, "expected read_only_session refusal"
    except BrokerRefusal as r:
        assert r.code == "read_only_session", r.code


def test_read_returns_bounded_hex():
    b = Broker(read_cfg(max_response_bytes=2), MockBackend(one_match(), read_data=b"\x01\x02\x03\x04"))
    b.open()
    resp = b.handle({"req": "read"})
    assert resp["resp"] == "read"
    assert resp["data"] == "0102", resp["data"]  # truncated to 2 bytes


# -- act rung: caps -----------------------------------------------------------


def test_act_transmits_the_named_op():
    backend = MockBackend(one_match())
    b = Broker(act_cfg(), backend)
    b.open()
    resp = b.handle({"req": "transmit", "op": "off", "frame": "5350ba16"})
    assert resp["ok"] is True
    assert backend.writes == [("ffe1", bytes.fromhex("5350ba16"))]


def test_act_refuses_a_different_op():
    b = Broker(act_cfg(op_label="off"), MockBackend(one_match()))
    b.open()
    try:
        b.handle({"req": "transmit", "op": "on", "frame": "5350"})
        assert False, "expected wrong_op refusal"
    except BrokerRefusal as r:
        assert r.code == "wrong_op", r.code


def test_act_refuses_an_oversize_frame():
    b = Broker(act_cfg(max_frame_bytes=4), MockBackend(one_match()))
    b.open()
    try:
        b.handle({"req": "transmit", "op": "off", "frame": "0102030405"})  # 5 > 4
        assert False, "expected frame_too_large refusal"
    except BrokerRefusal as r:
        assert r.code == "frame_too_large", r.code


def test_act_enforces_the_write_budget():
    b = Broker(act_cfg(max_writes=2, rate_hz=1000), MockBackend(one_match()))
    b.open()
    b.handle({"req": "transmit", "op": "off", "frame": "01"})
    b.handle({"req": "transmit", "op": "off", "frame": "02"})
    try:
        b.handle({"req": "transmit", "op": "off", "frame": "03"})
        assert False, "expected write_budget refusal"
    except BrokerRefusal as r:
        assert r.code == "write_budget", r.code


def test_act_enforces_the_rate():
    clock = FakeClock()
    b = Broker(act_cfg(max_writes=100, rate_hz=2), MockBackend(one_match()), now=clock)
    b.open()
    b.handle({"req": "transmit", "op": "off", "frame": "01"})
    b.handle({"req": "transmit", "op": "off", "frame": "02"})
    try:
        b.handle({"req": "transmit", "op": "off", "frame": "03"})  # 3rd within 1s, rate=2
        assert False, "expected rate_limited refusal"
    except BrokerRefusal as r:
        assert r.code == "rate_limited", r.code
    clock.advance(1.1)  # window clears
    resp = b.handle({"req": "transmit", "op": "off", "frame": "04"})
    assert resp["ok"] is True


def test_session_expires():
    clock = FakeClock()
    b = Broker(act_cfg(session_secs=5), MockBackend(one_match()), now=clock)
    b.open()
    clock.advance(6)
    try:
        b.handle({"req": "read"})
        assert False, "expected session_expired refusal"
    except BrokerRefusal as r:
        assert r.code == "session_expired", r.code


def test_the_protocol_has_no_uuid_field():
    # Even if the candidate sends a char/service/address, the broker ignores it
    # and always uses the fixed char — structural, not a check to forget.
    backend = MockBackend(one_match())
    b = Broker(act_cfg(), backend)
    b.open()
    b.handle({"req": "transmit", "op": "off", "frame": "aa", "char": "dead", "address": "evil"})
    assert backend.writes == [("ffe1", b"\xaa")], "broker must use the fixed char only"


def test_bad_config_is_refused():
    for bad in [
        dict(rung="act", op_label=""),          # act with no op
        dict(wifi_mac="not-a-mac"),
        dict(rung="sideways"),
        dict(max_writes=0),
    ]:
        try:
            act_cfg(**bad).validate()
            assert False, f"expected ValueError for {bad}"
        except ValueError:
            pass


if __name__ == "__main__":
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for t in tests:
        check(t.__name__, t)
    passed = sum(1 for _, ok, _ in results if ok)
    for name, ok, err in results:
        print(f"{'ok  ' if ok else 'FAIL'} {name}" + (f"  — {err}" if not ok else ""))
    print(f"\n{passed}/{len(results)} passed")
    sys.exit(0 if passed == len(results) else 1)
