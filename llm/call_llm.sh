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

# A caller's explicit provider choice wins over key.env — capture it before sourcing so a hard-set
# `export SUBSTRATE_LLM_PROVIDER=…` in key.env can't clobber `SUBSTRATE_LLM_PROVIDER=apple <cmd>`
# (the device-oracle path, a lab campaign pinning one provider, etc.).
_CALLER_PROVIDER="${SUBSTRATE_LLM_PROVIDER:-}"
# The boundary's cloud decision (ADR-0038) likewise wins over key.env: captured before
# sourcing so a stray export in key.env can never widen it. Unset means closed — a human
# running the adapter by hand gets the fail-closed default; the seam always sets it.
_ALLOW_CLOUD="${FAMILIAR_ALLOW_LLM_CLOUD:-0}"
if [ -f "$SCRIPT_DIR/key.env" ]; then
    . "$SCRIPT_DIR/key.env"
fi
[ -n "$_CALLER_PROVIDER" ] && SUBSTRATE_LLM_PROVIDER="$_CALLER_PROVIDER"
FAMILIAR_ALLOW_LLM_CLOUD="$_ALLOW_CLOUD"

if [ "$MODE" = "consult" ] && [ ! -f "$SCRIPT_DIR/prompt.txt" ]; then
    echo "error: prompt.txt not found at $SCRIPT_DIR/prompt.txt" >&2
    exit 1
fi

PROVIDERS="${SUBSTRATE_LLM_PROVIDER:-gemini,cerebras}"

python3 - "$SCRIPT_DIR" "$PROVIDERS" "$MODE" "$FAMILIAR_ALLOW_LLM_CLOUD" <<'PYEOF'
import os, sys, json, re, time, socket, urllib.request, urllib.error

# Prefer IPv4: some networks advertise IPv6 that silently blackholes, and Python's urllib
# has no Happy-Eyeballs fallback, so it would hang on the dead AAAA address (curl avoids
# this). Order IPv4 addresses first so HTTPS connects immediately; IPv6 stays as a fallback.
_gai = socket.getaddrinfo
socket.getaddrinfo = lambda *a, **k: sorted(_gai(*a, **k), key=lambda ai: ai[0] != socket.AF_INET)

script_dir, providers_str, mode = sys.argv[1], sys.argv[2], sys.argv[3]
cloud_ok = len(sys.argv) > 4 and sys.argv[4] == "1"
# What shape of answer this consult wants (T-192). "json" unless told otherwise, so a
# hand-run script keeps the strict contract the metabolism depends on.
expect = os.environ.get("FAMILIAR_EXPECT", "json")
want_prose = expect == "prose"
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
        # Same as cerebras: an application/json mime type makes prose impossible (T-192).
        "generationConfig": {**({} if want_prose
                                else {"response_mime_type": "application/json"}),
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
        "messages": [{"role": "user", "content": prompt_text}],
    }
    # Forcing json_object made a PROSE consult impossible: asked for "plain text only, no
    # quotes, no JSON", the API constraint still compelled JSON and the model emitted
    # `{"type":"object"}` — which the dialogue then rejected as junk and answered with a
    # stock acknowledgement. The familiar could never hold a conversation here (T-192).
    if not want_prose:
        payload["response_format"] = {"type": "json_object"}
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
    # CPU-only hosts generate slowly and the factory's answers are short: cap
    # output well below the network providers' budget (overridable), and give
    # the local server a longer deadline than the 90s network timeout — a
    # cold model load alone can eat a minute on an Intel host.
    cap = int(os.environ.get("OLLAMA_NUM_PREDICT", "700"))
    deadline = int(os.environ.get("OLLAMA_TIMEOUT", "240"))
    # NOT format:"json": grammar-forced JSON makes a small model's first
    # unescaped quote inside a script close the string and truncate the code.
    # Let it answer naturally; below, a non-JSON answer that is plainly a
    # script gets wrapped into the seam convention with real escaping.
    payload = {
        "model": model,
        "stream": False,
        "keep_alive": "60m",  # stay resident between consults of a campaign
        "options": {"num_predict": min(max_tokens, cap), "temperature": 0},
        "messages": [{"role": "user", "content": prompt_text}],
    }
    req = urllib.request.Request(
        f"{host}/api/chat", data=json.dumps(payload).encode(),
        headers={"content-type": "application/json",
                 "user-agent": "substrate/2.1"}, method="POST")
    with urllib.request.urlopen(req, timeout=deadline) as resp:
        body = json.loads(resp.read())
    spend_record("ollama",
                 body.get("prompt_eval_count", 0) + body.get("eval_count", 0))
    text = body["message"]["content"]
    try:
        json.loads(strip_fences(text))
        return text
    except Exception:
        return json.dumps({"script": strip_fences(text)})


def usable_json(text):
    """Return the device's answer as parseable JSON, or treat it as silence.

    Guided generation (`kind` of "script"/"theory") emits exact JSON, but the muse asks other
    shapes too and those still run free-form, where a small on-device model sometimes produces
    almost-JSON — a missing comma, prose wrapped around the object. Left alone that surfaces as a
    plain exception, which the provider chain classifies as an *error* rather than a rate limit:
    exit 1 instead of 2, so the lab stops the whole campaign, and `apple` is parked under an error
    cooldown so later cells skip the device entirely. One bad sentence should not cost a 168-cell
    run.

    So: unwrap fences, and failing that take the outermost {...}, which recovers an object the
    model buried in commentary. If it still will not parse, the device produced nothing usable,
    which is the same evidentiary case as a device that never answered — DeviceAsleep, a pause the
    muse retries, never a fabricated answer (ADR-0014).
    """
    text = strip_fences(text or "")
    try:
        json.loads(text)
        return text
    except Exception:
        pass
    start, end = text.find("{"), text.rfind("}")
    if start != -1 and end > start:
        inner = text[start : end + 1]
        try:
            json.loads(inner)
            return inner
        except Exception:
            pass
    raise DeviceAsleep("device answered with unparseable JSON")


class DeviceAsleep(Exception):
    """The apple provider queued a prompt but no member device answered in time. Treated as a
    rate-limit (exit 2) so the muse retries later and the lab records llm_unavailable, rather
    than contaminating evidence with a template answer (ADR-0014)."""
    pass


def call_apple(max_tokens):
    # The device oracle (ADR-0014): queue the prompt for a member device's on-device model
    # (Apple Intelligence) to answer over the /mesh/consult seam, then poll for the answer file.
    # Nothing but the answer comes back; a sleeping device is silence, not garbage.
    import uuid
    # The device answers over the mesh from ONE daemon's queue (the main hub Codex pulls from). A
    # scenario episode runs the adapter from its OWN copied data dir, so its default `device-queue`
    # is private and no device ever sees it. APPLE_QUEUE_DIR (set in the shared key.env, carried into
    # every episode copy) funnels all apple consults to the hub's queue so the enrolled device can
    # actually answer them. Falls back to the local dir when unset (e.g. the daemon's own muse).
    qdir = os.environ.get("APPLE_QUEUE_DIR", "").strip() or os.path.join(script_dir, "device-queue")
    os.makedirs(qdir, exist_ok=True)
    cid = "c-" + uuid.uuid4().hex[:12]
    prompt_file = os.path.join(qdir, cid + ".prompt.json")
    answer_file = os.path.join(qdir, cid + ".answer.json")
    # Pick the device's guided-generation strategy (ADR-0014). An explicit APPLE_CONSULT_KIND wins
    # (the eval harness sweeps strategies this way); otherwise infer from the JSON shape the muse asked
    # for. Guided generation guarantees valid JSON and tends not to hang the way free-form does on the
    # open-ended "write a script" prompts that stalled the A9 treatment cells.
    kind = os.environ.get("APPLE_CONSULT_KIND", "")
    if not kind:
        low = prompt_text.lower()
        if '"script"' in low:
            kind = "script"
        elif '"theory"' in low and '"direction"' in low:
            kind = "theory"
    with open(prompt_file, "w") as f:
        # cloud_ok: the boundary's ADR-0038 decision rides to the answering device, which
        # stacks its own consent on top before ever choosing Private Cloud Compute.
        json.dump({"id": cid, "prompt": prompt_text, "ts": int(time.time()), "kind": kind,
                   "cloud_ok": cloud_ok}, f)
    timeout = int(os.environ.get("APPLE_CONSULT_TIMEOUT", "90"))
    deadline = time.time() + timeout
    while time.time() < deadline:
        if os.path.exists(answer_file):
            with open(answer_file) as f:
                ans = json.load(f)
            try:
                os.remove(answer_file)
            except Exception:
                pass
            return usable_json(ans.get("json", ""))
        time.sleep(2)
    try:
        os.remove(prompt_file)  # don't leave an unanswered prompt lingering
    except Exception:
        pass
    raise DeviceAsleep(f"no device answered within {timeout}s")


def call_fm(name, model_flag):
    # The host's own Apple Intelligence via the macOS 27 `fm` CLI — `system` runs on this
    # machine's silicon (a LOCAL provider, ADR-0038); `pcc` sends to Apple's Private Cloud
    # Compute (a CLOUD provider, gated). `--schema` is fm's guided generation: the answer
    # comes back as exactly the JSON shape the muse asked for, so usable_json never guesses.
    import shutil, subprocess
    if not shutil.which("fm"):
        raise RuntimeError(f"{name}: fm CLI not present (macOS 27+)")
    avail = subprocess.run(["fm", "available", "--model", model_flag],
                           capture_output=True, text=True, timeout=15)
    if avail.returncode == 69:
        raise RuntimeError(f"{name}: fm license not agreed — run: sudo fm license")
    if avail.returncode != 0 or "available" not in avail.stdout.lower():
        # Model not ready / Apple Intelligence off / PCC unreachable: the same evidentiary
        # case as a sleeping device — transient silence the muse retries, never garbage.
        raise DeviceAsleep(f"{name}: {(avail.stdout or avail.stderr).strip()[:160]}")
    kind = os.environ.get("APPLE_CONSULT_KIND", "")
    if not kind:
        low = prompt_text.lower()
        if '"script"' in low:
            kind = "script"
        elif '"theory"' in low and '"direction"' in low:
            kind = "theory"
    args = ["fm", "respond", "--model", model_flag, "--no-stream"]
    if kind == "script":
        args += ["--schema",
                 '{"type":"object","properties":{"script":{"type":"string"}},"required":["script"]}']
    elif kind == "theory":
        args += ["--schema",
                 '{"type":"object","properties":{"question":{"type":"string"},'
                 '"theory":{"type":"string"},"direction":{"type":"string"}},'
                 '"required":["question","theory","direction"]}']
    args.append(prompt_text)
    deadline = int(os.environ.get("FM_TIMEOUT", "100"))  # under the seam's 120s kill
    try:
        r = subprocess.run(args, capture_output=True, text=True, timeout=deadline)
    except subprocess.TimeoutExpired:
        raise DeviceAsleep(f"{name}: no answer within {deadline}s")
    if r.returncode != 0:
        raise RuntimeError(f"{name}: fm exit {r.returncode}: {(r.stderr or '').strip()[:200]}")
    text = r.stdout.strip()
    # fm reports no token usage; estimate for the ledger so budgets still bind.
    spend_record(name, max(1, (len(prompt_text) + len(text)) // 4))
    return text if kind in ("script", "theory") else usable_json(text)


def call_apple_local(max_tokens):
    return call_fm("apple_local", "system")


def call_apple_pcc(max_tokens):
    return call_fm("apple_pcc", "pcc")


PROVIDERS = {"claude": call_claude, "anthropic": call_claude,
             "gemini": call_gemini, "cerebras": call_cerebras,
             "ollama": call_ollama, "apple": call_apple,
             "apple_local": call_apple_local, "apple_pcc": call_apple_pcc}


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

# ADR-0038: when the boundary closes the cloud, off-hardware providers leave the chain
# entirely — consult AND probe alike (a probe is still an outward request carrying tokens).
CLOUD_PROVIDERS = {"claude", "anthropic", "gemini", "cerebras", "apple_pcc"}
if not cloud_ok:
    dropped = [p for p in configured if p in CLOUD_PROVIDERS]
    configured = [p for p in configured if p not in CLOUD_PROVIDERS]
    if dropped and mode == "consult":
        print(f"cloud gate closed (allow_llm_cloud=false): skipping {','.join(dropped)}",
              file=sys.stderr)
if mode == "consult" and not configured:
    print("every configured provider is off-device and the boundary closes the cloud "
          "(allow_llm_cloud=false) — open it in boundary.json or configure a local "
          "provider (apple_local, apple, ollama)", file=sys.stderr)
    sys.exit(1)

# Order: providers not in cooldown first, then those last seen healthy, otherwise the
# configured order (stable sort). This is the quick rollover — a dead provider sinks.
def rank(p):
    h = health.get(p, {})
    cooling = 1 if h.get("available_after", 0) > now else 0
    # A provider with NO health record has never been tried — that is not-knowing, not a
    # known fault, and it must not be scored as one. Ranking it below every healthy
    # incumbent meant a newly configured provider could never reach the front of the chain
    # while any existing one was working, silently overriding the human's configured order:
    # adding `claude` at the head of `claude,cerebras,gemini` changed nothing, and the
    # adapter reported no error because it never called it (2026-08-15).
    # Untried ties with healthy, and `sorted` is stable, so the configured order decides.
    status = h.get("status")
    not_ok = 0 if status in ("ok", None) else 1
    return (cooling, not_ok)

order = configured if mode == "probe" else sorted(configured, key=rank)

# T-191 — presence outranks musing in QUOTA, not only in queue order.
#
# `Lane` in crates/llm already sends a waiting human to the head of the queue, but ordering is
# worthless once the metabolism has spent a free tier: the person then speaks and is refused by
# a cooldown their own familiar caused. Every 60s tick was retrying providers that had just
# said 429 — which keeps them pinned at the limit instead of letting it recover.
#
# So background thinking now STANDS DOWN from a provider that is cooling, and leaves that
# headroom for whoever is actually there. A human-lane consult still tries everything, cooling
# providers last, because a person waiting is worth the attempt.
lane = os.environ.get("FAMILIAR_LANE", "human")
if lane == "background" and mode == "consult":
    resting = [p for p in order if health.get(p, {}).get("available_after", 0) > now]
    if resting:
        order = [p for p in order if p not in resting]
        print(f"standing down from {','.join(resting)} — cooling, and the next words "
              f"spoken to the familiar have first call on it", file=sys.stderr)

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
        # T-192: only STRUCTURED consults are validated as JSON. The dialogue asks the model
        # for "plain text only, no quotes, no JSON" — validating that as JSON marked the
        # provider failed for obeying its instructions, rolled the chain on, and left the human
        # told the familiar could not reach its mind when it had reached it every time.
        if expect != "prose":
            json.loads(text)
        elif not text.strip():
            raise ValueError("empty response")
        succeed(health, name)
        if mode == "consult":
            with open(response_path, "w") as f:
                f.write(text)
            save_health(health)
            print(f"LLM response via {name} ({len(text)} bytes)", file=sys.stderr)
            sys.exit(0)
        answered = True  # probe: keep going to refresh every provider
    except DeviceAsleep as e:
        # The apple provider's device didn't answer — a rate-limit, not an error, so the
        # chain rolls on and (if nothing else answers) exits 2 → the lab pauses cleanly.
        mark(health, name, "rate_limited", str(e), COOL_RATELIMIT)
        rate_limited.append(name)
        errors.append(f"{name}: {e}")
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
