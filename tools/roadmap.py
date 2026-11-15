#!/usr/bin/env python3
"""Beverly's feature list: editor/features.json is the source (the editor's Help > Features window reads it).

Run after changing it (and with every release):  python3 tools/roadmap.py
- checks every feature's needs exist, L / XL features come in parts, and built ones say when (built: date, build: the
  release's build number, as Help > About shows it),
- renumbers the build order (built first, then each feature after everything it needs),
- writes docs/roadmap.html, the readable version with the order, sizes and parts.
"""
import html, json, pathlib, sys
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "editor" / "features.json"
OUT = ROOT / "docs" / "roadmap.html"
SESSIONS = {"S": 0.5, "M": 1}

data = json.loads(SRC.read_text())
F = data["features"]
by = {f["id"]: f for f in F}
for f in F:
    for n in f["needs"]:
        if n not in by: sys.exit(f"{f['id']} needs unknown feature {n}")
    if (f["effort"] in ("L", "XL")) != bool(f["chunks"]): sys.exit(f"{f['id']}: L and XL features need parts, S and M none")
    if f["status"] == "done" and not (f.get("built") and f.get("build")): sys.exit(f"{f['id']} is built: give it built (date) and build (number)")

depth = {}
def d(i, seen=()):
    if i in seen: sys.exit(f"dependency loop through {i}")
    if i not in depth: depth[i] = 1 + max((d(n, seen + (i,)) for n in by[i]["needs"]), default=0)
    return depth[i]
for f in F: d(f["id"])
# polish passes sit between steps on purpose, not by their needs: after step N of their group, or after the last step
for f in F:
    if "pass_after" in f:
        a = f["pass_after"]
        depth[f["id"]] = (max(depth.values()) if a == "last" else a) + 0.5
# the first game's path: what it needs, and everything those need, comes first
path = set()
def mark(i):
    if i not in path: path.add(i); [mark(n) for n in by[i]["needs"]]
for f in F:
    if f.get("first_game"): mark(f["id"])
for f in F: f["first_game"] = f["id"] in path
F.sort(key=lambda f: (f["status"] != "done", f["status"] == "hold", f.get("build", 0), not f["first_game"], not f.get("next"), depth[f["id"]]))  # stable: file order breaks ties
for k, f in enumerate(F): f["order"] = k + 1
SRC.write_text(json.dumps(data, indent=1, ensure_ascii=False) + "\n")

def sessions(f): return SESSIONS.get(f["effort"], len(f["chunks"]))
done = lambda i: by[i]["status"] == "done"
todo = [f for f in F if f["status"] not in ("done", "hold")]
held = [f for f in F if f["status"] == "hold"]
total = sum(sessions(f) for f in todo)
eff = Counter(f["effort"] for f in todo)
# a pass is ready once everything before it in its group is built
before = lambda f: [g for g in todo if g["first_game"] == f["first_game"] and depth[g["id"]] < depth[f["id"]]]
ready = [f for f in todo if all(done(n) for n in f["needs"]) and not ("pass_after" in f and before(f))]

# the longest chain of needs to each feature, for the critical paths shown
def chain(i):
    f = by[i]
    if not f["needs"]: return [i]
    return max((chain(n) for n in f["needs"]), key=lambda c: sum(sessions(by[x]) for x in c if not done(x))) + [i]

e = html.escape
import datetime
def day(s): d = datetime.date.fromisoformat(s); return f"{d.day} {d:%b %Y}"
def card(f):
    needs = " · ".join(f'<a href="#{n}">{e(by[n]["name"])}</a>{" ✓" if done(n) else ""}' for n in f["needs"]) or ("everything before it" if "pass_after" in f else "nothing")
    st = f"built {day(f['built'])}, build {f['build']}" if f["status"] == "done" else "on hold" if f["status"] == "hold" else ("ready" if f in ready else "planned")
    cls = "built" if f["status"] == "done" else st.replace(" ", "")
    return f'''<article class="f {cls}" id="{f["id"]}"><header><span class="n">{f["order"]}</span><h3>{e(f["name"])}</h3><span class="eff">{f["effort"]}</span></header>
<p>{e(f["what"])}</p><p class="meta"><span class="st">{st}</span> · {e(f["area"])} · needs {needs}</p>{f'<p class="note">{e(f["note"])}</p>' if f["note"] else ""}</article>'''

def tiles(f):
    """One tile per session-sized part (an S or M feature is one tile), so a row's length shows the work."""
    parts = f["chunks"] or [{"S": "In one go", "M": "In one go"}[f["effort"]]]
    size = {"S": "half a session", "M": "a session"}.get(f["effort"], "a session")
    return "".join(f'<div class="t"><b>{f["order"]}.{k + 1} · {size}</b><span>{e(c)}</span></div>' for k, c in enumerate(parts))

def row(f): return f'<div class="row">{card(f)}<div class="tiles">{tiles(f)}</div></div>'

phases = {}
for f in todo + held: phases.setdefault(("Last Ascent path" if f["first_game"] else "After Last Ascent", depth[f["id"]]), []).append(f)
# each step its own colour (golden-angle hues, so neighbours differ), on its heading and its cards
def hue(step): return round(step * 137.5 + 40) % 360
def phase_head(k, v):
    if k[1] % 1:  # a polish pass
        return f'<section class="phase pass"><h2><span class="sw"></span>{k[0]} · polish pass</h2><p class="sub">about {sum(sessions(f) for f in v):g} sessions · after everything above it, before what follows</p>'
    return f'<section class="phase" style="--h:{hue(k[1])}"><h2><span class="sw"></span>{k[0]} · step {k[1]}</h2><p class="sub">{len(v)} features · about {sum(sessions(f) for f in v):g} sessions · each needs only earlier steps</p>'
phase_html = "".join(f'{phase_head(k, v)}<div class="rows">{"".join(row(f) for f in v)}</div></section>' for k, v in sorted(phases.items(), key=lambda kv: (kv[0][0] != "Last Ascent path", kv[0][1])))
paths = "".join(f'<li><b>{e(by[t]["name"])}</b>: {" → ".join(e(by[x]["name"]) for x in chain(t) if not done(x))}</li>' for t in ["mech-port", "swarm", "screens", "vehicles", "destructibles"])

builds = {}
for f in F:
    if f["status"] == "done": builds.setdefault((f["build"], f["built"]), []).append(f)
built_html = "<h2>Built so far</h2><p class=\"sub\">Newest first: each release and the features it finished.</p><ul class=\"built\">" + "".join(f'<li><b>Build {b}</b> <span class="sub">{day(dt)}</span> · ' + " · ".join(f'<a href="#{f["id"]}">{e(f["name"])}</a>' for f in v) + "</li>" for (b, dt), v in sorted(builds.items(), reverse=True)) + "</ul>"
notes_html = "<h2>Research behind the new areas</h2><div class=\"cols\">" + "".join(f'<div><h3 style="margin-top:10px">{e(n["title"])}</h3><p class="sub" style="margin-top:4px">{e(n["text"])}</p><p class="sub">' + " · ".join(f'<a href="{e(u)}">{e(t)}</a>' for t, u in n["sources"]) + "</p></div>" for n in data.get("notes", [])) + "</div>"
OUT.write_text(f'''<title>Beverly Roadmap</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600&family=JetBrains+Mono:wght@500&display=swap">
<style>
/* Layout: one reading column for the summary, then a card grid per build step. Generated by tools/roadmap.py from editor/features.json. */
:root {{ --bg:#f6f5f2; --panel:#fff; --fg:#1b1820; --muted:#6b6672; --line:#ddd9d2; --accent:#a8860a; --chip:#f5dc4a; --ok:#2f8f46; --ready:#a8860a;
  --step-l:.62; --f-body:"Inter",system-ui,sans-serif; --f-mono:"JetBrains Mono",ui-monospace,monospace; }}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{ --bg:#0f0c10; --panel:#17131a; --fg:#f2f0f3; --muted:#a19aa8; --line:#2c2631; --accent:#f5dc4a; --ok:#7be08a; --ready:#f5dc4a; --step-l:.74; color-scheme:dark; }} }}
:root[data-theme="dark"] {{ --bg:#0f0c10; --panel:#17131a; --fg:#f2f0f3; --muted:#a19aa8; --line:#2c2631; --accent:#f5dc4a; --ok:#7be08a; --ready:#f5dc4a; --step-l:.74; color-scheme:dark; }}
* {{ box-sizing:border-box; }}
body {{ background:var(--bg); color:var(--fg); font:14px/1.55 var(--f-body); padding-block:28px 60px; padding-left:max(16px, env(safe-area-inset-left, 0px)); padding-right:max(16px, env(safe-area-inset-right, 0px)); }}
.wrap {{ max-width:1180px; margin:0 auto; }}
h1 {{ font-size:30px; margin:0 0 4px; text-wrap:balance; }}
h2 {{ font-size:19px; margin:34px 0 2px; }}
h3 {{ font-size:14.5px; margin:0; flex:1; }}
.lede {{ color:var(--muted); max-width:70ch; margin:0 0 18px; }}
.stats {{ display:flex; flex-wrap:wrap; gap:10px; margin:14px 0 6px; }}
.stat {{ background:var(--panel); border:1px solid var(--line); border-radius:14px 14px 14px 4px; padding:10px 14px; min-width:120px; }}
.stat b {{ display:block; font:600 22px var(--f-mono); font-variant-numeric:tabular-nums; }}
.stat span {{ color:var(--muted); font-size:12px; }}
.cols {{ display:grid; grid-template-columns:repeat(auto-fit, minmax(300px, 1fr)); gap:18px; margin-top:8px; }}
.cols > div {{ min-width:0; }}
.cols ul, .cols dl {{ margin:6px 0 0; padding-left:18px; }}
dl {{ padding-left:0 !important; display:grid; grid-template-columns:auto 1fr; gap:4px 10px; }}
dt {{ font:600 12px var(--f-mono); background:var(--chip); color:#1b1820; border-radius:6px; padding:0 6px; height:fit-content; }}
dd {{ margin:0; color:var(--muted); }}
.sub {{ color:var(--muted); margin:0 0 10px; }}
.f {{ background:var(--panel); border:1px solid var(--line); border-radius:14px 14px 14px 4px; padding:12px 14px; min-width:0; }}
.phase {{ --c:oklch(var(--step-l) 0.15 var(--h)); }}
.phase .f {{ border-left:4px solid var(--c); background:linear-gradient(90deg, oklch(var(--step-l) 0.15 var(--h) / .07), transparent 40%), var(--panel); }}
.phase .f .n {{ color:var(--c); font-weight:600; }}
.phase.pass {{ --c:var(--accent); }} .phase.pass .f {{ border-style:dashed; border-left-style:solid; background:var(--panel); }}
.sw {{ display:inline-block; width:12px; height:12px; border-radius:4px 4px 4px 1px; background:var(--c); margin-right:8px; vertical-align:-1px; }}
.more {{ margin-top:26px; border:1px solid var(--line); border-radius:14px 14px 14px 4px; background:var(--panel); padding:10px 14px; }}
.more > summary {{ font-size:15px; }} .more h2 {{ margin-top:18px; }}
.f header {{ display:flex; gap:8px; align-items:baseline; }}
.f .n {{ font:500 11px var(--f-mono); color:var(--muted); }}
.f .eff {{ font:600 11px var(--f-mono); background:var(--chip); color:#1b1820; border-radius:6px; padding:1px 7px; }}
.f p {{ margin:6px 0 0; }}
.f .meta {{ color:var(--muted); font-size:12px; }}
.f .meta a {{ color:inherit; }}
.f .st {{ font-weight:600; }}
.f.onhold {{ opacity:.6; }}
.f.built .st {{ color:var(--ok); }} .f.ready .st {{ color:var(--ready); }}
.rows {{ display:grid; gap:12px; }}
.row {{ display:grid; grid-template-columns:minmax(0, 360px) minmax(0, 1fr); gap:10px; align-items:start; }}
@media (max-width:760px) {{ .row {{ grid-template-columns:minmax(0, 1fr); }} }}
.tiles {{ display:grid; grid-template-columns:repeat(auto-fill, minmax(190px, 1fr)); gap:8px; }}
.t {{ background:var(--panel); border:1px solid var(--line); border-top:3px solid var(--c, var(--accent)); border-radius:12px 12px 12px 4px; padding:8px 10px; font-size:13px; min-width:0; }}
.t b {{ display:block; font:600 11px var(--f-mono); color:var(--c, var(--accent)); margin-bottom:3px; }}
.phase.pass .t {{ border-style:dashed; border-top-style:solid; }}
.f .note {{ font-size:12px; color:var(--muted); font-style:italic; }}
.f:target {{ outline:2px solid var(--accent); }}
a {{ color:var(--accent); }}
.built {{ margin:6px 0 0; padding-left:18px; }} .built li {{ margin:2px 0; }} .built .sub {{ margin:0; }}
details {{ margin-top:10px; }} summary {{ cursor:pointer; font-weight:600; }}
</style>
<div class="wrap">
<h1>Beverly roadmap</h1>
<p class="lede">Every planned feature in the order it can be built: each comes after everything it needs. What Last Ascent needs (and what that needs) comes first; the rest follows. Sizes: <b>S</b> under a session, <b>M</b> about one, <b>L</b> two or three, <b>XL</b> four or more, with L and XL split into session-sized parts. Next to each feature, one tile per part (about a session each), so a row's length shows the work. Updated {e(data["updated"])}; the editor shows the same list under Help &gt; Features.</p>
<div class="stats">
 <div class="stat"><b>{len(F) - len(todo) - len(held)}</b><span>built</span></div>
 <div class="stat"><b>{len(todo)}</b><span>to build</span></div>
 <div class="stat"><b>{total:g}</b><span>sessions, roughly</span></div>
 <div class="stat"><b>{eff["S"]}·{eff["M"]}·{eff["L"]}·{eff["XL"]}</b><span>S · M · L · XL</span></div>
 <div class="stat"><b>{sum(sessions(f) for f in todo if f["first_game"]):g}</b><span>sessions on the Last Ascent path</span></div>
 <div class="stat"><b>{len(ready)}</b><span>ready to build now</span></div>
</div>
{built_html}
<details class="more"><summary>Planning notes: longest chains, how the sizes were set, research</summary>
<div class="cols">
 <div><h2>Longest chains</h2><p class="sub">What has to come first to reach the big ones. The order inside each step is free.</p><ul>{paths}</ul></div>
 <div><h2>How the sizes were set</h2><dl>
  <dt>S</dt><dd>One small thing in one place: a new part type, a menu, a setting.</dd>
  <dt>M</dt><dd>A system with its UI, tested in Xvfb: triggers, health, the jump family.</dd>
  <dt>L</dt><dd>A system plus its editor and a second layer: lights, physics, abilities. 2–3 parts.</dd>
  <dt>XL</dt><dd>New ground with several layers: the timeline, vehicles, brains, the HUD, the goo swarm. 4+ parts.</dd>
 </dl><p class="sub" style="margin-top:8px">Libraries that cut the work, all checked against Bevy 0.20: avian3d (physics), vleue_navigator (navigation areas), bevy_tnua (walking), bevy_kira_audio (sound), lightyear (multiplayer).</p></div>
</div>
{notes_html}
</details>
{phase_html}
</div>
''')
print(f"{len(F)} features, {len(todo)} to build, about {total:g} sessions; wrote {OUT.relative_to(ROOT)}")
