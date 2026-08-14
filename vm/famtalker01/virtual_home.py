#!/usr/bin/env python3
"""A tiny, persistent virtual smart home for FamTalker01.

The actuator side deliberately prints ADR-0032's existing motorlights-shaped state
contract.  The observation side emits ordinary /local/observe JSON objects.  There is
no network listener and no hidden control path: the human-installed actuators.json is
the only route by which the familiar may drive these surfaces.
"""

import argparse
import fcntl
import json
import os
import tempfile
from pathlib import Path


SURFACES = {
    "living-room-lights": "bright",
    "greenhouse-lights": "off",
}
LEVELS = {
    "off": (False, 0),
    "dim": (True, 25),
    "bright": (True, 100),
}


def load(path: Path) -> dict[str, str]:
    try:
        value = json.loads(path.read_text())
    except FileNotFoundError:
        value = {}
    if not isinstance(value, dict):
        raise ValueError(f"{path} is not a JSON object")
    state = {}
    for surface, initial in SURFACES.items():
        label = value.get(surface, initial)
        if label not in LEVELS:
            raise ValueError(f"{path}: unknown state for {surface}: {label!r}")
        state[surface] = label
    return state


def save(path: Path, state: dict[str, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix="state.", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(state, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def state_text(label: str) -> str:
    on, percent = LEVELS[label]
    raw = round(percent * 255 / 100)
    mode = "0x01 Virtual Light" if on else "0x00 Off"
    return f"light mode : {mode}\nbrightness : {raw}/255  ({percent}%)"


def observations(state: dict[str, str]) -> list[dict]:
    records = []
    watts = 0
    for surface in sorted(state):
        label = state[surface]
        _, percent = LEVELS[label]
        watts += {"off": 0, "dim": 3, "bright": 9}[label]
        room = surface.removesuffix("-lights")
        records.append(
            {
                "actor": f"virtual-home:{room}",
                "action": "reports",
                "object": f"lighting:{label}",
                "context": (
                    f"{room.replace('-', ' ')} lights are {label} "
                    f"({percent}% brightness)"
                ),
                "confidence": 1.0,
            }
        )
    records.append(
        {
            "actor": "virtual-home:energy-meter",
            "action": "reports",
            "object": f"lighting-draw:{watts}W",
            "context": f"the two declared lighting surfaces draw {watts} watts together",
            "confidence": 1.0,
        }
    )
    return records


def parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--state-dir", default="/var/lib/familiar/virtual-home")
    sub = ap.add_subparsers(dest="command", required=True)
    surface = sub.add_parser("surface", help="read or set a declared surface")
    surface.add_argument("name", choices=sorted(SURFACES))
    surface.add_argument("act", choices=["state", *LEVELS])
    sub.add_parser("observations", help="emit the current observation snapshot as JSONL")
    return ap


def main() -> int:
    args = parser().parse_args()
    state_dir = Path(args.state_dir)
    state_dir.mkdir(parents=True, exist_ok=True)
    with (state_dir / ".lock").open("a+") as lock:
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
        path = state_dir / "state.json"
        state = load(path)
        if args.command == "surface":
            if args.act != "state":
                state[args.name] = args.act
                save(path, state)
            print(state_text(state[args.name]))
            return 0
        for record in observations(state):
            print(json.dumps(record, separators=(",", ":"), sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
