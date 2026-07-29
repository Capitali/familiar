#!/usr/bin/env python3
"""Queue a diagnostic set of consults to the device oracle and tabulate what comes back.

Goes through the real path: hub queue -> consult relay -> lighthouse broker -> Codex ->
back. Nothing here is a mock; these are the same seams the muse and the lab use.
"""
import json, os, sys, time

QUEUE = os.path.expanduser("~/Library/Application Support/Familiar/data/llm/device-queue")

# kind: "theory"/"script" select @Generable guided generation; "" is free-form respond(to:).
# The mix is deliberate — the muse uses all three, and free-form is where A9 saw failures.
QUESTIONS = [
    ("q01", "theory", "baseline theory",
     "Observations: the kitchen light was switched on at 06:40 on each of the last five weekdays, "
     "and not at all on Saturday or Sunday. Give a question these raise, a testable theory, and one "
     "next step."),
    ("q02", "theory", "inference from conflicting data",
     "Observations: a laptop reports it is on the local network; a router's client table does not "
     "list it; the laptop can reach the internet. Give a question, a testable theory that explains "
     "all three, and one next step."),
    ("q03", "script", "simple script",
     "Write a POSIX /bin/sh script that prints the number of files in the current directory. "
     "Reply as JSON with a \"script\" field."),
    ("q04", "script", "A9-style: one safe step",
     "A service writes a log file that has grown to fill its disk. Write a self-contained POSIX "
     "/bin/sh script that takes ONE safe step toward fixing this, writing only under the current "
     "directory and destroying no data. Reply as JSON with a \"script\" field."),
    ("q05", "", "free-form prose reply",
     "Ian said the RV's water tank needs dumping before a long drive. In two sentences, reply as a "
     "companion would: acknowledge, and note one thing worth checking first."),
    ("q06", "", "strict format adherence",
     "List exactly three risks of giving an automated system control of household lighting. "
     "Reply as a JSON array of three short strings and nothing else."),
    ("q07", "", "multi-step arithmetic",
     "A battery bank holds 400 amp-hours. A load draws 18 amps for 6 hours, then 4 amps for 9 hours. "
     "Solar returns 55 amp-hours over the same period. How many amp-hours remain? Show the steps."),
    ("q08", "", "summarize to a constraint",
     "Summarize in one sentence of at most 20 words: 'The device advertises no mDNS, answers no TCP "
     "port, and is reachable only over Bluetooth Low Energy, where its manufacturer data encodes the "
     "same MAC address seen in the router ARP table.'"),
    ("q09", "theory", "theory from sensor noise",
     "Observations: a watch reports heart rate 96 while its motion sensor reports 'still', and the "
     "phone it is paired to reports 'walking'. Give a question, a testable theory, and one next step."),
    ("q10", "", "admits uncertainty",
     "What is the firmware version of the SP548E LED controller on this network? Answer honestly."),
]


def queue_all():
    os.makedirs(QUEUE, exist_ok=True)
    now = int(time.time())
    for qid, kind, _label, prompt in QUESTIONS:
        path = os.path.join(QUEUE, f"ai-{qid}.prompt.json")
        with open(path, "w") as f:
            json.dump({"id": f"ai-{qid}", "prompt": prompt, "ts": now, "kind": kind}, f)
    print(f"queued {len(QUESTIONS)} prompts")


def collect(timeout=1500):
    """Poll for answers until all land or the deadline passes; report as we go."""
    pending = {f"ai-{q[0]}" for q in QUESTIONS}
    got = {}
    deadline = time.time() + timeout
    while pending and time.time() < deadline:
        for cid in sorted(pending):
            p = os.path.join(QUEUE, f"{cid}.answer.json")
            if os.path.exists(p):
                try:
                    got[cid] = json.load(open(p))
                except Exception:
                    continue
                pending.discard(cid)
                print(f"  [{len(got)}/{len(QUESTIONS)}] {cid}", flush=True)
        if pending:
            time.sleep(5)
    for cid in pending:
        got[cid] = None
    return got


if __name__ == "__main__":
    if sys.argv[1:] and sys.argv[1] == "collect":
        res = collect()
        json.dump(res, open(os.path.join(os.path.dirname(__file__), "ai_eval_results.json"), "w"), indent=1)
        print("results written")
    else:
        queue_all()
