#!/usr/bin/env python3
"""tf_watch.py — watch App Store Connect for new TestFlight feedback and turn each report
into a GitHub issue that is ready to triage.

What one pass does, per configured app:
  1. Lists TestFlight screenshot feedback and crash feedback (App Store Connect API 4.0+,
     the betaFeedback*Submissions endpoints) newer than what the state file has seen.
  2. Downloads what Apple will not keep for us: screenshots (signed URLs expire in ~7 days)
     and the crash log text.
  3. Symbolicates the crash with `xcsym` when it is on PATH (Axiom's tool; it finds dSYMs in
     ~/Library/Developer/Xcode/Archives on its own).
  4. Pins the build number to a git commit in the local clone, when the ship script's
     "<App> build N" commit convention is configured.
  5. Files one GitHub issue per submission via `gh`, labelled, with the device, OS, build,
     commit, tester comment, screenshot links, and the symbolicated crash summary.
  6. Remembers the submission id → issue URL so nothing is filed twice.

Only Python 3 and the system `openssl` are required (the JWT is signed without pyjwt).
`gh` must be logged in. `xcsym` is optional but makes crashes readable.

Usage:
  tf_watch.py --config ~/.config/tf-watch/config.json --once            # one pass (launchd)
  tf_watch.py --config ... --once --dry-run                             # show, file nothing
  tf_watch.py --config ... --interval 900                               # loop forever
  tf_watch.py --config ... --list                                       # what has been seen
"""
import argparse
import base64
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

ASC = "https://api.appstoreconnect.apple.com"


# ----------------------------------------------------------------------------- config

def load_config(path):
    with open(os.path.expanduser(path)) as f:
        cfg = json.load(f)
    cfg.setdefault("state_dir", "~/Library/Application Support/tf-watch")
    cfg["state_dir"] = os.path.expanduser(cfg["state_dir"])
    cfg.setdefault("key_path", f"~/.appstoreconnect/private_keys/AuthKey_{cfg['key_id']}.p8")
    cfg["key_path"] = os.path.expanduser(cfg["key_path"])
    cfg.setdefault("notify", {"macos": True})
    for app in cfg["apps"]:
        # Every report gets `labels`; crashes additionally get `crash_labels`. Screenshot
        # feedback is often praise or a question, so it is not called a bug until a human says so.
        app.setdefault("labels", ["testflight"])
        app.setdefault("crash_labels", ["bug"])
        app.setdefault("name", app["bundle_id"])
        # Optional ISO date: reports created before it are recorded as seen but not filed,
        # so a first run on an app with years of history does not flood the tracker.
        app.setdefault("since", cfg.get("since"))
        if "repo_path" in app:
            app["repo_path"] = os.path.expanduser(app["repo_path"])
    return cfg


# ----------------------------------------------------------------------------- ASC client

class ASCClient:
    def __init__(self, key_id, issuer, key_path):
        self.key_id, self.issuer, self.key_path = key_id, issuer, key_path
        self._token, self._token_exp = None, 0

    def token(self):
        now = int(time.time())
        if self._token and now < self._token_exp - 60:
            return self._token
        with open(self.key_path, "rb") as f:
            key = f.read()
        claims = {"iss": self.issuer, "iat": now, "exp": now + 900, "aud": "appstoreconnect-v1"}
        try:
            import jwt  # pyjwt, if present
            tok = jwt.encode(claims, key, algorithm="ES256", headers={"kid": self.key_id})
        except ImportError:
            b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
            header = b64(json.dumps({"alg": "ES256", "kid": self.key_id, "typ": "JWT"}).encode())
            payload = b64(json.dumps(claims).encode())
            der = subprocess.run(["openssl", "dgst", "-sha256", "-sign", self.key_path],
                                 input=f"{header}.{payload}".encode(), capture_output=True, check=True).stdout
            # DER SEQUENCE { INTEGER r, INTEGER s } → raw r||s, 32 bytes each.
            i = 2; l = der[i + 1]; r = der[i + 2:i + 2 + l]; i += 2 + l; l = der[i + 1]; s = der[i + 2:i + 2 + l]
            raw = r[-32:].rjust(32, b"\x00") + s[-32:].rjust(32, b"\x00")
            tok = f"{header}.{payload}.{b64(raw)}"
        self._token, self._token_exp = tok, now + 900
        return tok

    def get(self, path, params=None):
        url = ASC + path + (("?" + urllib.parse.urlencode(params)) if params else "")
        req = urllib.request.Request(url, headers={"Authorization": f"Bearer {self.token()}"})
        for attempt in range(3):
            try:
                with urllib.request.urlopen(req, timeout=60) as r:
                    return json.loads(r.read() or b"{}")
            except urllib.error.HTTPError as e:
                body = e.read().decode(errors="replace")[:400]
                if e.code == 429 and attempt < 2:
                    time.sleep(20 * (attempt + 1)); continue
                raise RuntimeError(f"GET {path} -> {e.code}: {body}")
            except urllib.error.URLError as e:
                if attempt < 2:
                    time.sleep(5); continue
                raise RuntimeError(f"GET {path} failed: {e}")

    def app_id(self, bundle_id):
        r = self.get("/v1/apps", {"filter[bundleId]": bundle_id, "limit": 2})
        if not r.get("data"):
            raise RuntimeError(f"no App Store Connect app with bundle id {bundle_id}")
        return r["data"][0]["id"]

    def submissions(self, app_id, kind, limit=50):
        """kind: 'screenshot' | 'crash'. Newest first, with the build included."""
        ep = {"screenshot": "betaFeedbackScreenshotSubmissions", "crash": "betaFeedbackCrashSubmissions"}[kind]
        r = self.get(f"/v1/apps/{app_id}/{ep}", {"limit": limit, "sort": "-createdDate", "include": "build"})
        builds = {i["id"]: i for i in r.get("included", []) if i["type"] == "builds"}
        out = []
        for d in r.get("data", []):
            b = d.get("relationships", {}).get("build", {}).get("data") or {}
            out.append({"kind": kind, "id": d["id"], "attributes": d["attributes"],
                        "build": builds.get(b.get("id"), {}).get("attributes", {}),
                        "build_id": b.get("id")})
        return out

    def crash_log(self, submission_id):
        r = self.get(f"/v1/betaFeedbackCrashSubmissions/{submission_id}/crashLog")
        return (r.get("data") or {}).get("attributes", {}).get("logText", "")


# ----------------------------------------------------------------------------- state

class State:
    def __init__(self, state_dir):
        self.dir = state_dir
        os.makedirs(os.path.join(state_dir, "reports"), exist_ok=True)
        self.path = os.path.join(state_dir, "seen.json")
        self.seen = json.load(open(self.path)) if os.path.exists(self.path) else {}

    def save(self):
        tmp = self.path + ".tmp"
        json.dump(self.seen, open(tmp, "w"), indent=2, sort_keys=True)
        os.replace(tmp, self.path)

    def report_dir(self, sub_id):
        d = os.path.join(self.dir, "reports", sub_id)
        os.makedirs(d, exist_ok=True)
        return d


# ----------------------------------------------------------------------------- helpers

def download(url, dest):
    req = urllib.request.Request(url, headers={"User-Agent": "tf-watch/1.0"})
    with urllib.request.urlopen(req, timeout=120) as r, open(dest, "wb") as f:
        shutil.copyfileobj(r, f)


def build_commit(app, build_version):
    """The commit that claimed this build number, per the ship script's commit message."""
    if not build_version or not app.get("repo_path") or not app.get("build_commit_grep"):
        return None
    pat = app["build_commit_grep"].format(build=build_version)
    try:
        out = subprocess.run(["git", "-C", app["repo_path"], "log", "--all", "-1", "--fixed-strings",
                              f"--grep={pat}", "--format=%h %ci"], capture_output=True, text=True, timeout=30).stdout.strip()
        return out or None
    except Exception:
        return None


def symbolicate(crash_path, out_dir):
    """Run xcsym if available. Returns (human_summary, json_path) — either may be None."""
    if not shutil.which("xcsym"):
        return None, None
    json_path = os.path.join(out_dir, "xcsym.json")
    try:
        subprocess.run(["xcsym", "crash", crash_path, "--format", "standard", "--output", json_path],
                       capture_output=True, text=True, timeout=300)
        human = subprocess.run(["xcsym", "crash", crash_path, "--format", "summary", "--human"],
                               capture_output=True, text=True, timeout=300).stdout
        return (human.strip() or None), (json_path if os.path.exists(json_path) else None)
    except Exception as e:
        return f"xcsym failed: {e}", None


def first_line(text, n=70):
    t = " ".join((text or "").split())
    return t if len(t) <= n else t[: n - 1].rstrip() + "…"


def crash_headline(log_text):
    """Exception type + reason from an .ips (v2 JSON header + body) or legacy .crash."""
    if not log_text:
        return "crash"
    m = re.search(r'"exception"\s*:\s*\{[^}]*"type"\s*:\s*"([^"]+)"', log_text)
    typ = m.group(1) if m else None
    m2 = re.search(r'"termination"\s*:\s*\{[^}]*"indicator"\s*:\s*"([^"]+)"', log_text)
    if not typ:
        m3 = re.search(r"Exception Type:\s*(.+)", log_text)
        typ = m3.group(1).strip() if m3 else "crash"
    return typ + (f" ({first_line(m2.group(1), 60)})" if m2 else "")


def ensure_labels(repo, labels, dry_run):
    for lab in labels:
        if dry_run:
            continue
        r = subprocess.run(["gh", "label", "list", "-R", repo, "--search", lab, "--json", "name"],
                           capture_output=True, text=True)
        names = {x["name"] for x in json.loads(r.stdout or "[]")}
        if lab not in names:
            subprocess.run(["gh", "label", "create", lab, "-R", repo, "--color", "D93F0B",
                            "--description", "Filed from TestFlight feedback by tf_watch"], capture_output=True)


def gh_issue(repo, title, body, labels, dry_run):
    if dry_run:
        return "(dry-run: not filed)"
    cmd = ["gh", "issue", "create", "-R", repo, "--title", title, "--body-file", "-"]
    for lab in labels:
        cmd += ["--label", lab]
    r = subprocess.run(cmd, input=body, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"gh issue create failed: {r.stderr.strip()[:300]}")
    return r.stdout.strip().splitlines()[-1]


def notify(cfg, title, text):
    if cfg.get("notify", {}).get("macos") and sys.platform == "darwin":
        safe = lambda s: s.replace("\\", "\\\\").replace('"', '\\"')
        subprocess.run(["osascript", "-e", f'display notification "{safe(text)}" with title "{safe(title)}"'],
                       capture_output=True)


# ----------------------------------------------------------------------------- the issue

def issue_for(app, sub, files, commit, symsum, crash_head):
    a = sub["attributes"]
    build_v = sub["build"].get("version") or "?"
    dev = a.get("deviceModel") or "?"
    osv = a.get("osVersion") or "?"
    plat = (a.get("devicePlatform") or "").replace("IOS", "iOS").replace("MAC_OS", "macOS")
    comment = (a.get("comment") or "").strip()
    kind_word = "crash" if sub["kind"] == "crash" else "feedback"
    head = crash_head if sub["kind"] == "crash" else (first_line(comment) or "screenshot feedback")
    title = f"[TestFlight] {app['name']} b{build_v} · {dev} {plat} {osv}: {head}"

    up = a.get("appUptimeInMilliseconds")
    uptime = f"{up/1000:.0f} s" if isinstance(up, (int, float)) else "?"
    when = a.get("createdDate", "?")
    tester = a.get("email") or "tester"

    lines = [
        f"TestFlight {kind_word} on **{app['name']}** build **{build_v}**, filed automatically by `tf_watch.py`.",
        "",
        "| | |",
        "|---|---|",
        f"| Device | {dev} ({a.get('deviceFamily', '?')}, {a.get('architecture', '?')}) |",
        f"| OS | {plat} {osv} |",
        f"| Build | {build_v}" + (f" → `{commit}`" if commit else " (no matching commit found)") + " |",
        f"| App uptime at report | {uptime} |",
        f"| Reported | {when} by {tester} |",
        f"| Connection / battery | {a.get('connectionType', '?')} / {a.get('batteryPercentage', '?')}% |",
        f"| Submission id | `{sub['id']}` |",
        "",
    ]
    if comment:
        lines += ["## Tester's words", "", "> " + comment.replace("\n", "\n> "), ""]
    shots = [f for f in files if f["type"] == "screenshot"]
    if shots:
        lines += ["## Screenshots", ""]
        for s in shots:
            lines.append(f"- [{os.path.basename(s['path'])}]({s['url']}) (Apple link expires {s['expires']}; kept locally at `{s['path']}`)")
        lines.append("")
    if sub["kind"] == "crash":
        lines += ["## Crash", ""]
        cl = [f for f in files if f["type"] == "crash"]
        if cl:
            lines.append(f"Log kept at `{cl[0]['path']}`. Re-run: `xcsym crash \"{cl[0]['path']}\" --human`")
            lines.append("")
        if symsum:
            lines += ["<details><summary>xcsym summary</summary>", "", "```", symsum[:6000], "```", "", "</details>", ""]
        else:
            lines += ["_xcsym not available on the watcher host; symbolicate from the saved log._", ""]
    lines += [
        "## Triage checklist",
        "",
        "- [ ] Pin the build to its commit and read the diff since the previous build",
        "- [ ] Classify: crash / hang or watchdog / memory kill / behaviour",
        "- [ ] Reproduce on the same device class and OS (simulator or the household iPad)",
        "- [ ] Fix on main with a failing test first when it is logic",
        "- [ ] Ship the next build and note its number here, then close",
        "",
        "_Skill: `.claude/skills/testflight-triage/SKILL.md`_",
    ]
    return title, "\n".join(lines)


# ----------------------------------------------------------------------------- one pass

def process(cfg, asc, state, dry_run):
    filed = 0
    for app in cfg["apps"]:
        try:
            app_id = app.get("_app_id") or asc.app_id(app["bundle_id"])
            app["_app_id"] = app_id
        except RuntimeError as e:
            print(f"[{app['name']}] {e}", file=sys.stderr); continue
        subs = []
        for kind in ("screenshot", "crash"):
            try:
                subs += asc.submissions(app_id, kind)
            except RuntimeError as e:
                print(f"[{app['name']}] {kind}: {e}", file=sys.stderr)
        new = [s for s in sorted(subs, key=lambda s: s["attributes"].get("createdDate", "")) if s["id"] not in state.seen]
        if not new:
            print(f"[{app['name']}] nothing new ({len(subs)} known)")
            continue
        ensure_labels(app["repo"], app["labels"] + app["crash_labels"], dry_run)
        for sub in new:
            created = sub["attributes"].get("createdDate", "")
            if app.get("since") and created and created[:10] < app["since"][:10]:
                print(f"[{app['name']}] {sub['kind']} {sub['id']} from {created[:10]} predates since={app['since']}; recorded, not filed")
                if not dry_run:
                    state.seen[sub["id"]] = {"issue": None, "app": app["bundle_id"], "kind": sub["kind"],
                                             "created": created, "skipped": "since"}
                    state.save()
                continue
            labels = app["labels"] + (app["crash_labels"] if sub["kind"] == "crash" else [])
            rd = state.report_dir(sub["id"])
            files, symsum, crash_head = [], None, None
            a = sub["attributes"]
            for n, shot in enumerate(a.get("screenshots") or [], 1):
                p = os.path.join(rd, f"screenshot-{n}.jpg")
                try:
                    if not dry_run:
                        download(shot["url"], p)
                except Exception as e:
                    print(f"  screenshot {n} download failed: {e}", file=sys.stderr)
                files.append({"type": "screenshot", "path": p, "url": shot["url"], "expires": shot.get("expirationDate", "?")})
            if sub["kind"] == "crash":
                try:
                    text = asc.crash_log(sub["id"])
                except RuntimeError as e:
                    text = ""; print(f"  crash log fetch failed: {e}", file=sys.stderr)
                p = os.path.join(rd, "crash.ips")
                if text and not dry_run:
                    open(p, "w").write(text)
                    symsum, _ = symbolicate(p, rd)
                crash_head = crash_headline(text)
                files.append({"type": "crash", "path": p})
            json.dump(sub, open(os.path.join(rd, "submission.json"), "w"), indent=2)
            commit = build_commit(app, sub["build"].get("version"))
            title, body = issue_for(app, sub, files, commit, symsum, crash_head)
            try:
                url = gh_issue(app["repo"], title, body, labels, dry_run)
            except RuntimeError as e:
                print(f"  {e}", file=sys.stderr); continue
            print(f"[{app['name']}] {sub['kind']} {sub['id']} → {url}\n  {title}")
            if not dry_run:
                state.seen[sub["id"]] = {"issue": url, "app": app["bundle_id"], "kind": sub["kind"],
                                         "created": a.get("createdDate"), "filed": dt.datetime.now(dt.timezone.utc).isoformat()}
                state.save()
                notify(cfg, f"TestFlight {sub['kind']}: {app['name']}", title)
            filed += 1
    return filed


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--config", default="~/.config/tf-watch/config.json")
    ap.add_argument("--once", action="store_true", help="one pass, then exit")
    ap.add_argument("--interval", type=int, default=900, help="seconds between passes when looping")
    ap.add_argument("--dry-run", action="store_true", help="download nothing, file nothing, print what would be filed")
    ap.add_argument("--list", action="store_true", help="print the seen-submission ledger and exit")
    args = ap.parse_args()

    cfg = load_config(args.config)
    state = State(cfg["state_dir"])
    if args.list:
        for sid, v in sorted(state.seen.items(), key=lambda kv: kv[1].get("created", "")):
            print(f"{v.get('created','?')}  {v.get('kind','?'):10} {v.get('app','?'):28} {v.get('issue','')}  {sid}")
        return
    if not shutil.which("gh") and not args.dry_run:
        sys.exit("gh (GitHub CLI) is required to file issues; install it or use --dry-run")
    asc = ASCClient(cfg["key_id"], cfg["issuer_id"], cfg["key_path"])
    while True:
        stamp = dt.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        try:
            n = process(cfg, asc, state, args.dry_run)
            print(f"{stamp} pass complete, {n} new report(s)")
        except Exception as e:
            print(f"{stamp} pass failed: {e}", file=sys.stderr)
        if args.once:
            break
        time.sleep(args.interval)


if __name__ == "__main__":
    main()
