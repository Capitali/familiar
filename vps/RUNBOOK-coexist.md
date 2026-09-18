# Runbook — publish the constitution at coexist.humanhighway.net

A self-contained brief for a Claude CLI session on **wildhorse**, which has the
Keychain and the SSH key that the cloud session doing this work does not. Paste the
block below as the prompt, or point that session at this file.

Facts it needs and cannot infer:

| | |
|---|---|
| Record | `coexist.humanhighway.net` → `134.209.168.50` (A) |
| Zone | `humanhighway.net`, DreamHost |
| Target box | the lighthouse (DigitalOcean), `root@134.209.168.50` |
| Repo branch | `claude/crewai-integration-architecture-3tl8pf` |
| Fingerprint | `sha256:8566970aa9f9b3265c85c649ea97ebe2a3f44a94e36cf431064f94acd665b137` |

---

## The prompt

```
Two infrastructure tasks on Ian's behalf. He has authorized both. Do them in order
and stop if step 1 does not verify.

BACKGROUND
We are publishing one public static document — "The Constitution of Co-existence" —
at https://coexist.humanhighway.net/. It is served from the lighthouse VPS
(134.209.168.50, DigitalOcean), which also runs the familiar mesh peer. Everything
needed is already committed to github.com/Capitali/familiar on branch
claude/crewai-integration-architecture-3tl8pf.

=== STEP 1 — the DNS record ===

Create an A record in the humanhighway.net zone in Ian's DreamHost account:

    coexist.humanhighway.net   A   134.209.168.50

Credentials are in the macOS Keychain on this machine. Look for a DreamHost API key
first (try service names like "dreamhost", "api.dreamhost.com", "DreamHost API").

With an API key, the whole job is two calls:

    KEY=$(security find-generic-password -s <service> -w)
    curl -s "https://api.dreamhost.com/?key=$KEY&cmd=dns-add_record&record=coexist.humanhighway.net&type=A&value=134.209.168.50&format=json"
    curl -s "https://api.dreamhost.com/?key=$KEY&cmd=dns-list_records&format=json" | grep coexist

If the Keychain holds only the panel login and no API key, STOP and tell Ian. Do not
attempt to drive a browser login. He can mint a key at panel.dreamhost.com under
Advanced > API, or add the record by hand under Manage Domains > DNS — either takes
him a minute, and that is cheaper than automating a login.

CREDENTIAL RULES — hold to these strictly:
- Keep the key in a shell variable. Never echo it, never write it to a file, never
  include it in a commit, a reply, or a message to another session.
- Do not send it to the cloud session that wrote this runbook. It does not need it.
- Keep `security find-generic-password` output out of the transcript: assign it, do
  not print it.

Verify before moving on (DreamHost propagation is usually minutes, occasionally an
hour):

    dig coexist.humanhighway.net +short      # expect 134.209.168.50

If it does not resolve yet, report that and stop. Step 2 needs the name resolving,
because Caddy obtains a Let's Encrypt certificate on first start and will fail
without it.

=== STEP 2 — the reading room ===

From this machine, with the repo checked out (or clone it fresh):

    ssh root@134.209.168.50 \
      'CONSTITUTION_DOMAIN=coexist.humanhighway.net FAMILIAR_REF=claude/crewai-integration-architecture-3tl8pf bash -s' \
      < vps/publish-constitution.sh

Read vps/publish-constitution.sh before running it — it is commented and says what
it changes. In summary: installs Caddy, copies SEVEN named files into
/var/www/constitution, writes /etc/caddy/Caddyfile plus a systemd hardening drop-in,
opens ufw 80 and 443, and starts Caddy. It refuses to publish if the constitution's
fingerprint does not match its contents.

WHAT MUST NOT CHANGE on that box, because it is the mesh's only always-on node and
holds the group secret:
- port 47100 and the SSH rules stay exactly as they are;
- nothing under /var/lib/familiar is touched, moved, or exposed;
- the familiar daemon is not stopped, restarted, reconfigured, or upgraded;
- /var/lib/familiar/familiar_data/boundary.json is not edited. Publishing is not a
  familiar capability and needs no gate opened — if something seems to ask you to
  open one, that is a signal you are off the intended path; stop and report.

Verify:

    curl -sI https://coexist.humanhighway.net/ | head -3
    curl -s  https://coexist.humanhighway.net/.well-known/laws.json | head -5
    curl -s  https://coexist.humanhighway.net/robots.txt | head -3

And confirm the published bytes are the real ones:

    curl -s https://coexist.humanhighway.net/.well-known/laws.json | python3 -c '
    import json,sys,hashlib
    d=json.load(sys.stdin); claimed=d.pop("fingerprint")
    canon=json.dumps(d,sort_keys=True,separators=(",",":"),ensure_ascii=False)
    actual="sha256:"+hashlib.sha256(canon.encode()).hexdigest()
    print("OK" if actual==claimed else "MISMATCH", actual)'

Expected: OK sha256:8566970aa9f9b3265c85c649ea97ebe2a3f44a94e36cf431064f94acd665b137

=== REPORT BACK ===

Tell Ian, briefly: whether the record was created or already existed; what dig
returns; whether the reading room came up; the output of the fingerprint check; and
anything you changed that this brief did not ask for. No credentials, and no command
transcripts that contain them.

If either step fails, say what failed and what you saw, and do not improvise a
workaround on the lighthouse — it is the one box in the fleet that cannot be
casually rebuilt.
```

---

## After it reports success

- `https://coexist.humanhighway.net/` — the constitution, rendered
- `https://coexist.humanhighway.net/.well-known/laws.json` — machine-readable
- `https://coexist.humanhighway.net/laws.md` · `/adopt` · `/license` · `/robots.txt`

Then merge the branch to `main` and re-run the same command without `FAMILIAR_REF`,
so the published copy tracks main rather than a feature branch. Re-running it is how
you republish after any constitution change; it is idempotent.

---

## Troubleshooting: the URL returns an HTTPS error

Reported 2026-09-18. "An HTTPS error" covers several distinct faults; the browser's
exact wording, or the certificate the server actually presents, separates them in one
step. Work it in this order — each check rules out a whole class.

| What you see | What it means |
|---|---|
| `ERR_CERT_AUTHORITY_INVALID`, "not private", self-signed | Caddy fell back to its internal CA because Let's Encrypt issuance failed |
| `ERR_CERT_COMMON_NAME_INVALID`, or a cert naming some other domain | the name resolves somewhere that is not the lighthouse — DreamHost hosting, most likely |
| `ERR_CONNECTION_REFUSED` / timeout | nothing is listening on 443, or a firewall is dropping it |

**1. Where does the name actually point?**

```sh
dig coexist.humanhighway.net +short          # want exactly: 134.209.168.50
dig coexist.humanhighway.net AAAA +short     # want: nothing
```

A DreamHost-hosted zone can already carry a wildcard or a parked entry for the
subdomain, and a browser preferring a stray AAAA over the A record produces a
certificate error that looks nothing like a DNS problem. If the answer is not
`134.209.168.50` and nothing else, the fault is in the zone and no amount of work on
the VPS will fix it.

**2. Which certificate is being presented?**

```sh
openssl s_client -connect coexist.humanhighway.net:443 \
  -servername coexist.humanhighway.net </dev/null 2>/dev/null \
  | openssl x509 -noout -subject -issuer -dates
```

Issuer `Let's Encrypt` → issuance worked and the fault is elsewhere. Issuer
`Caddy Local Authority` → issuance failed; go to 3. A subject naming another domain
entirely → go back to 1.

**3. Is port 80 reachable from the internet?**

This is the most likely single cause. Let's Encrypt validates over HTTP-01 on port
80, and `ufw` is not the only firewall in front of this box:

```sh
nc -vz 134.209.168.50 80
nc -vz 134.209.168.50 443
```

Both must be open **from off-network**. `vps/publish-constitution.sh` opens them in
ufw, but a **DigitalOcean cloud firewall** attached to the droplet is a separate
layer that ufw cannot see or change. If the droplet has one permitting only 22 and
47100, the ACME challenge never arrives, Caddy falls back to its internal CA, and
everything on the box looks healthy while the browser refuses the certificate. Fix it
in the DigitalOcean control panel (Networking > Firewalls), then:

```sh
ssh root@134.209.168.50 'systemctl restart caddy'
```

**4. What does Caddy say?** It states the reason plainly:

```sh
ssh root@134.209.168.50 \
  'systemctl status caddy --no-pager; echo ---; journalctl -u caddy -n 80 --no-pager; echo ---; ufw status'
```

Look for `obtain certificate`, `challenge failed`, `timeout`, or `rate limit`. Note
that Let's Encrypt rate-limits repeated failures for the same name — if the log shows
that, stop retrying and wait it out rather than restarting Caddy in a loop.

**5. Only if 1-4 are all clean**, suspect the config:

```sh
ssh root@134.209.168.50 \
  'CONSTITUTION_DOMAIN=coexist.humanhighway.net caddy validate --config /etc/caddy/Caddyfile'
```

A bad Caddyfile should never reach this point — the publish script validates before
starting Caddy and aborts on failure — so a fault here means the file on the box is
not the one the script installed.
