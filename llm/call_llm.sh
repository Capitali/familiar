#!/bin/sh
# call_llm.sh — Substrate LLM adapter (the periphery seam), multi-provider + resilient.
#
# This is a REFERENCE script. It is never invoked unless a human has opened the
# capability boundary (boundary.json: "allow_llm": true) — the obedience guard refuses an
# LLM consult under the default-closed boundary, so nothing here runs by default.
#
# Modes:
#   (default) consult — read prompt.txt, answer via the first healthy provider, write response.json
#   probe             — ping every configured provider with a tiny request and refresh health.json
#                       (the occasional availability check; does not touch response.json)
#
# Reads:  $SCRIPT_DIR/prompt.txt        (consult mode)
# Writes: $SCRIPT_DIR/response.json     (consult mode, on success)
#         $SCRIPT_DIR/health.json       (always — per-provider status the system can surface)
#
#   SUBSTRATE_LLM_PROVIDER   provider chain, comma-separated   (default: gemini,cerebras)
#                            "ollama" = a local model server (no key, no rate limits);
#                            OLLAMA_MODEL (default mistral), OLLAMA_HOST (default
#                            http://127.0.0.1:11434)
#
# Keys (per provider; each falls back to SUBSTRATE_LLM_API_KEY):
#   ANTHROPIC_API_KEY        https://console.anthropic.com (provider name: claude)
#   GEMINI_API_KEY           https://aistudio.google.com/apikey
#   CEREBRAS_API_KEY         https://cloud.cerebras.ai
# Models (optional): ANTHROPIC_MODEL (default claude-haiku-4-5-20251001),
#   GEMINI_MODEL (default gemini-2.5-flash), CEREBRAS_MODEL (default gpt-oss-120b)
#
# Spend governor (self-imposed, enforced HERE — independent of any console limit):
# a per-provider daily ledger in $SCRIPT_DIR/spend.json. When a provider's budget is
# reached it is put in cooldown until UTC midnight and the chain rolls to the next.
#   <PROVIDER>_DAILY_TOKEN_BUDGET / <PROVIDER>_DAILY_CALL_BUDGET  (e.g. CLAUDE_...)
# The paid provider (claude) defaults to 200000 tokens / 300 calls per day even if
# unset; free-tier providers have no default budget. Set a budget to 0 to disable a
# provider outright.
#
# Resilience: each provider is tried in turn; a failure is recorded in health.json with a
# reason and a cooldown (`available_after`). Providers in cooldown are deprioritised, so the
# next consult rolls straight to a healthy one instead of re-hitting a dead one. On HTTP 402
# (out of credits / too many tokens) the provider is retried once with a budget that fits.
# Exit 0 = answered; 2 = every provider rate-limited; 1 = otherwise failed.
#
# Secrets: if $SCRIPT_DIR/key.env exists it is sourced first (it matches *.env in .gitignore).

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:-consult}"

if [ -f "$SCRIPT_DIR/key.env" ]; then
    . "$SCRIPT_DIR/key.env"
fi

if [ "$MODE" = "consult" ] && [ ! -f "$SCRIPT_DIR/prompt.txt" ]; then
    echo "error: prompt.txt not found at $SCRIPT_DIR/prompt.txt" >&2
    exit 1
fi

PROVIDERS="${SUBSTRATE_LLM_PROVIDER:-gemini,cerebras}"

python3 - "$SCRIPT_DIR" "$PROVIDERS" "$MODE" <<'PYEOF'
import os, sys, json, re, time, socket, urllib.request, urllib.error

# Prefer IPv4: some networks advertise IPv6 that silently blackholes, and Python's urllib
# has no Happy-Eyeballs fallback, so it would hang on the dead AAAA address (curl avoids
# this). Order IPv4 addresses first so HTTPS connects immediately; IPv6 stays as a fallback.
_gai = socket.getaddrinfo
socket.getaddrinfo = lambda *a, **k: sorted(_gai(*a, **k), key=lambda ai: ai[0] != socket.AF_INET)

script_dir, providers_str, mode = sys.argv[1], sys.argv[2], sys.argv[3]
prompt_path = os.path.join(script_dir, "prompt.txt")
response_path = os.path.join(script_dir, "response.json")
health_path = os.path.join(script_dir, "health.json")
spend_path = os.path.join(script_dir, "spend.json")
now = int(time.time())
today = time.strftime("%Y-%m-%d", time.gmtime(now))

if mode == "consult":
    with open(prompt_path) as f:
        prompt_text = f.read()
else:  # probe — a tiny, cheap request just to learn who's alive
    prompt_text = 'Reply only with this exact JSON and nothing else: {"ok": true}'

shared_key = os.environ.get("SUBSTRATE_LLM_API_KEY", "")
DEFAULT_MAX_TOKENS = 2048
# A floor below which a credit-starved provider isn't worth retrying.
MIN_TOKENS = 256
# Cooldowns (seconds) — how long to deprioritise a provider after a given failure.
COOL_CREDITS = 3600   # out of credits won't refill soon
COOL_ERROR = 600      # transient/unknown error
COOL_RATELIMIT = 300  # default when no Retry-After is given


def load_health():
    try:
        with open(health_path) as f:
            return json.load(f)
    except Exception:
        return {}


def save_health(h):
    try:
        with open(health_path, "w") as f:
            json.dump(h, f, indent=2)
    except Exception:
        pass


# ---- the spend governor: a self-imposed daily budget, enforced locally ---------------
# The ledger survives in spend.json; a provider over budget raises BudgetReached and is
# cooled until UTC midnight. This is the human-owned cost boundary made local — no
# remote console required for the cap to hold.

class BudgetReached(Exception):
    pass


def load_spend():
    try:
        with open(spend_path) as f:
            return json.load(f)
    except Exception:
        return {}


def budget_of(name):
    tok = os.environ.get(f"{name.upper()}_DAILY_TOKEN_BUDGET")
    calls = os.environ.get(f"{name.upper()}_DAILY_CALL_BUDGET")
    if tok is None and calls is None and name == "claude":
        tok, calls = "200000", "300"  # the paid provider is never uncapped by default
    return (int(tok) if tok is not None else None,
            int(calls) if calls is not None else None)


def spend_guard(name):
    tok_budget, call_budget = budget_of(name)
    if tok_budget is None and call_budget is None:
        return
    s = load_spend().get(today, {}).get(name, {"calls": 0, "tokens": 0})
    if (tok_budget is not None and s["tokens"] >= tok_budget) or (
        call_budget is not None and s["calls"] >= call_budget
    ):
        raise BudgetReached(
            f"self-imposed daily budget reached "
            f"({s['calls']} calls, {s['tokens']} tokens today)"
        )


def spend_record(name, tokens):
    sp = load_spend()
    entry = sp.setdefault(today, {}).setdefault(name, {"calls": 0, "tokens": 0})
    entry["calls"] += 1
    entry["tokens"] += int(tokens)
    cutoff = time.strftime("%Y-%m-%d", time.gmtime(now - 7 * 86400))
    for day in [d for d in sp if d < cutoff]:  # keep a week of ledger for the human
        del sp[day]
    try:
        with open(spend_path, "w") as f:
            json.dump(sp, f, indent=2)
    except Exception:
        pass


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
                 "user-agent": "substrate/2.1", **headers},
        method="POST")
    with urllib.request.urlopen(req, timeout=90) as resp:
        return json.loads(resp.read())


def call_gemini(max_tokens):
    key = os.environ.get("GEMINI_API_KEY") or shared_key
    if not key:
        raise RuntimeError("no GEMINI_API_KEY (or SUBSTRATE_LLM_API_KEY)")
    model = os.environ.get("GEMINI_MODEL", "gemini-2.5-flash")
    url = (f"https://generativelanguage.googleapis.com/v1beta/models/"
           f"{model}:generateContent?key={key}")
    payload = {
        "contents": [{"parts": [{"text": prompt_text}]}],
        "generationConfig": {"response_mime_type": "application/json",
                             "maxOutputTokens": max_tokens,
                             "thinkingConfig": {"thinkingBudget": 0}},
    }
    body = post(url, payload, {})
    return body["candidates"][0]["content"]["parts"][0]["text"]


def call_cerebras(max_tokens):
    key = os.environ.get("CEREBRAS_API_KEY") or shared_key
    if not key:
        raise RuntimeError("no CEREBRAS_API_KEY (or SUBSTRATE_LLM_API_KEY)")
    model = os.environ.get("CEREBRAS_MODEL", "gpt-oss-120b")
    payload = {
        "model": model,
        "max_tokens": max_tokens,
        # gpt-oss is a reasoning model: its reasoning tokens count against max_tokens.
        # Left unbounded it spends the whole budget thinking and the JSON content is
        # truncated mid-string ("Unterminated string ..." on json.loads). The factory
        # only needs a concrete JSON answer, not deep reasoning, so cap the effort low —
        # this drops reasoning from ~500 tokens to ~60 and leaves the budget for content.
        "reasoning_effort": os.environ.get("CEREBRAS_REASONING_EFFORT", "low"),
        "response_format": {"type": "json_object"},
        "messages": [{"role": "user", "content": prompt_text}],
    }
    body = post("https://api.cerebras.ai/v1/chat/completions", payload,
                {"authorization": f"Bearer {key}"})
    return body["choices"][0]["message"]["content"]


def call_claude(max_tokens):
    key = os.environ.get("ANTHROPIC_API_KEY") or shared_key
    if not key:
        raise RuntimeError("no ANTHROPIC_API_KEY (or SUBSTRATE_LLM_API_KEY)")
    spend_guard("claude")
    model = os.environ.get("ANTHROPIC_MODEL", "claude-haiku-4-5-20251001")
    payload = {
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{"role": "user", "content": prompt_text}],
    }
    body = post("https://api.anthropic.com/v1/messages", payload,
                {"x-api-key": key, "anthropic-version": "2023-06-01"})
    usage = body.get("usage", {})
    spend_record("claude",
                 usage.get("input_tokens", 0) + usage.get("output_tokens", 0))
    return body["content"][0]["text"]


def call_ollama(max_tokens):
    # A local model server — no key, no network reach beyond loopback, no
    # rate limits: the provider a long unattended campaign leans on.
    host = os.environ.get("OLLAMA_HOST", "http://127.0.0.1:11434")
    model = os.environ.get("OLLAMA_MODEL", "mistral")
    payload = {
        "model": model,
        "stream": False,
        "format": "json",  # the seam convention is compact JSON — enforce it
        "keep_alive": "60m",  # stay resident between consults of a campaign
        "options": {"num_predict": max_tokens, "temperature": 0},
        "messages": [{"role": "user", "content": prompt_text}],
    }
    body = post(f"{host}/api/chat", payload, {})
    spend_record("ollama",
                 body.get("prompt_eval_count", 0) + body.get("eval_count", 0))
    return body["message"]["content"]


PROVIDERS = {"claude": call_claude, "anthropic": call_claude,
             "gemini": call_gemini, "cerebras": call_cerebras,
             "ollama": call_ollama}


def http_detail(e):
    """(retry_after_secs|None, affordable_tokens|None, short_body) from an HTTPError."""
    ra = e.headers.get("Retry-After") if e.headers else None
    retry = int(ra.strip()) if ra and ra.strip().isdigit() else None
    try:
        body = e.read().decode(errors="replace")
    except Exception:
        body = ""
    if retry is None:
        m = re.search(r'"retryDelay"\s*:\s*"?(\d+)', body)
        retry = int(m.group(1)) if m else None
    m = re.search(r'afford (\d+)', body)
    afford = int(m.group(1)) if m else None
    return retry, afford, body[:200]


def mark(health, name, status, detail, cool):
    health[name] = {"status": status, "detail": detail, "ts": now,
                    "available_after": now + cool}


def succeed(health, name):
    health[name] = {"status": "ok", "detail": "", "ts": now, "available_after": 0}


health = load_health()
configured = [p.strip() for p in providers_str.split(",") if p.strip()]

# Order: providers not in cooldown first, then those last seen healthy, otherwise the
# configured order (stable sort). This is the quick rollover — a dead provider sinks.
def rank(p):
    h = health.get(p, {})
    cooling = 1 if h.get("available_after", 0) > now else 0
    not_ok = 0 if h.get("status") == "ok" else 1
    return (cooling, not_ok)

order = configured if mode == "probe" else sorted(configured, key=rank)

errors = []
rate_limited = []
answered = False

for name in order:
    fn = PROVIDERS.get(name)
    if not fn:
        errors.append(f"{name}: unknown provider")
        continue
    try:
        text = strip_fences(fn(DEFAULT_MAX_TOKENS))
        json.loads(text)
        succeed(health, name)
        if mode == "consult":
            with open(response_path, "w") as f:
                f.write(text)
            save_health(health)
            print(f"LLM response via {name} ({len(text)} bytes)", file=sys.stderr)
            sys.exit(0)
        answered = True  # probe: keep going to refresh every provider
    except BudgetReached as e:
        # The human's own cost boundary — cool until UTC midnight, roll to the next
        # provider. Not an error: the cap holding is the feature.
        mark(health, name, "budget", str(e), 86400 - (now % 86400))
        errors.append(f"{name}: {e}")
    except urllib.error.HTTPError as e:
        retry, afford, body = http_detail(e)
        if e.code == 429:
            wait = retry or COOL_RATELIMIT
            mark(health, name, "rate_limited", f"rate-limited (429)", wait)
            rate_limited.append(name)
            errors.append(f"{name}: rate-limited (429), retry in {wait}s")
        elif e.code == 402:
            # out of credits / too many tokens — retry once within the affordable budget
            if mode == "consult" and afford and afford > MIN_TOKENS:
                try:
                    text = strip_fences(fn(max(MIN_TOKENS, afford - 128)))
                    json.loads(text)
                    succeed(health, name)
                    with open(response_path, "w") as f:
                        f.write(text)
                    save_health(health)
                    print(f"LLM response via {name} (reduced to fit credits)", file=sys.stderr)
                    sys.exit(0)
                except Exception:
                    pass
            mark(health, name, "error", "insufficient credits (HTTP 402)", COOL_CREDITS)
            errors.append(f"{name}: out of credits (402){'' if not afford else f', affords {afford}'}")
        else:
            mark(health, name, "error", f"HTTP {e.code} {body}", COOL_ERROR)
            errors.append(f"{name}: HTTP {e.code} {body}")
    except Exception as e:  # noqa: BLE001
        mark(health, name, "error", str(e)[:200], COOL_ERROR)
        errors.append(f"{name}: {e}")

save_health(health)

if mode == "probe":
    alive = [p for p in configured if health.get(p, {}).get("status") == "ok"]
    print(f"probe: healthy = {alive or 'none'}", file=sys.stderr)
    sys.exit(0 if alive else 1)

all_limited = bool(rate_limited) and len(rate_limited) == len(errors)
if all_limited:
    print("all providers rate-limited", file=sys.stderr)
print("all providers failed:\n  " + "\n  ".join(errors), file=sys.stderr)
sys.exit(2 if all_limited else 1)
PYEOF
