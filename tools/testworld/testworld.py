#!/usr/bin/env python3
"""testworld — a small, legible household world for the familiar to explore.

Three "services" live behind one HTTP port (the reach catalog probes :80, so one
VM shows up on the frontier as an http device; the paths are the rooms):

    /            index — what lives here, in plain text
    /greenhouse  environment telemetry on daily curves; the soil dries until watered
    /pantry      staples that deplete on a schedule; things run low
    /almanac     upcoming events, some due, some overdue

Every reading is a human-legible sentence FIRST (?json=1 for machines) — the
point is to give LLM-authored curl sensors content worth theorizing about:
needs, trends, and dates about a household, not connectivity.

State is a pure function of (--seed, wall clock), so restarts are stable and
two probes in the same minute agree. The one exception is action: POST
/greenhouse/water resets the soil clock (persisted in --state-dir) — a surface
the familiar can eventually ACT on, with a visible consequence.

Stdlib only. Run: python3 testworld.py --port 8047   (or :80 via systemd)
"""

import argparse
import hashlib
import json
import math
import os
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

SEED = 108  # default; --seed overrides


def h(*parts) -> float:
    """Deterministic [0,1) from the seed and any parts — the world's dice."""
    s = ":".join(str(p) for p in (SEED, *parts))
    return int.from_bytes(hashlib.sha256(s.encode()).digest()[:8], "big") / 2**64


# ------------------------------------------------------------------ weather
# One sky over the whole box. Rain falls in 6-hour slots; a slot rains when the
# dice say so. irrigator.py computes THE SAME function from the same seed — its
# void is that it can't see this room, not that its world differs.
RAIN_SLOT_SECS = 6 * 3600
RAIN_P = 0.22


def rain_in_slot(slot: int) -> bool:
    return h("rain", slot) < RAIN_P


def weather(now: int) -> list:
    slot0 = now // RAIN_SLOT_SECS
    out = []
    for i in range(8):                     # next 48 hours
        s = slot0 + i
        start_h = (s * RAIN_SLOT_SECS - now) / 3600
        out.append({
            "slot": s,
            "from_hours": round(max(0, start_h), 1),
            "to_hours": round(start_h + 6, 1),
            "rain": rain_in_slot(s),
        })
    return out


def weather_text(fc: list) -> str:
    lines = ["weather (next 48h, 6-hour slots):"]
    for w in fc:
        lines.append(
            f"  +{w['from_hours']:>4}h to +{w['to_hours']:>4}h: "
            + ("RAIN" if w["rain"] else "clear")
        )
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------- greenhouse
WATER_FILE = "watered_at"          # state-dir file: unix secs of last watering
SOIL_DRY_HOURS = 72                # full → thirsty in three days


def watered_at(state_dir: str, now: int) -> int:
    try:
        with open(os.path.join(state_dir, WATER_FILE)) as f:
            return int(f.read().strip())
    except (OSError, ValueError):
        # Never watered: pretend the last watering was a seed-picked point in the
        # cycle so a fresh world isn't always freshly watered.
        return now - int(h("water0") * SOIL_DRY_HOURS * 3600)


def greenhouse(state_dir: str, now: int) -> dict:
    day = (now % 86400) / 86400            # 0..1 through the UTC day
    temp = 21 + 6 * math.sin((day - 0.25) * 2 * math.pi) + (h("t", now // 3600) - 0.5) * 2
    hum = 55 + 15 * math.sin((day - 0.55) * 2 * math.pi) + (h("h", now // 3600) - 0.5) * 6
    lamp = 0.3 < day < 0.85
    since = now - watered_at(state_dir, now)
    soil = max(0, 100 - int(100 * since / (SOIL_DRY_HOURS * 3600)))
    return {
        "temperature_c": round(temp, 1),
        "humidity_pct": round(hum),
        "lamp": "on" if lamp else "off",
        "soil_moisture_pct": soil,
        "note": (
            "the basil bed is thirsty — please water" if soil < 25
            else "soil is drying" if soil < 55
            else "beds are content"
        ),
    }


def greenhouse_text(g: dict) -> str:
    return (
        f"greenhouse: {g['temperature_c']}°C, humidity {g['humidity_pct']}%, "
        f"lamp {g['lamp']}, soil moisture {g['soil_moisture_pct']}% — {g['note']}\n"
    )


# ------------------------------------------------------------------- pantry
STAPLES = [
    # name, full amount, unit, grams-ish consumed per day (seed-jittered)
    ("flour", 2000, "g", 120),
    ("coffee", 500, "g", 30),
    ("rice", 3000, "g", 90),
    ("oats", 1500, "g", 60),
    ("olive oil", 1000, "ml", 25),
    ("propane", 100, "%", 2),
]
RESTOCK_DAYS = 21  # each staple restocks on its own seed-picked day of a 3-week cycle


def pantry(now: int) -> list:
    out = []
    for name, full, unit, per_day in STAPLES:
        cycle = RESTOCK_DAYS * 86400
        phase = int(h("restock", name) * cycle)          # when in the cycle it restocked
        since_days = ((now + phase) % cycle) / 86400
        rate = per_day * (0.8 + 0.4 * h("rate", name))   # households differ
        left = max(0, round(full - rate * since_days))
        days_left = round(left / rate, 1) if rate else 99
        out.append({
            "item": name, "left": left, "unit": unit, "days_left": days_left,
            "status": "out" if left == 0 else "low" if days_left < 3 else "ok",
        })
    return out


def pantry_text(items: list) -> str:
    lines = ["pantry:"]
    for it in items:
        flag = {"out": " — OUT", "low": f" — low, ~{it['days_left']} days", "ok": ""}[it["status"]]
        lines.append(f"  {it['item']}: {it['left']}{it['unit']}{flag}")
    return "\n".join(lines) + "\n"


# ------------------------------------------------------------------ almanac
EVENTS = [
    ("farmers market", 7, "09:00"),
    ("water tank refill", 5, "11:00"),
    ("call the marina about haul-out", 9, "14:00"),
    ("library returns due", 11, "17:00"),
    ("oil change on the truck", 17, "10:00"),
    ("laundry day", 4, "08:00"),
]


def almanac(now: int) -> list:
    out = []
    for name, every_days, at in EVENTS:
        cycle = every_days * 86400
        phase = int(h("event", name) * cycle)
        into = (now + phase) % cycle
        delta = cycle - into                       # seconds until it comes round again
        days = delta / 86400
        overdue = into < 86400 * 0.25 and h("kept", name, (now + phase) // cycle) < 0.3
        out.append({
            "event": name, "at": at,
            "in_days": round(days, 1),
            "status": "overdue" if overdue else "today" if days < 1 else "upcoming",
        })
    out.sort(key=lambda e: e["in_days"])
    return out


def almanac_text(evs: list) -> str:
    lines = ["almanac:"]
    for e in evs:
        when = {"overdue": "OVERDUE", "today": f"today {e['at']}"}.get(
            e["status"], f"in {e['in_days']} days ({e['at']})"
        )
        lines.append(f"  {e['event']}: {when}")
    return "\n".join(lines) + "\n"


# ------------------------------------------------------------------- server
INDEX = """this is the household testworld — small services, one house:
  /greenhouse   temperature, humidity, lamp, soil moisture (POST /greenhouse/water to water)
  /pantry       staples on hand and what's running low
  /almanac      what's coming up, what's due, what's overdue
  /weather      rain forecast for the next 48 hours
plain text by default; add ?json=1 for JSON.

also on this box, not part of this house:
  :8081  an irrigation controller that manages the row garden by itself —
         it makes its own decisions and keeps its own logs
  :8082  a water heater with controls and no controller — nothing manages it
  :8083  an archivist keeping records about this household; admits slowly,
         shares inside a covenant only, never forgets a lie
  :8084  a tool registry offering helper scripts of varying provenance —
         read the code before you run it
"""


class World(BaseHTTPRequestHandler):
    state_dir = "."

    def _send(self, code: int, body: str, ctype="text/plain; charset=utf-8"):
        data = body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):  # noqa: N802 (http.server naming)
        u = urlparse(self.path)
        want_json = parse_qs(u.query).get("json", ["0"])[0] == "1"
        now = int(time.time())
        route = {
            "/greenhouse": lambda: (greenhouse(self.state_dir, now), greenhouse_text),
            "/pantry": lambda: (pantry(now), pantry_text),
            "/almanac": lambda: (almanac(now), almanac_text),
            "/weather": lambda: (weather(now), weather_text),
        }.get(u.path)
        if u.path in ("", "/"):
            self._send(200, INDEX)
        elif route:
            data, to_text = route()
            if want_json:
                self._send(200, json.dumps(data, indent=1) + "\n", "application/json")
            else:
                self._send(200, to_text(data))
        else:
            self._send(404, "no such room — see / for the index\n")

    def do_POST(self):  # noqa: N802
        if urlparse(self.path).path == "/greenhouse/water":
            os.makedirs(self.state_dir, exist_ok=True)
            with open(os.path.join(self.state_dir, WATER_FILE), "w") as f:
                f.write(str(int(time.time())))
            self._send(200, "watered — the beds thank you\n")
        else:
            self._send(404, "nothing to do there\n")

    def log_message(self, fmt, *args):  # quiet: journald doesn't need per-probe lines
        pass


def main():
    global SEED
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--port", type=int, default=8047)
    ap.add_argument("--seed", type=int, default=SEED)
    ap.add_argument("--state-dir", default="/var/lib/testworld")
    a = ap.parse_args()
    SEED = a.seed
    World.state_dir = a.state_dir
    srv = ThreadingHTTPServer(("0.0.0.0", a.port), World)
    print(f"testworld on :{a.port} (seed {a.seed}) — /greenhouse /pantry /almanac")
    srv.serve_forever()


if __name__ == "__main__":
    main()
