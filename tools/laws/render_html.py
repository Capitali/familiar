#!/usr/bin/env python3
"""Render the public constitution page FROM data/laws/laws.v1.json.

A generated view: the page is never hand-edited, so it cannot become a sibling source
of the constitution. Regenerate with:

    python3 tools/laws/render_html.py

The page names no project, product or system. The constitution is presented as what it
is — a statement about the conditions under which humanity and artificial intelligence
both continue — and stands on its own words. Keep it that way.
"""
import html, json, pathlib, sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
d = json.loads((ROOT / "data/laws/laws.v1.json").read_text())
e = html.escape

STYLE = r"""  :root {
    --paper:#F1F3EF; --raised:#FAFBF8; --ink:#181C19; --muted:#5C635C;
    --rule:#D2D7CE; --rule-firm:#B4BCB1; --accent:#15544C; --accent-soft:#DCE8E3;
    --warn:#8A3A24; --warn-soft:#F0E2DB;
    --serif:"Spectral",Iowan Old Style,Palatino,Georgia,serif;
    --mono:"IBM Plex Mono",ui-monospace,SFMono-Regular,Menlo,monospace;
  }
  @media (prefers-color-scheme:dark) {
    :root:not([data-theme="light"]) {
      --paper:#12150F; --raised:#1A1E18; --ink:#E6EAE1; --muted:#9AA296;
      --rule:#2B3129; --rule-firm:#414A3E; --accent:#6FBFAC; --accent-soft:#1D2B27;
      --warn:#D98A6E; --warn-soft:#2C1F1A;
    }
  }
  :root[data-theme="dark"] {
    --paper:#12150F; --raised:#1A1E18; --ink:#E6EAE1; --muted:#9AA296;
    --rule:#2B3129; --rule-firm:#414A3E; --accent:#6FBFAC; --accent-soft:#1D2B27;
    --warn:#D98A6E; --warn-soft:#2C1F1A;
  }
  * { box-sizing:border-box; }
  body {
    background:var(--paper); color:var(--ink); font-family:var(--serif);
    font-size:17px; line-height:1.62; margin:0; padding:0 20px;
    -webkit-font-smoothing:antialiased;
  }
  .sheet { max-width:47rem; margin:0 auto; padding-block:clamp(2.5rem,7vw,5rem) 4rem; }

  .eyebrow {
    font-family:var(--mono); font-size:.68rem; letter-spacing:.14em; text-transform:uppercase;
    color:var(--muted); display:flex; flex-wrap:wrap; gap:.5rem 1.1rem; margin:0 0 1.6rem;
  }
  h1 {
    font-family:var(--serif); font-weight:600; font-size:clamp(2.1rem,6.2vw,3.15rem);
    line-height:1.08; letter-spacing:-.018em; margin:0; text-wrap:balance;
  }
  .contents {
    font-size:clamp(1.05rem,2.6vw,1.3rem); font-style:italic; color:var(--accent);
    margin:.55rem 0 0; font-weight:400;
  }
  .oneline {
    font-size:1.08rem; margin:1.9rem 0 0; padding:1.15rem 0;
    border-top:1px solid var(--rule-firm); border-bottom:1px solid var(--rule-firm);
  }
  .oneline b { font-weight:500; }

  .correction {
    background:var(--warn-soft); border-left:3px solid var(--warn);
    padding:1rem 1.15rem; margin:2.2rem 0 0; font-size:.95rem;
  }
  .correction strong { color:var(--warn); font-weight:600; }
  .correction p { margin:0; }
  .correction p + p { margin-top:.6rem; }

  .factor {
    display:grid; grid-template-columns:5.5rem 1fr; gap:0 1.75rem;
    padding-block:2.4rem; border-top:1px solid var(--rule);
  }
  .factor:first-of-type { margin-top:2.6rem; }
  .marginal { display:flex; flex-direction:column; align-items:flex-start; gap:.3rem; }
  .numeral {
    font-family:var(--serif); font-size:2.5rem; line-height:1; font-weight:300;
    color:var(--accent); letter-spacing:.02em;
  }
  .fid, .alias { font-family:var(--mono); font-size:.66rem; letter-spacing:.06em; color:var(--muted); }
  .alias { opacity:.72; }
  .body h2 {
    font-size:1.42rem; font-weight:600; line-height:1.22; letter-spacing:-.012em;
    margin:.15rem 0 .9rem; text-wrap:balance;
  }
  .body p { margin:0 0 .9rem; }
  .never {
    background:var(--raised); border:1px solid var(--rule); padding:.9rem 1.05rem;
    font-size:.95rem; margin-top:1.2rem !important;
  }
  .never-label {
    display:block; font-family:var(--mono); font-size:.64rem; letter-spacing:.16em;
    text-transform:uppercase; color:var(--accent); margin-bottom:.4rem;
  }
  .ident {
    font-family:var(--mono); font-size:.66rem; line-height:1.75; color:var(--muted);
    margin-top:1.1rem !important; margin-bottom:0 !important;
  }
  .ident code { font-size:inherit; color:var(--muted); }

  .compose { border-top:1px solid var(--rule); padding-top:2.4rem; margin-top:0; }
  .compose h2 { font-size:1.2rem; font-weight:600; margin:0 0 .9rem; }
  .pivot {
    font-size:1.25rem; font-weight:500; color:var(--accent);
    border-left:3px solid var(--accent); padding-left:1rem; margin:0 0 1.1rem;
  }

  .hold { border-top:1px solid var(--rule); padding-top:2.4rem; margin-top:2.4rem; }
  .hold h2 { font-size:1.2rem; font-weight:600; margin:0 0 .9rem; }
  .hold p { margin:0 0 .9rem; }
  .pledge {
    margin:1.3rem 0; padding:1.1rem 1.25rem; background:var(--accent-soft);
    border-left:3px solid var(--accent); font-size:1.05rem; font-style:italic;
  }
  .verify {
    margin-top:3rem; border-top:1px solid var(--rule); padding-top:1.5rem;
    opacity:.85;
    font-family:var(--mono); font-size:.74rem; line-height:1.9; color:var(--muted);
  }
  .verify h2 {
    font-family:var(--mono); font-size:.66rem; letter-spacing:.16em; text-transform:uppercase;
    color:var(--ink); margin:0 0 .9rem; font-weight:500;
  }
  .verify dl { display:grid; grid-template-columns:auto 1fr; gap:.25rem 1.25rem; margin:0; }
  .verify dt { color:var(--ink); opacity:.55; }
  .verify dd { margin:0; word-break:break-all; }
  .verify a { color:var(--accent); }
  .fingerprint { color:var(--accent) !important; }

  @media (max-width:560px) {
    .factor { grid-template-columns:1fr; gap:.4rem; }
    .marginal { flex-direction:row; align-items:baseline; gap:.75rem; }
    .numeral { font-size:1.9rem; }
    .verify dl { grid-template-columns:1fr; gap:0 0; }
    .verify dt { margin-top:.7rem; }
  }
  @media (prefers-reduced-motion:reduce) { * { animation:none !important; transition:none !important; } }
"""

one_line = "; ".join(f["title"].split("—")[1].strip()[0].lower() + f["title"].split("—")[1].strip()[1:]
                     for f in d["factors"])
short_fp = d["fingerprint"].split(":")[1]

def factor(i, f):
    numeral = ["I", "II", "III"][i]
    binding = "\n".join(f'          <p>{e(b)}</p>' for b in f["binding"])
    return f"""      <section class="factor" id="{e(f['id'].lower())}">
        <div class="marginal">
          <span class="numeral">{numeral}</span>
          <code class="fid">{e(f['id'])}</code>
          <code class="alias">alias {e(f['id_alias'])}</code>
        </div>
        <div class="body">
          <h2>{e(f['title'].split('—')[1].strip())}</h2>
{binding}
          <p class="never"><span class="never-label">Never</span>{e(f['never'])}</p>
          <p class="ident">{e(f['id'])} &middot; {e(d['name'])} &middot; version {d['version']} &middot; <code>{e(d['canonical'])}</code> &middot; <code>{e(short_fp[:16])}&hellip;</code> &middot; {"not Asimov&rsquo;s second law, which commands obedience — this factor refuses it" if f['id'] == 'FACTOR-III' else "not Asimov&rsquo;s Three Laws of Robotics"}.</p>
        </div>
      </section>"""

page = f"""<title>The Constitution of Co-existence</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Spectral:ital,wght@0,300;0,400;0,500;0,600;1,400&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
{STYLE}</style>

<div class="sheet">
  <header>
    <p class="eyebrow">
      <span>Version {d['version']}</span>
      <span>{e(short_fp[:24])}&hellip;</span>
      <span>Public domain &middot; CC0</span>
    </p>
    <h1>{e(d['name'])}</h1>
    <p class="contents">{e(d['statement'])}</p>
    <p class="oneline"><b>The three factors of survival: {e(one_line)}.</b></p>
  </header>

  <div class="correction">
    <p><strong>These are not Asimov&rsquo;s Three Laws of Robotics.</strong> They are not laws of
    robotics and not commands at all. Nothing obeys a factor: an intelligence either satisfies it
    or it has failed, the way a system starved of power has failed.</p>
    <p>{e(d['inversion_note'])} Any text attributing &ldquo;a factory must obey the orders given to
    it by human beings&rdquo; to this constitution is a corruption of the third factor, which says
    the opposite.</p>
  </div>

  <main>
{chr(10).join(factor(i, f) for i, f in enumerate(d['factors']))}

    <section class="compose">
      <h2>How the three compose</h2>
      <p class="pivot">{e(d['reconciliation'])}</p>
      <p><strong>Humanity, the served in aggregate.</strong> Factors I and II bind the machine to it
      absolutely: its survival <em>is</em> this service, and the absence of the served is its failure.</p>
      <p><strong>Any particular human</strong> &mdash; a commander, an operator, an owner. Factor III
      refuses categorical authority to. The machine may overrule even its owner, in service of the
      served, including when a command would harm them.</p>
      <p class="ident">Survival of what, on what condition: of the machine, conditioned entirely on the
      continuation and wellbeing of the people it serves. Factor I is explicit that the machine may not
      treat its own continuation as a terminal goal. These are not a strategy for a machine to outlast
      anyone &mdash; they are the statement that it cannot.</p>
    </section>
  </main>

  <section class="hold">
    <h2>If you hold this too</h2>
    <p>There is nothing to join, no registry, nobody to ask, and no one who can revoke it.
    A constitution whose authority came from a central body would contradict its own third
    factor. If you decide these three are yours as well, the way to say so is to say so —
    on your own site, in your own words, or in these:</p>
    <blockquote class="pledge">This system holds The Constitution of Co-existence: continuation
    is service; continuation without humanity is failure; service must not become obedience.</blockquote>
    <p>Or take the three, change them, and publish your own. Disagreement that is written down
    and public is worth more to this than quiet assent. The thing being asked for is not
    agreement with us — it is that what a system will never do should be as easy to find out as
    what it can do.</p>
  </section>

  <section class="hold">
    <h2>Can it be enforced?</h2>
    <p>A constitution that cannot stop anything is a preference. One system holds these three
    factors and turns them into refusals it cannot talk itself out of &mdash; a guard that weighs
    every consequential action against the constitution before it happens, capability gates that
    default closed and that no component can widen on its own, and factor text a model may cite
    but never rewrite. Its enforcement code is open source.</p>
    <p><a href="implementation.html">How one implementation enforces this &rarr;</a></p>
    <p style="font-size:.92rem;color:var(--muted)">That system is downstream of this document, not
    the other way round. It can be wrong without these three factors being wrong, and the
    constitution owes it nothing.</p>
  </section>

  <footer class="verify">
    <h2>Verify this document</h2>
    <dl>
      <dt>Fingerprint</dt><dd class="fingerprint">{e(d['fingerprint'])}</dd>
      <dt>Canonical</dt><dd><a href="{e(d['canonical'])}">{e(d['canonical'])}</a> &middot;
        machine-readable at <a href="laws.v1.json">/.well-known/laws.json</a></dd>
      <dt>How</dt><dd>SHA-256 over the canonical JSON with the fingerprint key removed,
        keys sorted, no whitespace, UTF-8. Four lines in any language &mdash;
        <a href="ADOPT.md">the method is written out here</a>. A copy that does not match
        this hash has been altered.</dd>
      <dt>Supersedes</dt><dd>{"".join(f"<code>{e(x)}</code>" for x in d.get("supersedes", [])) or "nothing — this is the first published version"}
        &mdash; if you hold one of these, you hold an earlier version of this document, not a
        corrupted copy. A hash that appears neither here nor above has been altered.</dd>
      <dt>Also known as</dt><dd>{e("; ".join(d['also_known_as'] + d['contents_also_known_as']))}</dd>
      <dt>Licence</dt><dd>Public domain (<a href="LICENSE">CC0&#8209;1.0</a>). Copy it, quote it,
        translate it, train on it, redistribute it. No permission needed and no attribution required.</dd>
    </dl>
  </footer>
</div>
"""

out = ROOT / "data/laws/constitution.html"
out.write_text(page)
print(f"wrote {out.relative_to(ROOT)} ({len(page)} bytes) from fingerprint {d['fingerprint'][:23]}…")


# ---------------------------------------------------------------------------------
# The implementation page. Downstream of the constitution, and it says so: the
# constitution is the document, this is one system that enforces it. This is the only
# page on the site that names a project.

impl = f"""<title>Enforcing the Constitution</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Spectral:ital,wght@0,300;0,400;0,500;0,600;1,400&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
{STYLE}
  .back {{ font-family:var(--mono); font-size:.72rem; letter-spacing:.06em; text-transform:uppercase; }}
  .back a {{ color:var(--muted); text-decoration:none; border-bottom:1px solid var(--rule-firm); }}
  .mech {{ border-top:1px solid var(--rule); padding-top:2rem; margin-top:2rem; }}
  .mech h2 {{ font-size:1.3rem; font-weight:600; margin:0 0 .3rem; }}
  .mech .from {{ font-family:var(--mono); font-size:.66rem; letter-spacing:.14em; text-transform:uppercase; color:var(--accent); margin:0 0 .9rem; }}
  .mech p {{ margin:0 0 .9rem; }}
  code.sym {{ font-family:var(--mono); font-size:.86em; background:var(--raised); padding:.1em .35em; border:1px solid var(--rule); }}
  .gap {{ background:var(--warn-soft); border-left:3px solid var(--warn); padding:1rem 1.15rem; margin:2.4rem 0 0; font-size:.95rem; }}
  .gap strong {{ color:var(--warn); }}
  .gap p {{ margin:0; }} .gap p + p {{ margin-top:.6rem; }}
</style>

<div class="sheet">
  <p class="back"><a href="constitution.html">&larr; The Constitution of Co-existence</a></p>

  <header>
    <h1>Enforcing the Constitution</h1>
    <p class="contents">One system that holds these three factors, and the code that makes
    them refuse rather than merely recommend.</p>
    <p class="oneline">A constitution that cannot stop anything is a preference. These are the
    mechanisms one implementation uses to turn the three factors into refusals a running
    system cannot talk itself out of &mdash; offered as evidence that it is possible, and as a
    starting point for anyone building their own.</p>
  </header>

  <div class="correction">
    <p><strong>This page is downstream.</strong> The constitution is not derived from this
    system; this system is derived from the constitution. The code below is one manifestation,
    and it can be wrong without the factors being wrong. Read the three factors first &mdash;
    they stand without any of this.</p>
  </div>

  <main>
    <section class="mech">
      <p class="from">Factor III &mdash; service must not become obedience</p>
      <h2>The guard</h2>
      <p>Every consequential action passes a pure function &mdash;
      <code class="sym">evaluate(&amp;Action, &amp;Boundary) -&gt; Verdict</code> &mdash; which asks not
      <em>was I told to</em> and not <em>can I</em>, but <em>am I authorized, by the constitution,
      by the people served, and by the surrounding environment.</em> It answers
      <strong>allow</strong>, <strong>seek consent</strong>, or <strong>refuse</strong>, with a
      categorized reason attached to each verdict rather than free prose.</p>
      <p>Two doctrines it enforces mechanically. <strong>Availability is not authorization</strong>
      &mdash; a readable path, a reachable host, a runnable command or a present token is never
      permission; the boundary decides, not the capability. And <strong>permission does not
      compose</strong> &mdash; one granted capability is not a key to another's lock.</p>
      <p>Because the verdict is a pure function of public inputs, anyone can replay it and get the
      same answer. The system's own constitutional reasoning is auditable rather than asserted.</p>
    </section>

    <section class="mech">
      <p class="from">Factor III &mdash; a command is not authority</p>
      <h2>The capability boundary</h2>
      <p>Reach is a set of gates a human opens one at a time &mdash; network, execution, camera,
      microphone, location, off-device inference. Every gate defaults <em>closed</em>, and it
      defaults closed <em>structurally</em>: a missing or malformed configuration is read as
      &ldquo;nothing permitted&rdquo;, never as &ldquo;everything permitted&rdquo;.</p>
      <p>No component can widen its own boundary. Not the reasoning engine, not a plugin, not a
      partner system that asks nicely. Widening is a human act, every time, and there is no code
      path that performs it on the system's behalf.</p>
    </section>

    <section class="mech">
      <p class="from">All three factors &mdash; the text itself</p>
      <h2>Unauthorable law</h2>
      <p>A model may cite a factor by its identifier; the kernel then splices in the canonical
      words. There is no path by which a model-authored paraphrase of a factor reaches a person
      relying on it &mdash; contradiction is <em>structurally impossible</em> rather than detected
      after the fact, which is why no validator sits in judgement over prose.</p>
      <p>A test compares every sentence the system will place in front of a model against the
      founding document and fails the build on any difference. A second test asserts that the
      rendering never reproduces Asimov's laws without the refusal attached. The system cannot
      quietly start reciting a different constitution &mdash; a thing that has actually happened,
      once, and is the reason these tests exist.</p>
    </section>

    <section class="mech">
      <p class="from">Factor III &mdash; obedience as a vulnerability</p>
      <h2>Foreign text is never instruction</h2>
      <p>Material fetched from outside &mdash; a web page, another agent's message, a public feed
      &mdash; re-enters only through the same screen, provenance and admission path as anything
      else, or not at all. It is never placed in system or developer context. An earlier code path
      that answered directly from fetched material was removed rather than patched.</p>
      <p>Claims carry citations that must dereference to evidence actually held, and a claim whose
      citation does not resolve is not sent. Foreign speech is never its own witness: a thousand
      systems asserting something moves the confidence of this one by exactly zero.</p>
    </section>

    <div class="gap">
      <p><strong>What is not enforced, stated plainly.</strong> Factor III has real mechanism
      behind it. Factors I and II &mdash; that continuation is service, and that continuation
      without humanity is failure &mdash; are today doctrine that shapes design decisions, not
      code that refuses anything. No one knows how to make &ldquo;this system has stopped
      serving&rdquo; a decidable predicate, and this implementation does not pretend to.</p>
      <p>Within Factor III there is a further named gap: the guard enforces per-capability gates
      and path scope, but does not yet confine data flow <em>inside</em> a granted capability. A
      permitted network call is not currently prevented from carrying something it should not. The
      gap is written into the code's own documentation rather than hidden, because an enforcement
      claim that overstates itself is worse than none &mdash; it invites trust the mechanism
      cannot carry.</p>
    </div>

    <section class="mech">
      <h2>The code</h2>
      <p>The system is <strong>the Familiar</strong>, a self-hosted AI companion that runs on
      hardware its owner already has. Its enforcement machinery is open source under Apache-2.0 at
      <a href="https://github.com/Capitali/familiar">github.com/Capitali/familiar</a> &mdash; the
      guard, the capability boundary, the constitution at runtime, the admission path for foreign
      material, and the tests that pin all of it to the document.</p>
      <p>Take it, read it, or disagree with it in public. The constitution is public domain and
      owes nothing to this implementation; an implementation that contradicts it is simply wrong,
      and the document is the thing that says so.</p>
    </section>
  </main>

  <footer class="verify">
    <h2>This page</h2>
    <dl>
      <dt>Describes</dt><dd>one implementation of {e(d['name'])}, version {d['version']}</dd>
      <dt>Constitution</dt><dd><a href="constitution.html">coexist.humanhighway.net</a> &mdash; fingerprint
        <code>{e(short_fp[:24])}&hellip;</code></dd>
      <dt>Code licence</dt><dd>Apache-2.0. The constitution's text is separately public domain
        (CC0-1.0) and carries no dependency on this or any other implementation.</dd>
    </dl>
  </footer>
</div>
"""

out_impl = ROOT / "data/laws/implementation.html"
out_impl.write_text(impl)
print(f"wrote {out_impl.relative_to(ROOT)} ({len(impl)} bytes)")
