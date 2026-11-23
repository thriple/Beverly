#!/usr/bin/env python3
"""Checks Beverly's MCP server end to end: open tests/lift in Beverly, switch on Help > MCP Server, then run
    python3 tests/mcp/check.py
It reads the address and token from your Beverly settings (mcp.json), edits the level and undoes it all (nothing is
saved), takes a screenshot, plays the lift (press E, ride up 5 m) and runs the lift's scripted test as a job.
Exits 0 when every check passes."""
import base64, json, os, pathlib, sys, time, urllib.request

def settings():
    if os.environ.get("APPDATA"): d = pathlib.Path(os.environ["APPDATA"]) / "Beverly"
    elif os.environ.get("XDG_CONFIG_HOME"): d = pathlib.Path(os.environ["XDG_CONFIG_HOME"]) / "beverly"
    else: d = pathlib.Path.home() / ".config" / "beverly"
    return json.loads((d / "mcp.json").read_text())

S = settings()
URL, TOKEN = f"http://127.0.0.1:{S['port']}/mcp", S["token"]
failed = 0

def rpc(method, params=None):
    body = {"jsonrpc": "2.0", "id": 1, "method": method, "params": dict(params or {}, _meta={"io.modelcontextprotocol/protocolVersion": "2026-07-28"})}
    req = urllib.request.Request(URL, json.dumps(body).encode(), {"Authorization": f"Bearer {TOKEN}", "Content-Type": "application/json", "Accept": "application/json, text/event-stream"})
    return json.load(urllib.request.urlopen(req, timeout=90))["result"]

def tool(name, /, **args):
    r = rpc("tools/call", {"name": name, "arguments": args})
    c = r["content"][0]
    if r["isError"]: return {"error": c["text"]}
    return c if c["type"] == "image" else json.loads(c["text"])

def setting(part, field): return next((s["value"] for s in part.get("settings", []) if s["field"] == field), None)

def check(what, ok, got=""):
    global failed
    print(("✓ " if ok else "✗ FAIL ") + what + ("" if ok else f": {got}"))
    failed += not ok

names = [t["name"] for t in rpc("tools/list")["tools"]]
check("every tool is listed", {"add_part", "change_part", "screenshot", "start_test", "press_keys", "run_job"} <= set(names), names)
p = tool("get_project")
check("the lift project is open with nothing unsaved", p.get("parts") == 4 and not p["unsaved_changes"] and not p["testing"], p)
undo0 = p["undo_steps"]

a = tool("add_part", kind="Box", position=[5, 1, 0], settings={"Name": "Check box", "Turn": 30})
check("add_part places a part with settings", a.get("name") == "Check box", a)
# the level notes when it is borrowed for change, so an idle editor (here with a part selected) never has it compared
s0 = tool("get_project")["change_stamp"]; time.sleep(1.5)
check("an idle editor leaves the level alone (its change stamp holds still)", tool("get_project")["change_stamp"] == s0, s0)
check("a plain box is an instance of the lift's ledge (same contents)", tool("get_part", name="Check box").get("instances") == 1)
r = tool("change_part", name="Check box", settings={"Visible": False})
check("changing an instance's contents asks for instances", "error" in r and "instances" in r["error"] and setting(tool("get_part", name="Check box"), "visible"), r)
u = tool("change_part", name="Check box", settings={"Visible": False}, instances="this one")
check("instances \"this one\" makes it Unique and changes only it", setting(u.get("part", {"settings": []}), "unique") and setting(tool("get_part", name="Ledge 1"), "visible"), u)
c = tool("change_part", name="Check box", settings={"Size": [3, 1, 1], "Moves": {"Speed": 1}})
check("change_part changes settings", sorted(c.get("changed", [])) == ["mover", "size"], c)
bad = tool("change_part", name="Check box", settings={"Size": [1, 1, 1], "Collision": "Sticky"})
check("a bad setting changes nothing", "error" in bad and tool("get_part", name="Check box")["size"] == [3, 1, 1], bad)
t = tool("change_part", name="Check box", settings={"Tags": ["slippery", "Climbable"]})
check("tags are set by name and listed", setting(t.get("part", {"settings": []}), "tags") == ["Climbable", "Slippery"] and next((q for q in tool("list_parts") if q["name"] == "Check box"), {}).get("tags") == ["Climbable", "Slippery"], t)
l = tool("add_part", kind="Spot light", position=[2, 4, 2], aim=[2, 0, 2], settings={"Light": {"Cone": 30}})
light = {s["field"]: s["value"] for s in setting(l, "light") or []}
check("a spot light shines at its aim, reaching it, with the cone asked for", l.get("shines") == [0, -1, 0] and light.get("reach") == 4 and light.get("cone") == 30, l)
oc = tool("add_part", kind="Orbit camera", position=[0, 3, 8], aim=[0, 1, 0], settings={"Camera": {"Lap": 8}})
cam = {s["field"]: s["value"] for s in setting(oc, "camera") or []}
check("an orbit camera looks at its aim, which it circles", oc.get("looks") and abs(oc["looks"][2] + 0.97) < 0.01 and cam.get("target") == [0, 1, 0] and cam.get("lap") == 8, oc)
img = tool("screenshot", camera=oc.get("name", ""), at=2, width=320)
check("a screenshot through a camera, a quarter lap on, is a PNG", img.get("type") == "image" and base64.b64decode(img["data"])[:4] == b"\x89PNG", str(img)[:200])
steps = tool("get_project")["undo_steps"]
tool("run_action", id="view.look_through"); bad = tool("change_part", name="Check box", settings={"Visible": True}); tool("run_action", id="view.look_through")
check("looking through and back without flying changes nothing; edits wait meanwhile", tool("get_project")["undo_steps"] == steps and "error" in bad, bad)
tool("run_action", id="level.build_navigation")
nav = tool("get_level").get("navigation", {})
check("Build navigation finds the ground and the ledge, apart", nav.get("built") and nav.get("separate_areas", 0) >= 2 and nav.get("walkable_m2", 0) > 100 and not nav.get("out_of_date"), nav)
fp = tool("find_path", **{"from": [0, 0, 4], "to": [0, 0, -12]})
check("a path from in front of the ledge to behind it goes round (more than its 16 m straight line)", 16.2 < fp.get("length_m", 0) < 30 and len(fp.get("corners", [])) >= 3, fp)
fp = tool("find_path", **{"from": [0, 0, 4], "to": [0, 5, -8]})
check("there is no path onto the ledge (only the lift goes up)", "no way" in fp.get("error", ""), fp)
nv = tool("change_level", navigation={"Step": 0.3})
check("changing who it is for leaves it out of date", (nv.get("level") or {}).get("navigation", {}).get("out_of_date") is True, nv)
check("the light budget counts the shadowed spot", tool("get_level").get("light_budget", {}).get("worst") == 1)
lv = tool("change_level", settings={"Interior": True, "Height": 15})
check("change_level turns the sun off and lowers it", setting(lv.get("level", {"settings": []}), "interior") is True and setting(lv["level"], "height") == 15, lv)
for _ in range(8): tool("run_action", id="edit.undo")
check("undo brings the sun back", setting(tool("get_level"), "interior") is False)
p = tool("get_project")
check("eight undos take every edit back", p["parts"] == 4 and p["undo_steps"] == undo0 and not p["unsaved_changes"], p)

# parts attached to parts: the child goes with the parent, and it is one undo step
tool("add_part", kind="Box", position=[30, 1, 30], settings={"Name": "Ship"})
tool("add_part", kind="Box", position=[32, 1, 30], settings={"Name": "Door"}, parent="Ship")
check("a part says what it is attached to, and the parent lists its children", tool("get_part", name="Door").get("parent_name") == "Ship" and [c["name"] for c in tool("get_part", name="Ship").get("children", [])] == ["Door"])
steps = tool("get_project")["undo_steps"]
tool("change_part", name="Ship", settings={"Position": [40, 1, 30], "Turn": 90})
time.sleep(0.5)
door = tool("get_part", name="Door")
check("moving and turning the parent takes the child with it (a quarter turn about its middle)", abs(door["position"][0] - 40) < 0.05 and abs(door["position"][2] - 28) < 0.05, door["position"])
check("that was one undo step", tool("get_project")["undo_steps"] == steps + 1)
bad = tool("change_part", name="Ship", parent="Door")
check("a part can't be attached to what is attached to it", "error" in bad, bad)
tool("run_action", id="edit.undo")
check("undo takes both back", abs(tool("get_part", name="Door")["position"][0] - 32) < 0.05 and abs(tool("get_part", name="Ship")["position"][0] - 30) < 0.05)
tool("delete_part", name="Ship")
check("deleting the parent lets the child go where it was", tool("get_part", name="Door").get("parent") is None)
for _ in range(4): tool("run_action", id="edit.undo")
check("undo takes the whole parent business back", tool("get_project")["undo_steps"] == undo0 and not tool("get_project")["unsaved_changes"], tool("get_project"))

sh = tool("draw_shape", name="Path", points=[[0, 0, 6], {"at": [4, 0, 6], "handle_in": [-1, 0, 1], "handle_out": [1, 0, -1]}, [8, 0, 2]])
check("draw_shape draws a curve through its points, handles bending it", sh.get("name") == "Path" and len(sh.get("points", [])) == 3 and sh["length"] > 8.5 and not sh["closed"], sh)
check("list_shapes lists it", [c["name"] for c in tool("list_shapes")] == ["Path"])
check("a curve can be closed and redrawn by its name", tool("draw_shape", shape="Path", closed=True).get("closed") is True)
check("and deleted", tool("draw_shape", shape="Path", delete=True).get("deleted") and tool("list_shapes") == [])
st = tool("draw_shape", name="Rope", points=[{"at": [0, 3, 0], "stretch": "air"}, [6, 3, 0]])
check("a point says how its stretch treats geometry (the path's, ground by default, when it says nothing)", [p.get("stretch") for p in st.get("points", [])] == ["air", None] and st.get("stretch") == "ground", st)
check("a stretch is ground, over, air or fall", "error" in tool("draw_shape", points=[{"at": [0, 0, 0], "stretch": "sideways"}, [1, 0, 0]]))
tool("draw_shape", shape="Rope", delete=True)
check("a strip needs a material to paint (the lift has none)", "error" in tool("draw_shape", points=[[0, 0, 0], [3, 0, 0]], strip={"width": 1}))
check("a fill needs a material to paint too, and says when it has none", "error" in tool("draw_shape", points=[[0, 0, 0], [3, 0, 0], [3, 0, 3]], closed=True, fill={"feather": 1}) and sh.get("fill", 1) is None, sh)
check("a curve needs two points", "error" in tool("draw_shape", points=[[0, 0, 0]]))
for _ in range(5): tool("run_action", id="edit.undo")
p = tool("get_project")
check("five undos take the curve edits back", tool("list_shapes") == [] and p["undo_steps"] == undo0, p)

img = tool("screenshot", **{"from": [8, 6, 6], "look_at": [0, 2, -4], "width": 640})
check("a screenshot from elsewhere is a PNG", img.get("type") == "image" and base64.b64decode(img["data"])[:8] == b"\x89PNG\r\n\x1a\n", str(img)[:200])

tool("add_part", kind="Box", position=[4, 0.3, 2], settings={"Name": "Loose box", "Size": [0.6, 0.6, 0.6], "Physics": {"Weight": 10}})
s = tool("start_test")
check("start_test plays from the player start", s.get("testing") and abs(s["player"]["feet"][1] - 0.1) < 0.05, s)
tool("press_keys", keys=["E"], secs=0.2)
g = tool("press_keys", keys=[], secs=4)
pad = next((m for m in g.get("moving_parts", []) if m["name"] == "Pad 1"), {})
check("pressing E lifts the pad 5 m with the player on it", abs(pad.get("moved_by", [0, 0, 0])[1] - 5) < 0.05 and abs(g["player"]["feet"][1] - 5.1) < 0.1, g)
check("get_play says what the player stands on", g["player"].get("standing_on", {}).get("name") == "Pad 1", g["player"])
box = lambda st: next((q for q in st.get("loose_parts", []) if q["name"] == "Loose box"), {})
check("a loose part at rest falls asleep", box(tool("get_play")).get("asleep") is True, tool("get_play").get("loose_parts"))
b = box(tool("blast", at=[4, 0, 0], radius=4, strength=60, secs=0.5))
check("a blast throws a loose part away from it, waking it", b.get("moved_by", [0, 0, 0])[2] > 0.5, b)
p_ = box(tool("pull", part="Loose box", toward=[4, 0.3, -6], force=400, secs=1.5))
check("a winch drags a loose part toward its point", p_.get("position", [0, 0, 9])[2] < b.get("position", [0, 0, 0])[2] - 1, p_)
check("stop_test goes back to editing", tool("stop_test").get("testing") is False)
tool("run_action", id="edit.undo")

# scatter: debris along a curve is parts, made again when the curve changes, and gone with it
before = tool("get_project")["parts"]
steps = tool("get_project")["undo_steps"]
sc = tool("draw_shape", name="Debris", points=[[-6, 0.1, 4], [6, 0.1, 4]], scatter={"settings": {"Spacing": 1.5, "Band": 2}})
n = tool("get_project")["parts"] - before
check("a scatter makes ordinary parts, one undo step", n >= 5 and tool("get_project")["undo_steps"] == steps + 1 and sc.get("scatter", {}).get("items"), (n, sc))
ids = sorted(q["id"] for q in tool("list_parts") if q["name"].split(" ")[0] in ("Stump", "Log", "Boulder"))
tool("draw_shape", shape="Debris", scatter={"settings": {"Seed": 5}})
ids2 = sorted(q["id"] for q in tool("list_parts") if q["name"].split(" ")[0] in ("Stump", "Log", "Boulder"))
check("a new seed lays them again and keeps their ids", ids == ids2 and len(ids) == n, (ids, ids2))
tool("draw_shape", shape="Debris", scatter=None)
check("taking the scatter away takes its items", tool("get_project")["parts"] == before)
for _ in range(2): tool("run_action", id="edit.undo")
check("two undos bring the scatter back, then the curve is gone with a third", tool("get_project")["parts"] - before == n)
tool("run_action", id="edit.undo"); tool("run_action", id="edit.undo")
check("undo takes the scatter back", tool("get_project")["parts"] == before and tool("get_project")["undo_steps"] == steps, tool("get_project"))

# foliage: the carpet, a new species, and a Mix (plants over a ground)
f = tool("get_foliage")
check("get_foliage lists the carpet, the species and the Mixes", {"settings", "species", "mixes"} <= set(f) and len(f["species"]) >= 17, f)
steps = tool("get_project")["undo_steps"]
r = tool("change_foliage", settings={"Reach": 60})
check("change_foliage sets how far the grass grows", setting(r["foliage"], "reach") == 60, r)
r = tool("new_species", name="tall_fern", copy_of="grass", settings={"Height": 2})
check("new_species copies one and changes it", r.get("name") == "tall_fern" and r.get("changed_in_level") and setting(r, "h_scale") == 2, r)
soil = tool("new_material", name="Soil")
mix = tool("new_material", name="Meadow", grass=["tall_fern"], ground=soil["material"])
check("a Mix grows the new species over a ground", mix.get("grass", [{}])[0].get("species") == "tall_fern" and mix.get("ground") == soil["material"], mix)
check("a ground that doesn't exist is refused", "error" in tool("change_material", material=mix["material"], ground="nothing"))
check("a species that doesn't exist is refused", "error" in tool("change_species", species="nothing", settings={"Height": 2}))
for _ in range(4): tool("run_action", id="edit.undo")
check("four undos take the foliage back", tool("get_project")["undo_steps"] == steps and setting(tool("get_foliage")["settings"] and tool("get_foliage"), "reach") != 60)

j = tool("run_job", kind="test", test="lift")
while "job" in j and (r := tool("get_job", id=j["job"]))["status"] == "running": time.sleep(1)
check("the lift's scripted test passes as a job", "job" in j and r["status"] == "passed", j if "job" not in j else r.get("output", "")[-600:])

print("PASS" if not failed else f"FAIL: {failed} checks")
sys.exit(1 if failed else 0)
