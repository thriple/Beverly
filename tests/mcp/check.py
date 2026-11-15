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
check("a plain box is an instance of the lift's ledge (same contents)", tool("get_part", name="Check box").get("instances") == 1)
r = tool("change_part", name="Check box", settings={"Visible": False})
check("changing an instance's contents asks for instances", "error" in r and "instances" in r["error"] and setting(tool("get_part", name="Check box"), "visible"), r)
u = tool("change_part", name="Check box", settings={"Visible": False}, instances="this one")
check("instances \"this one\" makes it Unique and changes only it", setting(u.get("part", {"settings": []}), "unique") and setting(tool("get_part", name="Ledge 1"), "visible"), u)
c = tool("change_part", name="Check box", settings={"Size": [3, 1, 1], "Moves": {"Speed": 1}})
check("change_part changes settings", sorted(c.get("changed", [])) == ["mover", "size"], c)
bad = tool("change_part", name="Check box", settings={"Size": [1, 1, 1], "Collision": "Sticky"})
check("a bad setting changes nothing", "error" in bad and tool("get_part", name="Check box")["size"] == [3, 1, 1], bad)
l = tool("add_part", kind="Spot light", position=[2, 4, 2], aim=[2, 0, 2], settings={"Light": {"Cone": 30}})
light = {s["field"]: s["value"] for s in setting(l, "light") or []}
check("a spot light shines at its aim, reaching it, with the cone asked for", l.get("shines") == [0, -1, 0] and light.get("reach") == 4 and light.get("cone") == 30, l)
check("the light budget counts the shadowed spot", tool("get_level").get("light_budget", {}).get("worst") == 1)
lv = tool("change_level", settings={"Interior": True, "Height": 15})
check("change_level turns the sun off and lowers it", setting(lv.get("level", {"settings": []}), "interior") is True and setting(lv["level"], "height") == 15, lv)
for _ in range(5): tool("run_action", id="edit.undo")
check("undo brings the sun back", setting(tool("get_level"), "interior") is False)
p = tool("get_project")
check("five undos take every edit back", p["parts"] == 4 and p["undo_steps"] == undo0 and not p["unsaved_changes"], p)

img = tool("screenshot", **{"from": [8, 6, 6], "look_at": [0, 2, -4], "width": 640})
check("a screenshot from elsewhere is a PNG", img.get("type") == "image" and base64.b64decode(img["data"])[:8] == b"\x89PNG\r\n\x1a\n", str(img)[:200])

s = tool("start_test")
check("start_test plays from the player start", s.get("testing") and abs(s["player"]["feet"][1] - 0.1) < 0.05, s)
tool("press_keys", keys=["E"], secs=0.2)
g = tool("press_keys", keys=[], secs=4)
pad = next((m for m in g.get("moving_parts", []) if m["name"] == "Pad 1"), {})
check("pressing E lifts the pad 5 m with the player on it", abs(pad.get("moved_by", [0, 0, 0])[1] - 5) < 0.05 and abs(g["player"]["feet"][1] - 5.1) < 0.1, g)
check("stop_test goes back to editing", tool("stop_test").get("testing") is False)

j = tool("run_job", kind="test", test="lift")
while "job" in j and (r := tool("get_job", id=j["job"]))["status"] == "running": time.sleep(1)
check("the lift's scripted test passes as a job", "job" in j and r["status"] == "passed", j if "job" not in j else r.get("output", "")[-600:])

print("PASS" if not failed else f"FAIL: {failed} checks")
sys.exit(1 if failed else 0)
