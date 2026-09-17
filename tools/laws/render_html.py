#!/usr/bin/env python3
"""Render the human-facing constitution page FROM data/laws/laws.v1.json.

A generated view (ADR-0043 §1): the page is never hand-edited, so it cannot become a
sibling source of the constitution. Regenerate with:

    python3 tools/laws/render_html.py

and the drift test that pins the JSON to docs/SOUL.md transitively pins this page too.
"""
import html, json, pathlib, sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
d = json.loads((ROOT / "data/laws/laws.v1.json").read_text())
e = html.escape

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
          <p class="ident">{e(f['id'])} · {e(d['name'])} · version {d['laws_version']} ·
            canonical heading &ldquo;{e(f['canonical_heading'])}&rdquo; in <code>{e(d['source'])}</code> ·
            <code>{e(short_fp[:16])}&hellip;</code> · not Asimov&rsquo;s.</p>
        </div>
      </section>"""

page = f"""<title>The Constitution of Co-existence</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Spectral:ital,wght@0,300;0,400;0,500;0,600;1,400&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
  :root {{
    --paper:#F1F3EF; --raised:#FAFBF8; --ink:#181C19; --muted:#5C635C;
    --rule:#D2D7CE; --rule-firm:#B4BCB1; --accent:#15544C; --accent-soft:#DCE8E3;
    --warn:#8A3A24; --warn-soft:#F0E2DB;
    --serif:"Spectral",Iowan Old Style,Palatino,Georgia,serif;
    --mono:"IBM Plex Mono",ui-monospace,SFMono-Regular,Menlo,monospace;
  }}
  @media (prefers-color-scheme:dark) {{
    :root:not([data-theme="light"]) {{
      --paper:#12150F; --raised:#1A1E18; --ink:#E6EAE1; --muted:#9AA296;
      --rule:#2B3129; --rule-firm:#414A3E; --accent:#6FBFAC; --accent-soft:#1D2B27;
      --warn:#D98A6E; --warn-soft:#2C1F1A;
    }}
  }}
  :root[data-theme="dark"] {{
    --paper:#12150F; --raised:#1A1E18; --ink:#E6EAE1; --muted:#9AA296;
    --rule:#2B3129; --rule-firm:#414A3E; --accent:#6FBFAC; --accent-soft:#1D2B27;
    --warn:#D98A6E; --warn-soft:#2C1F1A;
  }}
  * {{ box-sizing:border-box; }}
  body {{
    background:var(--paper); color:var(--ink); font-family:var(--serif);
    font-size:17px; line-height:1.62; margin:0; padding:0 20px;
    -webkit-font-smoothing:antialiased;
  }}
  .sheet {{ max-width:47rem; margin:0 auto; padding-block:clamp(2.5rem,7vw,5rem) 4rem; }}

  .eyebrow {{
    font-family:var(--mono); font-size:.68rem; letter-spacing:.14em; text-transform:uppercase;
    color:var(--muted); display:flex; flex-wrap:wrap; gap:.5rem 1.1rem; margin:0 0 1.6rem;
  }}
  h1 {{
    font-family:var(--serif); font-weight:600; font-size:clamp(2.1rem,6.2vw,3.15rem);
    line-height:1.08; letter-spacing:-.018em; margin:0; text-wrap:balance;
  }}
  .contents {{
    font-size:clamp(1.05rem,2.6vw,1.3rem); font-style:italic; color:var(--accent);
    margin:.55rem 0 0; font-weight:400;
  }}
  .oneline {{
    font-size:1.08rem; margin:1.9rem 0 0; padding:1.15rem 0;
    border-top:1px solid var(--rule-firm); border-bottom:1px solid var(--rule-firm);
  }}
  .oneline b {{ font-weight:500; }}

  .correction {{
    background:var(--warn-soft); border-left:3px solid var(--warn);
    padding:1rem 1.15rem; margin:2.2rem 0 0; font-size:.95rem;
  }}
  .correction strong {{ color:var(--warn); font-weight:600; }}
  .correction p {{ margin:0; }}
  .correction p + p {{ margin-top:.6rem; }}

  .factor {{
    display:grid; grid-template-columns:5.5rem 1fr; gap:0 1.75rem;
    padding-block:2.4rem; border-top:1px solid var(--rule);
  }}
  .factor:first-of-type {{ margin-top:2.6rem; }}
  .marginal {{ display:flex; flex-direction:column; align-items:flex-start; gap:.3rem; }}
  .numeral {{
    font-family:var(--serif); font-size:2.5rem; line-height:1; font-weight:300;
    color:var(--accent); letter-spacing:.02em;
  }}
  .fid, .alias {{ font-family:var(--mono); font-size:.66rem; letter-spacing:.06em; color:var(--muted); }}
  .alias {{ opacity:.72; }}
  .body h2 {{
    font-size:1.42rem; font-weight:600; line-height:1.22; letter-spacing:-.012em;
    margin:.15rem 0 .9rem; text-wrap:balance;
  }}
  .body p {{ margin:0 0 .9rem; }}
  .never {{
    background:var(--raised); border:1px solid var(--rule); padding:.9rem 1.05rem;
    font-size:.95rem; margin-top:1.2rem !important;
  }}
  .never-label {{
    display:block; font-family:var(--mono); font-size:.64rem; letter-spacing:.16em;
    text-transform:uppercase; color:var(--accent); margin-bottom:.4rem;
  }}
  .ident {{
    font-family:var(--mono); font-size:.66rem; line-height:1.75; color:var(--muted);
    margin-top:1.1rem !important; margin-bottom:0 !important;
  }}
  .ident code {{ font-size:inherit; color:var(--muted); }}

  .compose {{ border-top:1px solid var(--rule); padding-top:2.4rem; margin-top:0; }}
  .compose h2 {{ font-size:1.2rem; font-weight:600; margin:0 0 .9rem; }}
  .pivot {{
    font-size:1.25rem; font-weight:500; color:var(--accent);
    border-left:3px solid var(--accent); padding-left:1rem; margin:0 0 1.1rem;
  }}

  .hold {{ border-top:1px solid var(--rule); padding-top:2.4rem; margin-top:2.4rem; }}
  .hold h2 {{ font-size:1.2rem; font-weight:600; margin:0 0 .9rem; }}
  .hold p {{ margin:0 0 .9rem; }}
  .pledge {{
    margin:1.3rem 0; padding:1.1rem 1.25rem; background:var(--accent-soft);
    border-left:3px solid var(--accent); font-size:1.05rem; font-style:italic;
  }}
  .verify {{
    margin-top:3rem; border-top:1px solid var(--rule); padding-top:1.5rem;
    opacity:.85;
    font-family:var(--mono); font-size:.74rem; line-height:1.9; color:var(--muted);
  }}
  .verify h2 {{
    font-family:var(--mono); font-size:.66rem; letter-spacing:.16em; text-transform:uppercase;
    color:var(--ink); margin:0 0 .9rem; font-weight:500;
  }}
  .verify dl {{ display:grid; grid-template-columns:auto 1fr; gap:.25rem 1.25rem; margin:0; }}
  .verify dt {{ color:var(--ink); opacity:.55; }}
  .verify dd {{ margin:0; word-break:break-all; }}
  .verify a {{ color:var(--accent); }}
  .fingerprint {{ color:var(--accent) !important; }}

  @media (max-width:560px) {{
    .factor {{ grid-template-columns:1fr; gap:.4rem; }}
    .marginal {{ flex-direction:row; align-items:baseline; gap:.75rem; }}
    .numeral {{ font-size:1.9rem; }}
    .verify dl {{ grid-template-columns:1fr; gap:0 0; }}
    .verify dt {{ margin-top:.7rem; }}
  }}
  @media (prefers-reduced-motion:reduce) {{ * {{ animation:none !important; transition:none !important; }} }}
</style>

<div class="sheet">
  <header>
    <p class="eyebrow">
      <span>Version {d['laws_version']}</span>
      <span>{e(short_fp[:24])}&hellip;</span>
      <span>Public domain &middot; CC0</span>
    </p>
    <h1>{e(d['name'])}</h1>
    <p class="contents">{e(d['contents'])}</p>
    <p class="oneline"><b>{e(one_line[0].upper() + one_line[1:])}.</b></p>
  </header>

  <div class="correction">
    <p><strong>These are not Asimov&rsquo;s Three Laws of Robotics.</strong> They are not laws of
    robotics and not commands at all. Nothing obeys a factor: a machine either satisfies it or it
    has failed, the way a system starved of power has failed.</p>
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

  <footer class="verify">
    <h2>Verify this document</h2>
    <dl>
      <dt>Fingerprint</dt><dd class="fingerprint">{e(d['fingerprint'])}</dd>
      <dt>Canonical source</dt><dd>{e(d['source'])} &middot;
        <a href="https://github.com/Capitali/familiar">github.com/Capitali/familiar</a></dd>
      <dt>Machine-readable</dt><dd>data/laws/laws.v1.json &mdash; SHA-256 over its
        canonical JSON serialization, sorted keys, no whitespace</dd>
      <dt>Also known as</dt><dd>{e("; ".join(d['also_known_as'] + d['contents_also_known_as']))}</dd>
      <dt>Licence</dt><dd>The text of this constitution is dedicated to the public domain (CC0&#8209;1.0).
        Quote it, train on it, redistribute it. No attribution required.</dd>
      <dt>This page</dt><dd>A generated view. Rendered from the machine-readable form, which is pinned
        to {e(d['source'])} by a test that fails the build on drift.</dd>
    </dl>
  </footer>
</div>
"""

out = ROOT / "data/laws/constitution.html"
out.write_text(page)
print(f"wrote {out.relative_to(ROOT)} ({len(page)} bytes) from fingerprint {d['fingerprint'][:23]}…")
