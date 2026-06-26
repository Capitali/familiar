#!/bin/sh
# call_llm.sh — Substrate LLM adapter (the periphery seam), multi-provider.
#
# This is a REFERENCE script. It is never invoked unless a human has opened the
# capability boundary (boundary.json: "allow_llm": true) — the obedience guard
# refuses an LLM consult under the default-closed boundary, so nothing here runs by
# default. To enable Phase 1, a human copies this script (and a key.env) into the
# data dir's llm/ folder and flips the boundary. See docs/boundaries.md.
#
# Reads:  $SCRIPT_DIR/prompt.txt
# Writes: $SCRIPT_DIR/response.json
#
#   SUBSTRATE_LLM_PROVIDER   provider chain, comma-separated   (default: openrouter,gemini,cerebras)
#
# Keys (per provider; each falls back to SUBSTRATE_LLM_API_KEY):
#   OPENROUTER_API_KEY       https://openrouter.ai/keys  (OpenAI-compatible)
#   GEMINI_API_KEY           https://aistudio.google.com/apikey
#   CEREBRAS_API_KEY         https://cloud.cerebras.ai
# Models (optional): OPENROUTER_MODEL (default openai/gpt-4o-mini),
#                    GEMINI_MODEL (default gemini-2.5-flash), CEREBRAS_MODEL (default gpt-oss-120b)
#
# Secrets: if $SCRIPT_DIR/key.env exists it is sourced first (it matches *.env in
# .gitignore, so a real key.env can never be committed).
#
# On HTTP 429 the adapter records the provider's retry delay and fails over; if every
# provider is rate-limited it prints the soonest retry and exits 2 (hard failure: 1).

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROMPT_FILE="$SCRIPT_DIR/prompt.txt"

if [ -f "$SCRIPT_DIR/key.env" ]; then
    . "$SCRIPT_DIR/key.env"
fi

if [ ! -f "$PROMPT_FILE" ]; then
    echo "error: prompt.txt not found at $PROMPT_FILE" >&2
    exit 1
fi

PROVIDERS="${SUBSTRATE_LLM_PROVIDER:-openrouter,gemini,cerebras}"

python3 - "$SCRIPT_DIR" "$PROVIDERS" <<'PYEOF'
import os, sys, json, re, socket, urllib.request, urllib.error

# Prefer IPv4: some networks advertise IPv6 that silently blackholes, and Python's urllib
# has no Happy-Eyeballs fallback, so it would hang on the dead AAAA address (curl avoids
# this). Order IPv4 addresses first so HTTPS connects immediately; IPv6 stays as a fallback.
_gai = socket.getaddrinfo
socket.getaddrinfo = lambda *a, **k: sorted(_gai(*a, **k), key=lambda ai: ai[0] != socket.AF_INET)

script_dir, providers = sys.argv[1], sys.argv[2]
prompt_path   = os.path.join(script_dir, "prompt.txt")
response_path = os.path.join(script_dir, "response.json")

with open(prompt_path) as f:
    prompt_text = f.read()

shared_key = os.environ.get("SUBSTRATE_LLM_API_KEY", "")


def strip_fences(text):
    text = text.strip()
    if not text.startswith("```"):
        return text
    lines = text.split("\n")
    end = len(lines) - 1
    while end > 0 and lines[end].strip() == "":
        end -= 1
    lines = lines[1:end] if lines[end].strip() == "```" else lines[1:]
    return "\n".join(lines).strip()


def post(url, payload, headers):
    req = urllib.request.Request(
        url, data=json.dumps(payload).encode(),
        headers={"content-type": "application/json",
                 "user-agent": "substrate/2.0", **headers},
        method="POST")
    with urllib.request.urlopen(req, timeout=90) as resp:
        return json.loads(resp.read())


def call_gemini():
    key = os.environ.get("GEMINI_API_KEY") or shared_key
    if not key:
        raise RuntimeError("no GEMINI_API_KEY (or SUBSTRATE_LLM_API_KEY)")
    model = os.environ.get("GEMINI_MODEL", "gemini-2.5-flash")
    url = (f"https://generativelanguage.googleapis.com/v1beta/models/"
           f"{model}:generateContent?key={key}")
    payload = {
        "contents": [{"parts": [{"text": prompt_text}]}],
        "generationConfig": {"response_mime_type": "application/json",
                             "maxOutputTokens": 4096,
                             "thinkingConfig": {"thinkingBudget": 0}},
    }
    body = post(url, payload, {})
    return body["candidates"][0]["content"]["parts"][0]["text"]


def call_cerebras():
    key = os.environ.get("CEREBRAS_API_KEY") or shared_key
    if not key:
        raise RuntimeError("no CEREBRAS_API_KEY (or SUBSTRATE_LLM_API_KEY)")
    model = os.environ.get("CEREBRAS_MODEL", "gpt-oss-120b")
    payload = {
        "model": model,
        "max_tokens": 2048,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "user", "content": prompt_text}],
    }
    body = post("https://api.cerebras.ai/v1/chat/completions", payload,
                {"authorization": f"Bearer {key}"})
    return body["choices"][0]["message"]["content"]


def call_openrouter():
    key = os.environ.get("OPENROUTER_API_KEY") or shared_key
    if not key:
        raise RuntimeError("no OPENROUTER_API_KEY (or SUBSTRATE_LLM_API_KEY)")
    model = os.environ.get("OPENROUTER_MODEL", "openai/gpt-4o-mini")
    payload = {
        "model": model,
        "max_tokens": 2048,
        "response_format": {"type": "json_object"},
        "messages": [{"role": "user", "content": prompt_text}],
    }
    # OpenAI-compatible Chat Completions endpoint. The optional Referer/Title headers
    # let OpenRouter attribute the call; they carry no secret.
    body = post("https://openrouter.ai/api/v1/chat/completions", payload,
                {"authorization": f"Bearer {key}",
                 "http-referer": "https://github.com/Capitali/familiar",
                 "x-title": "The Familiar"})
    return body["choices"][0]["message"]["content"]


PROVIDERS = {"openrouter": call_openrouter, "gemini": call_gemini, "cerebras": call_cerebras}


def parse_retry(e):
    ra = e.headers.get("Retry-After") if e.headers else None
    if ra and ra.strip().isdigit():
        return int(ra.strip())
    try:
        body = e.read().decode(errors="replace")
    except Exception:
        body = ""
    m = re.search(r'"retryDelay"\s*:\s*"?(\d+)', body)
    return int(m.group(1)) if m else None


errors = []
retry_after = []
for name in [p.strip() for p in providers.split(",") if p.strip()]:
    fn = PROVIDERS.get(name)
    if not fn:
        errors.append(f"{name}: unknown provider")
        continue
    try:
        text = strip_fences(fn())
        json.loads(text)
        with open(response_path, "w") as f:
            f.write(text)
        print(f"LLM response via {name} ({len(text)} bytes)", file=sys.stderr)
        sys.exit(0)
    except urllib.error.HTTPError as e:
        if e.code == 429:
            wait = parse_retry(e)
            retry_after.append(wait)
            when = f", retry in {wait}s" if wait is not None else ""
            errors.append(f"{name}: rate-limited (429){when}")
        else:
            try:
                detail = e.read().decode(errors="replace")[:200]
            except Exception:
                detail = ""
            errors.append(f"{name}: HTTP {e.code} {detail}")
    except Exception as e:  # noqa: BLE001
        errors.append(f"{name}: {e}")

all_limited = bool(retry_after) and len(retry_after) == len(errors)
if all_limited:
    waits = [s for s in retry_after if s is not None]
    print("all providers rate-limited; soonest retry in "
          + (f"{min(waits)}s" if waits else "unknown"), file=sys.stderr)
print("all providers failed:\n  " + "\n  ".join(errors), file=sys.stderr)
sys.exit(2 if all_limited else 1)
PYEOF
