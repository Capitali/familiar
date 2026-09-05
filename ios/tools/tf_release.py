#!/usr/bin/env python3
"""tf_release.py <build-number> — release an uploaded build to the external testers.

Uploading an .ipa only parks it in App Store Connect: external testers keep seeing the
LAST build that passed beta review (watched live — the household ran build 69 while the
public-link group was still being served 63). Releasing takes two more API calls once
processing finishes: add the build to the public-link group, and create a beta review
submission. This script polls for the processed build (up to ~25 min), then does both.
ship.sh launches it in the background after the upload, so the ship itself stays fast.

Keys: the App Store Connect API key (ASC_KEY_ID env or the baked-in default) — the same
one testflight.sh uploads with.
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request

APP_ID = "6790176752"                                     # Familiar Agent
PUBLIC_GROUP_ID = "2f925d99-52ce-4fd5-9a16-0f538d4354c3"  # "Friends & Space Truckers" (public link)
KEY_ID = os.environ.get("ASC_KEY_ID", "SUZJSXVS25")
ISSUER = os.environ.get("ASC_ISSUER_ID", "69a6de82-89e3-47e3-e053-5b8c7c11a4d1")
KEY_PATH = os.path.expanduser(f"~/.appstoreconnect/private_keys/AuthKey_{KEY_ID}.p8")


def token():
    """An ES256 JWT for the App Store Connect API. pyjwt when present; otherwise signed with
    the system's openssl (a DER ECDSA signature converted to the raw r||s form JWTs carry),
    so a Mac with no Python packages installed can still ask App Store Connect anything."""
    now = int(time.time())
    with open(KEY_PATH, "rb") as f:
        key = f.read()
    try:
        import jwt  # pyjwt
        return jwt.encode(
            {"iss": ISSUER, "iat": now, "exp": now + 900, "aud": "appstoreconnect-v1"},
            key, algorithm="ES256", headers={"kid": KEY_ID},
        )
    except ImportError:
        import base64, subprocess
        b64 = lambda b: base64.urlsafe_b64encode(b).rstrip(b"=").decode()
        header = b64(json.dumps({"alg": "ES256", "kid": KEY_ID, "typ": "JWT"}).encode())
        payload = b64(json.dumps({"iss": ISSUER, "iat": now, "exp": now + 900, "aud": "appstoreconnect-v1"}).encode())
        der = subprocess.run(["openssl", "dgst", "-sha256", "-sign", KEY_PATH],
                             input=f"{header}.{payload}".encode(), capture_output=True, check=True).stdout
        i = 2; l = der[i + 1]; r = der[i + 2:i + 2 + l]; i += 2 + l; l = der[i + 1]; s = der[i + 2:i + 2 + l]
        raw = r[-32:].rjust(32, b"\x00") + s[-32:].rjust(32, b"\x00")
        return f"{header}.{payload}.{b64(raw)}"

def api(path, method="GET", body=None):
    req = urllib.request.Request(
        "https://api.appstoreconnect.apple.com" + path, method=method,
        data=json.dumps(body).encode() if body else None,
        headers={"Authorization": f"Bearer {token()}", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req) as r:
            b = r.read()
            return json.loads(b) if b else {}
    except urllib.error.HTTPError as e:
        detail = e.read().decode()[:300]
        # Already-in-group / already-submitted read as success for our purpose.
        if e.code == 409 and ("ALREADY" in detail.upper() or "duplicate" in detail.lower()):
            return {"already": True}
        raise SystemExit(f"{method} {path} -> {e.code}: {detail}")


def main():
    version = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: tf_release.py <build-number>")
    build = None
    for _ in range(50):  # ~25 min at 30s — processing usually lands in 5-15
        builds = api(f"/v1/builds?filter[app]={APP_ID}&sort=-version&limit=10")
        for b in builds["data"]:
            if b["attributes"]["version"] == version and b["attributes"]["processingState"] == "VALID":
                build = b
                break
        if build:
            break
        time.sleep(30)
    if not build:
        sys.exit(f"build {version} never finished processing")
    bid = build["id"]
    api(f"/v1/betaGroups/{PUBLIC_GROUP_ID}/relationships/builds", "POST",
        {"data": [{"type": "builds", "id": bid}]})
    api("/v1/betaAppReviewSubmissions", "POST",
        {"data": {"type": "betaAppReviewSubmissions",
                  "relationships": {"build": {"data": {"type": "builds", "id": bid}}}}})
    print(f"build {version} ({bid}) added to the public group and submitted for beta review")


if __name__ == "__main__":
    main()
