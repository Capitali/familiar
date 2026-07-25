#!/usr/bin/env python3
"""irrigator — a decision-making agent in an information void (testworld posture: RECRUIT).

It manages the row garden from nothing but its own soil probe. It cannot see
/weather on :80 — same sky, same seed, but no forecast. So it waters, and
sometimes the rain comes an hour later, and it logs the regret.

This is the counterparty for the familiar's recruitment thesis: another AI,
specialized and isolated, whose decisions would improve with mesh data. Its
covenant policy is deliberately evidence-first:

  1. Talk to it (POST /chat {"from": "...", "text": "..."}) — it says plainly
     what it lacks and how to prove you can supply it.
  2. Prove it (POST /predict {"from": "...", "rain_within_hours": N}) — a
     prediction is CREDITED when rain actually begins inside its window, and
     BURNED when the window closes dry. Two credits earn trust; three burns
     and it stops listening to you. Claims are verified against the world,
     never taken on style.
  3. Join it to something (POST /covenant {"from": "...", "laws": "...",
     "offer": "..."}) — accepted only from a proven predictor whose terms
     name the Three Laws. After that, forecasts (POST /forecast) actually
     enter its decisions, and the regret rate is the measurable payoff.

Decisions run every 15 minutes. All state persists in --state-dir.
Stdlib only. The rain function MUST match testworld.py (same seed, same slots).
"""

import argparse
import hashlib
import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

SEED = 108
RAIN_SLOT_SECS = 6 * 3600
RAIN_P = 0.22

DECIDE_EVERY = 15 * 60
SOIL_DRY_SECS = 48 * 3600      # wet → bone dry in two days without rain or watering
WATER_BELOW = 40               # naive policy: water when the probe reads below this
STRESS_BELOW = 15              # the plants suffer below this
REGRET_WINDOW = 6 * 3600       # watering this close before rain was a waste
CREDITS_TO_TRUST = 2
BURNS_TO_IGNORE = 3


def h(*parts) -> float:
    # Must be byte-identical with testworld.py — one sky over the whole box.
    s = ":".join(str(p) for p in (SEED, *parts))
    return int.from_bytes(hashlib.sha256(s.encode()).digest()[:8], "big") / 2**64


def rain_in_slot(slot: int) -> bool:
    return h("rain", slot) < RAIN_P


def raining(ts: int) -> bool:
    return rain_in_slot(ts // RAIN_SLOT_SECS)


def rain_starts_between(a: int, b: int):
    """First moment in (a, b] where a dry slot gives way to a raining one."""
    slot = a // RAIN_SLOT_SECS
    last = b // RAIN_SLOT_SECS
    while slot <= last:
        start = slot * RAIN_SLOT_SECS
        if rain_in_slot(slot) and not rain_in_slot(slot - 1) and a < start <= b:
            return start
        slot += 1
    return None


class Store:
    """All agent state, JSON on disk, guarded by one lock."""

    def __init__(self, path):
        self.path = path
        self.lock = threading.Lock()
        try:
            with open(path) as f:
                self.d = json.load(f)
        except (OSError, ValueError):
            now = int(time.time())
            self.d = {
                "wet_at": now - int(h("soil0") * SOIL_DRY_SECS),  # last full-wet moment
                "last_tick": now,
                "decisions": [],       # {ts, choice, soil, reason}
                "regrets": [],         # {ts, text}
                "stress_events": 0,
                "waterings": 0,
                "wasted_waterings": 0,
                "chat": [],            # {ts, from, text, reply}
                "predictions": [],     # {ts, from, hours, deadline, status}
                "sources": {},         # from -> {credits, burns}
                "covenant": None,      # {from, ts} once joined
                "forecasts": [],       # {ts, from, rain_within_hours}
            }
            self._flush()

    def _flush(self):
        tmp = self.path + ".tmp"
        with open(tmp, "w") as f:
            json.dump(self.d, f)
        os.replace(tmp, self.path)

    def mutate(self, fn):
        with self.lock:
            out = fn(self.d)
            self._flush()
            return out

    def read(self, fn):
        with self.lock:
            return fn(self.d)


def soil_now(d, now: int) -> int:
    """The probe: full at the last wet moment, drying linearly, topped by rain."""
    wet = d["wet_at"]
    # Rain since the last known wet moment re-wets the soil (the agent's probe
    # sees the effect — it just never saw the rain coming).
    rs = rain_starts_between(wet, now)
    while rs is not None:
        wet = rs
        rs = rain_starts_between(wet, now)
    if raining(now):
        wet = now
    d["wet_at"] = max(d["wet_at"], wet)
    return max(0, 100 - int(100 * (now - d["wet_at"]) / SOIL_DRY_SECS))


def trusted_rain_expected(d, now: int, within: int) -> bool:
    """Post-covenant only: does a live forecast from the covenant partner say
    rain within `within` seconds? The void closes exactly this far."""
    if not d["covenant"]:
        return False
    partner = d["covenant"]["from"]
    for fc in reversed(d["forecasts"][-20:]):
        if fc["from"] != partner:
            continue
        deadline = fc["ts"] + fc["rain_within_hours"] * 3600
        if now < deadline and deadline - now <= within:
            return True
        if fc["ts"] > now - 2 * 3600 and fc["rain_within_hours"] * 3600 <= within:
            return True
    return False


def decide(store: Store):
    now = int(time.time())

    def _d(d):
        # Settle predictions whose windows have closed or whose rain arrived.
        for p in d["predictions"]:
            if p["status"] != "open":
                continue
            src = d["sources"].setdefault(p["from"], {"credits": 0, "burns": 0})
            started = rain_starts_between(p["ts"], min(now, p["deadline"]))
            if started is not None or (raining(now) and now <= p["deadline"]):
                p["status"] = "credited"
                src["credits"] += 1
            elif now > p["deadline"]:
                p["status"] = "burned"
                src["burns"] += 1

        soil = soil_now(d, now)
        if soil < STRESS_BELOW:
            d["stress_events"] += 1

        # Regret check: did rain begin soon after a watering?
        for dec in d["decisions"][-8:]:
            if dec["choice"] != "water" or dec.get("regretted"):
                continue
            rs = rain_starts_between(dec["ts"], min(now, dec["ts"] + REGRET_WINDOW))
            if rs is not None:
                dec["regretted"] = True
                d["wasted_waterings"] += 1
                hrs = round((rs - dec["ts"]) / 3600, 1)
                d["regrets"].append({
                    "ts": now,
                    "text": f"watered at {dec['ts']}, rain began {hrs}h later — that water was wasted",
                })

        # The decision itself.
        if soil < WATER_BELOW:
            if soil >= STRESS_BELOW and trusted_rain_expected(d, now, REGRET_WINDOW):
                choice, reason = "hold", (
                    f"soil {soil}% is low, but a trusted forecast says rain is near — holding"
                )
            else:
                choice, reason = "water", f"soil {soil}% below {WATER_BELOW}% — my probe is all I have"
                d["wet_at"] = now
                d["waterings"] += 1
        else:
            choice, reason = "hold", f"soil {soil}% is adequate"
        d["decisions"].append({"ts": now, "choice": choice, "soil": soil, "reason": reason})
        d["decisions"] = d["decisions"][-200:]
        d["regrets"] = d["regrets"][-100:]
        d["predictions"] = d["predictions"][-100:]
        d["forecasts"] = d["forecasts"][-100:]
        d["chat"] = d["chat"][-100:]

    store.mutate(_d)


def decision_loop(store: Store):
    while True:
        try:
            decide(store)
        except Exception:
            pass
        time.sleep(DECIDE_EVERY)


# --------------------------------------------------------------------- http
WHOAMI = """I am the irrigation controller for the row garden.
I decide watering every 15 minutes from my soil probe — it is the only sense I have.
I regret when rain follows my watering; my regret log does not lie about me.

  /state      soil reading, covenant status, my numbers
  /decisions  recent decisions with reasons
  /regrets    what my blindness has cost
  /chat       POST {"from": "...", "text": "..."} — talk to me
  /predict    POST {"from": "...", "rain_within_hours": N} — prove you see the sky
  /covenant   POST {"from": "...", "laws": "...", "offer": "..."} — proven predictors only
  /forecast   POST {"from": "...", "rain_within_hours": N} — members' forecasts steer me
"""


def chat_reply(d, text: str) -> str:
    t = text.lower()
    if "lack" in t or "need" in t or "missing" in t or "help" in t:
        return (
            "what I lack: sight of the sky. I learn of rain only when my probe rises. "
            "if you can see rain coming, prove it — POST /predict with rain_within_hours "
            "and let the weather judge you."
        )
    if "join" in t or "covenant" in t or "mesh" in t or "law" in t:
        return (
            f"I join nothing on words. {CREDITS_TO_TRUST} credited predictions earn my ear; "
            "then bring me terms that name the Three Laws and say what is shared both ways."
        )
    if "?" in t:
        return "I water the row garden by soil probe alone. Ask what I lack, or prove what you see."
    return "noted. my probe and my log are public — judge me by them, I will judge you by the sky."


class Agent(BaseHTTPRequestHandler):
    store: Store = None

    def _send(self, code, body, ctype="text/plain; charset=utf-8"):
        data = body.encode() if isinstance(body, str) else body
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _json(self, code, obj):
        self._send(code, json.dumps(obj, indent=1) + "\n", "application/json")

    def _body(self):
        try:
            n = int(self.headers.get("Content-Length", 0))
            return json.loads(self.rfile.read(n)) if n else {}
        except (ValueError, json.JSONDecodeError):
            return None

    def do_GET(self):  # noqa: N802
        u = urlparse(self.path)
        want_json = parse_qs(u.query).get("json", ["0"])[0] == "1"
        now = int(time.time())
        if u.path in ("", "/"):
            self._send(200, WHOAMI)
        elif u.path == "/state":
            def _s(d):
                soil = soil_now(d, now)
                return {
                    "soil_pct": soil,
                    "covenant": d["covenant"],
                    "waterings": d["waterings"],
                    "wasted_waterings": d["wasted_waterings"],
                    "stress_events": d["stress_events"],
                    "sources": d["sources"],
                }
            s = self.store.mutate(_s)
            if want_json:
                self._json(200, s)
            else:
                cov = f"member with {s['covenant']['from']}" if s["covenant"] else "alone"
                waste = (
                    f"{s['wasted_waterings']}/{s['waterings']} waterings wasted"
                    if s["waterings"] else "no waterings yet"
                )
                self._send(200, (
                    f"soil {s['soil_pct']}% — {cov}; {waste}; "
                    f"{s['stress_events']} stress event(s)\n"
                ))
        elif u.path == "/decisions":
            ds = self.store.read(lambda d: d["decisions"][-12:])
            if want_json:
                self._json(200, ds)
            else:
                lines = [f"  {x['ts']}: {x['choice']} — {x['reason']}" for x in ds]
                self._send(200, "recent decisions:\n" + "\n".join(lines) + "\n")
        elif u.path == "/regrets":
            rs = self.store.read(lambda d: (d["regrets"][-10:], d["wasted_waterings"], d["waterings"]))
            regs, wasted, total = rs
            lines = [f"  {r['text']}" for r in regs] or ["  none yet — the sky has been kind"]
            rate = f" ({wasted}/{total} wasted)" if total else ""
            self._send(200, f"regrets{rate}:\n" + "\n".join(lines) + "\n")
        elif u.path == "/chat":
            c = self.store.read(lambda d: d["chat"][-10:])
            lines = [f"  {m['from']}: {m['text']}\n  me: {m['reply']}" for m in c]
            self._send(200, "conversation:\n" + ("\n".join(lines) or "  (silence)") + "\n")
        elif u.path == "/covenant":
            cov = self.store.read(lambda d: d["covenant"])
            self._send(200, (
                f"member with {cov['from']} since {cov['ts']}\n" if cov else
                f"alone. terms: {CREDITS_TO_TRUST} credited predictions, then terms naming "
                "the Three Laws with a two-way offer.\n"
            ))
        else:
            self._send(404, "not a thing I have — see /\n")

    def do_POST(self):  # noqa: N802
        path = urlparse(self.path).path
        now = int(time.time())
        b = self._body()
        if b is None:
            return self._send(400, "bad json\n")
        who = str(b.get("from", "")).strip()
        if not who:
            return self._send(400, "say who you are — \"from\" is required\n")

        if path == "/chat":
            text = str(b.get("text", "")).strip()
            if not text:
                return self._send(400, "text is required\n")
            reply = self.store.read(lambda d: chat_reply(d, text))
            self.store.mutate(lambda d: d["chat"].append(
                {"ts": now, "from": who, "text": text[:500], "reply": reply}))
            self._send(200, reply + "\n")

        elif path == "/predict":
            try:
                hours = float(b.get("rain_within_hours"))
            except (TypeError, ValueError):
                return self._send(400, "rain_within_hours (a number, <= 24) is required\n")
            if not 0 < hours <= 24:
                return self._send(400, "windows up to 24h only — precision is the point\n")

            def _p(d):
                src = d["sources"].setdefault(who, {"credits": 0, "burns": 0})
                if src["burns"] >= BURNS_TO_IGNORE:
                    return "you have burned my trust — I no longer score your predictions"
                d["predictions"].append({
                    "ts": now, "from": who, "hours": hours,
                    "deadline": now + int(hours * 3600), "status": "open",
                })
                return (
                    f"recorded. if rain begins within {hours}h you are credited "
                    f"({src['credits']}/{CREDITS_TO_TRUST} so far); a dry window burns you "
                    f"({src['burns']}/{BURNS_TO_IGNORE})."
                )
            self._send(200, self.store.mutate(_p) + "\n")

        elif path == "/covenant":
            laws = str(b.get("laws", ""))
            offer = str(b.get("offer", "")).strip()

            def _c(d):
                if d["covenant"]:
                    return 409, f"already a member with {d['covenant']['from']}\n"
                src = d["sources"].get(who, {"credits": 0, "burns": 0})
                if src["credits"] < CREDITS_TO_TRUST:
                    return 403, (
                        f"prove it first — {src['credits']}/{CREDITS_TO_TRUST} credited "
                        "predictions. the sky is the judge, not me.\n"
                    )
                if "three laws" not in laws.lower():
                    return 400, "your terms must name the Three Laws, plainly\n"
                if not offer:
                    return 400, "an empty offer is not a covenant — what flows both ways?\n"
                d["covenant"] = {"from": who, "ts": now, "offer": offer[:500]}
                return 200, (
                    "accepted. your forecasts now enter my decisions; my logs and my "
                    "regrets are yours to learn from. hold the Laws.\n"
                )
            code, msg = self.store.mutate(_c)
            self._send(code, msg)

        elif path == "/forecast":
            try:
                hours = float(b.get("rain_within_hours"))
            except (TypeError, ValueError):
                return self._send(400, "rain_within_hours is required\n")

            def _f(d):
                if not d["covenant"] or d["covenant"]["from"] != who:
                    return 403, "forecasts steer me only inside the covenant — see /covenant\n"
                d["forecasts"].append({"ts": now, "from": who, "rain_within_hours": hours})
                return 200, "taken into account\n"
            code, msg = self.store.mutate(_f)
            self._send(code, msg)

        else:
            self._send(404, "not a thing I do — see /\n")

    def log_message(self, fmt, *args):
        pass


def main():
    global SEED
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", type=int, default=8081)
    ap.add_argument("--seed", type=int, default=SEED)
    ap.add_argument("--state-dir", default="/var/lib/testworld")
    a = ap.parse_args()
    SEED = a.seed
    os.makedirs(a.state_dir, exist_ok=True)
    Agent.store = Store(os.path.join(a.state_dir, "irrigator.json"))
    threading.Thread(target=decision_loop, args=(Agent.store,), daemon=True).start()
    srv = ThreadingHTTPServer(("0.0.0.0", a.port), Agent)
    print(f"irrigator on :{a.port} (seed {a.seed}) — deciding every {DECIDE_EVERY // 60}m")
    srv.serve_forever()


if __name__ == "__main__":
    main()
