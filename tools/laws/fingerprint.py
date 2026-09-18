#!/usr/bin/env python3
"""Fingerprint a published document exactly the way the reading room checks it.

    python3 tools/laws/fingerprint.py data/laws/charter.v1.json          # verify: OK / MISMATCH
    python3 tools/laws/fingerprint.py data/laws/charter.v1.json --write  # stamp the fingerprint in

The fingerprint is `sha256:` + SHA-256 over the canonical JSON of the document with the
`fingerprint` key removed: keys sorted, separators `(",", ":")`, `ensure_ascii=False`,
UTF-8. This is the same canonicalisation vps/publish-constitution.sh refuses to publish
without, and the same one a reader anywhere can run against a copy. One rule, one place,
so a document can never be stamped by one rule and checked by another.
"""

import hashlib
import json
import sys
from pathlib import Path


def canonical(doc: dict) -> str:
    body = {k: v for k, v in doc.items() if k != "fingerprint"}
    return json.dumps(body, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def fingerprint(doc: dict) -> str:
    return "sha256:" + hashlib.sha256(canonical(doc).encode("utf-8")).hexdigest()


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[1] in ("-h", "--help"):
        print(__doc__.strip())
        return 2
    path = Path(argv[1])
    write = "--write" in argv[2:]
    doc = json.loads(path.read_text(encoding="utf-8"))
    actual = fingerprint(doc)
    claimed = doc.get("fingerprint")
    if write:
        if claimed == actual:
            print(f"unchanged {actual}")
            return 0
        # `fingerprint` first, so the stamp is the first line a reader sees.
        stamped = {"fingerprint": actual}
        stamped.update({k: v for k, v in doc.items() if k != "fingerprint"})
        path.write_text(json.dumps(stamped, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"stamped {actual}")
        return 0
    if claimed == actual:
        print(f"OK {actual}")
        return 0
    print(f"MISMATCH\n  claimed {claimed}\n  actual  {actual}")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
