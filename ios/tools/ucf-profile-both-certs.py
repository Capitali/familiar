#!/usr/bin/env python3
"""Recreate the UCF Familiar App Store profile carrying BOTH of the team's distribution
certificates (MacOnStick 27FF5K2QAM + wildhorse 8PV78GZUUJ) and install it on this Mac —
the 2026-08-13 fix for the Familiar profiles, applied to io.river.familiar.ucf.
Run from the repo root: python3 ios/tools/ucf-profile-both-certs.py
Then copy the printed .mobileprovision to the other Mac's
~/Library/Developer/Xcode/UserData/Provisioning Profiles/ so its next export uses it too
(the old profile is invalidated by the delete)."""
import sys, base64, urllib.parse, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__))); sys.argv = ["x"]
import tf_release as t
NAME = "UCF Familiar AppStore io.river.familiar.ucf"
CERTS = ["27FF5K2QAM", "8PV78GZUUJ"]
b = t.api("/v1/bundleIds?filter[identifier]=io.river.familiar.ucf")["data"]
assert len(b) == 1, b
for p in t.api("/v1/profiles?filter[name]=" + urllib.parse.quote(NAME))["data"]:
    t.api(f"/v1/profiles/{p['id']}", method="DELETE"); print("deleted", p["id"])
new = t.api("/v1/profiles", method="POST", body={"data": {"type": "profiles",
    "attributes": {"name": NAME, "profileType": "IOS_APP_STORE"},
    "relationships": {"bundleId": {"data": {"type": "bundleIds", "id": b[0]["id"]}},
                      "certificates": {"data": [{"type": "certificates", "id": c} for c in CERTS]}}}})
a = new["data"]["attributes"]; raw = base64.b64decode(a["profileContent"])
for d in ["~/Library/Developer/Xcode/UserData/Provisioning Profiles", "~/Library/MobileDevice/Provisioning Profiles"]:
    d = os.path.expanduser(d); os.makedirs(d, exist_ok=True)
    path = os.path.join(d, a["uuid"] + ".mobileprovision"); open(path, "wb").write(raw); print("installed", path)
print("created", new["data"]["id"], a["profileState"], "expires", a["expirationDate"])
