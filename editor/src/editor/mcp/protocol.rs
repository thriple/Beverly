//! The Model Context Protocol messages (ported from Prism 3D's), with no sockets: one
//! JSON-RPC message in, the HTTP status and body out. Both eras are spoken:
//! the `initialize` handshake (2025-03-26 to 2025-11-25) and the stateless
//! 2026-07-28 revision (version in each request's `_meta`).

use serde_json::{Value, json};

/// The stateless revision.
pub const MODERN: &str = "2026-07-28";
/// Versions that start with `initialize`, newest first.
pub const LEGACY: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

const VERSION_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const SERVER_INFO_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// What a tool gives back.
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    Text(String),
    /// A PNG picture, with a line about it.
    Image { png: Vec<u8>, text: String },
}

impl From<String> for Output {
    fn from(s: String) -> Self {
        Output::Text(s)
    }
}

/// What runs the tools (the app, or a fake in tests).
pub trait Host {
    /// Run tool `name`. `Err` is a tool error the assistant sees (not a
    /// protocol error).
    fn call(&mut self, name: &str, args: &Value) -> Result<Output, String>;
}

/// The HTTP answer: a status, and a JSON body unless there is none.
#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Option<Value>,
}

impl Reply {
    fn ok(id: &Value, result: Value) -> Reply {
        Reply {
            status: 200,
            body: Some(json!({"jsonrpc": "2.0", "id": id, "result": result})),
        }
    }

    fn error(status: u16, id: &Value, code: i64, message: &str, data: Option<Value>) -> Reply {
        let mut e = json!({"code": code, "message": message});
        if let Some(d) = data {
            e["data"] = d;
        }
        Reply {
            status,
            body: Some(json!({"jsonrpc": "2.0", "id": id, "error": e})),
        }
    }
}

fn server_info() -> Value {
    json!({"name": "Beverly", "version": env!("CARGO_PKG_VERSION")})
}

const INSTRUCTIONS: &str = "Beverly is a 3D game editor. get_project describes the open project; list_parts and \
    get_part show the level's parts and every setting they have (with ranges and units); list_part_kinds shows what \
    can be placed. add_part, change_part and delete_part edit the level, wire connects one part's output to \
    another's input. screenshot shows the window or the level from any point; get_camera and move_camera move the \
    view. list_actions shows what can run now and run_action runs one exactly as its menu item or key would. Every \
    change is one undo step (run_action edit.undo). Metres, Y up; a part's position is its middle (a model's is its \
    origin), so a 2 m box stands on the ground at y = 1.";

/// The tools, in a fixed order.
pub fn tools() -> Value {
    let none = json!({"type": "object", "properties": {}});
    let part = json!({
        "type": "object",
        "properties": {
            "id": {"type": "integer", "description": "The part's id (from list_parts)."},
            "name": {"type": "string", "description": "Or its name."}
        },
    });
    json!([
        {
            "name": "get_project",
            "title": "Describe the project",
            "description": "The open project: its name and folder, the level, how many parts and wires, the selection, whether there are unsaved changes, how many steps Undo and Redo can take, whether test mode is on, and Beverly's build.",
            "inputSchema": none,
        },
        {
            "name": "list_parts",
            "title": "List the level's parts",
            "description": "The level's parts: id, name, kind (shape), position, size (or scale for models), the model file for models, and whether it moves. Filter with an optional query matched against the name and kind.",
            "inputSchema": {
                "type": "object",
                "properties": {"query": {"type": "string", "description": "Words to match, such as \"box\" or \"lift\"."}},
            },
        },
        {
            "name": "get_part",
            "title": "Describe a part",
            "description": "One part with every setting it has, as its panel shows them: each with its label, section, kind (number, feel slider, toggle, colour, choice, optional, list, text, turn), value, unit and range (a feel slider gives its two words and the values at each end), plus its wire outputs and inputs and the wires to and from it.",
            "inputSchema": part,
        },
        {
            "name": "get_selection",
            "title": "Describe the selection",
            "description": "The selected part, as get_part describes it, or that nothing is selected.",
            "inputSchema": none,
        },
        {
            "name": "select",
            "title": "Select a part",
            "description": "Select a part by id or name (as clicking it does), or nothing with {\"none\": true}. Actions such as edit.duplicate and edit.delete work on the selection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"},
                    "none": {"type": "boolean", "description": "Select nothing."}
                },
            },
        },
        {
            "name": "list_wires",
            "title": "List wires",
            "description": "Every wire: which part sends which output to which part's input, by id and name.",
            "inputSchema": none,
        },
        {
            "name": "list_assets",
            "title": "List assets",
            "description": "The project's asset files by folder (textures, models, sounds, other), as the Assets panel shows them.",
            "inputSchema": none,
        },
        {
            "name": "get_log",
            "title": "Recent messages",
            "description": "The editor's recent messages, newest last: what was saved, deleted, undone, and errors such as a file that couldn't be read.",
            "inputSchema": none,
        },
        {
            "name": "list_part_kinds",
            "title": "List part kinds",
            "description": "What add_part can place: each kind with what it is, its size when placed, its outputs, and the settings every part has (as get_part lists them, with ranges and units).",
            "inputSchema": none,
        },
        {
            "name": "add_part",
            "title": "Add a part",
            "description": "Place a new part and select it. Without a position it goes where the editor's view is looking, standing on what is there. Settings are set as change_part sets them, so a part can be named, sized, coloured and turned in one call.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "description": "From list_part_kinds, such as Box, Ramp or Player start; Model for a model file."},
                    "model": {"type": "string", "description": "For a Model: its file in the project, such as assets/models/tower.glb (list_assets)."},
                    "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Its middle in metres [x, y, z], Y up (a model's origin)."},
                    "settings": {"type": "object", "description": "Settings by label or field, as change_part takes them."},
                },
                "required": ["kind"],
            },
        },
        {
            "name": "change_part",
            "title": "Change a part",
            "description": "Change any of a part's settings (the ones get_part lists), by label or field name in any case: numbers (kept in range), [x, y, z] for Position and Size (a model's Size is its scale), degrees about the vertical or a quaternion for Turn, [r, g, b] from 0 to 1 or \"#rrggbb\" for Colour, true/false for toggles, a choice by name, and for an optional part true, false or an object of its own settings, such as {\"Moves\": {\"By\": [0, 4, 0], \"Speed\": 1.5}}. All or nothing: one bad setting changes none.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"},
                    "settings": {"type": "object", "description": "Setting label or field → new value."},
                },
                "required": ["settings"],
            },
        },
        {
            "name": "delete_part",
            "title": "Delete a part",
            "description": "Delete a part, by id or name, and its wires.",
            "inputSchema": part,
        },
        {
            "name": "wire",
            "title": "Wire two parts",
            "description": "When part `from` sends `output`, part `to` gets `input` (get_part lists each part's outputs and inputs; a part that moves has inputs Go, Back and Toggle). Parts by id or name. With remove: true, takes the wire out instead.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": {"type": ["integer", "string"]},
                    "output": {"type": "string"},
                    "to": {"type": ["integer", "string"]},
                    "input": {"type": "string"},
                    "remove": {"type": "boolean"},
                },
                "required": ["from", "output", "to", "input"],
            },
        },
        {
            "name": "screenshot",
            "title": "Screenshot",
            "description": "A picture of Beverly's window as the user sees it, or, given `from` (and `look_at`), of the level seen from that point without moving the user's view.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Where to look from, [x, y, z] in metres."},
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "What to look at (default: the selected part, or the middle of the level)."},
                    "width": {"type": "integer", "description": "Pixels wide, 256 to 1920 (default 1024)."},
                },
            },
        },
        {
            "name": "get_camera",
            "title": "Where the view is",
            "description": "The editor view's position, where it looks, and its turn and tilt in degrees.",
            "inputSchema": none,
        },
        {
            "name": "move_camera",
            "title": "Move the view",
            "description": "Move the editor's view (the user sees it move): to `position`, looking at `look_at`; or turned by `yaw` / `pitch` in degrees; or framing a part given by `frame` (id or name). Any left out stay as they are.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                    "yaw": {"type": "number", "description": "Turn in degrees, 0 looking along -Z, 90 along -X."},
                    "pitch": {"type": "number", "description": "Tilt in degrees, up positive, -89 to 89."},
                    "frame": {"type": ["integer", "string"], "description": "A part to frame, by id or name."},
                },
            },
        },
        {
            "name": "list_actions",
            "title": "List actions",
            "description": "Beverly's actions (the same ones as the menus and keys): id, name, menu, keys, and whether it can run now (and why not). Filter with an optional query matched against id, name and menu.",
            "inputSchema": {
                "type": "object",
                "properties": {"query": {"type": "string", "description": "Words to match, such as \"save\" or \"edit\"."}},
            },
        },
        {
            "name": "run_action",
            "title": "Run an action",
            "description": "Run one action by id (from list_actions), exactly as its menu item or key would. Changes go through the undo history: run_action edit.undo takes one back. Actions that open a file dialog need a person and are refused.",
            "inputSchema": {
                "type": "object",
                "properties": {"id": {"type": "string", "description": "The action's id, such as file.save or edit.undo."}},
                "required": ["id"],
            },
        },
    ])
}

/// Standard base64, for pictures.
pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            s.push(if k <= c.len() { A[((n >> (18 - 6 * k)) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// Handle one message. `header` is the `MCP-Protocol-Version` header.
pub fn handle(msg: &Value, header: Option<&str>, host: &mut dyn Host) -> Reply {
    let null = Value::Null;
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return Reply::error(400, &null, -32600, "Invalid request", None);
    };
    // A notification (no id): accepted, nothing to answer.
    let Some(id) = msg.get("id") else {
        return Reply {
            status: 202,
            body: None,
        };
    };
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    let meta_version = params
        .get("_meta")
        .and_then(|m| m.get(VERSION_KEY))
        .and_then(Value::as_str);

    // The stateless revision names its version in every request.
    let modern = match meta_version {
        Some(v) => {
            if let Some(h) = header
                && h != v
            {
                return Reply::error(
                    400,
                    id,
                    -32020,
                    &format!(
                        "Header mismatch: MCP-Protocol-Version header value '{h}' does not match body value '{v}'"
                    ),
                    None,
                );
            }
            if v != MODERN {
                return Reply::error(
                    400,
                    id,
                    -32022,
                    "Unsupported protocol version",
                    Some(json!({"supported": [MODERN], "requested": v})),
                );
            }
            true
        }
        None => false,
    };
    let finish = |mut result: Value| {
        result["resultType"] = json!("complete");
        if modern {
            result["_meta"] = json!({ SERVER_INFO_KEY: server_info() });
        }
        Reply::ok(id, result)
    };

    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked.filter(|v| LEGACY.contains(v)).unwrap_or(LEGACY[0]);
            finish(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {}},
                "serverInfo": server_info(),
                "instructions": INSTRUCTIONS,
            }))
        }
        "server/discover" => finish(json!({
            "supportedVersions": [MODERN],
            "capabilities": {"tools": {}},
            "instructions": INSTRUCTIONS,
            "ttlMs": 3_600_000,
            "cacheScope": "private",
        })),
        "ping" => finish(json!({})),
        "tools/list" => finish(json!({
            "tools": tools(),
            "ttlMs": 3_600_000,
            "cacheScope": "private",
        })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let known = tools()
                .as_array()
                .is_some_and(|t| t.iter().any(|t| t["name"] == name));
            if !known {
                return Reply::error(200, id, -32602, &format!("Unknown tool: {name}"), None);
            }
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let (content, is_error) = match host.call(name, &args) {
                Ok(Output::Text(t)) => (json!([{"type": "text", "text": t}]), false),
                Ok(Output::Image { png, text }) => (
                    json!([
                        {"type": "image", "data": base64(&png), "mimeType": "image/png"},
                        {"type": "text", "text": text},
                    ]),
                    false,
                ),
                Err(e) => (json!([{"type": "text", "text": e}]), true),
            };
            finish(json!({
                "content": content,
                "isError": is_error,
            }))
        }
        _ => Reply::error(
            if modern { 404 } else { 200 },
            id,
            -32601,
            &format!("Method not found: {method}"),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<String>);
    impl Host for Fake {
        fn call(&mut self, name: &str, args: &Value) -> Result<Output, String> {
            self.0.push(name.to_owned());
            match args.get("id").and_then(Value::as_str) {
                Some("bad") => Err("greyed".into()),
                _ => Ok(format!("ran {name}").into()),
            }
        }
    }

    fn req(method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params})
    }

    fn modern(method: &str, mut params: Value) -> Value {
        params["_meta"] = json!({ VERSION_KEY: MODERN });
        req(method, params)
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    fn result(r: &Reply) -> &Value {
        &r.body.as_ref().expect("a body")["result"]
    }

    #[test]
    fn initialize_answers_with_a_version_it_knows() {
        let mut h = Fake(vec![]);
        let r = handle(
            &req("initialize", json!({"protocolVersion": "2025-06-18"})),
            None,
            &mut h,
        );
        assert_eq!(r.status, 200);
        assert_eq!(result(&r)["protocolVersion"], "2025-06-18");
        assert_eq!(result(&r)["serverInfo"]["name"], "Beverly");
        assert!(result(&r)["capabilities"]["tools"].is_object());
        let r = handle(
            &req("initialize", json!({"protocolVersion": "1999-01-01"})),
            None,
            &mut h,
        );
        assert_eq!(result(&r)["protocolVersion"], LEGACY[0]);
    }

    #[test]
    fn notifications_get_202_and_no_body() {
        let r = handle(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            None,
            &mut Fake(vec![]),
        );
        assert_eq!(
            r,
            Reply {
                status: 202,
                body: None
            }
        );
    }

    #[test]
    fn discover_and_list_in_the_new_revision() {
        let mut h = Fake(vec![]);
        let r = handle(&modern("server/discover", json!({})), Some(MODERN), &mut h);
        assert_eq!(result(&r)["supportedVersions"], json!([MODERN]));
        assert_eq!(result(&r)["resultType"], "complete");
        assert_eq!(result(&r)["_meta"][SERVER_INFO_KEY]["name"], "Beverly");
        let r = handle(&modern("tools/list", json!({})), None, &mut h);
        let names: Vec<_> = result(&r)["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            names,
            [
                "get_project",
                "list_parts",
                "get_part",
                "get_selection",
                "select",
                "list_wires",
                "list_assets",
                "get_log",
                "list_part_kinds",
                "add_part",
                "change_part",
                "delete_part",
                "wire",
                "screenshot",
                "get_camera",
                "move_camera",
                "list_actions",
                "run_action"
            ]
        );
        assert!(result(&r)["ttlMs"].is_number());
        assert_eq!(result(&r)["cacheScope"], "private");
    }

    #[test]
    fn tool_calls_reach_the_host_and_errors_are_tool_errors() {
        let mut h = Fake(vec![]);
        let call = |name: &str, id: &str| {
            req("tools/call", json!({"name": name, "arguments": {"id": id}}))
        };
        let r = handle(&call("run_action", "edit.undo"), None, &mut h);
        assert_eq!(result(&r)["content"][0]["text"], "ran run_action");
        assert_eq!(result(&r)["isError"], false);
        let r = handle(&call("run_action", "bad"), None, &mut h);
        assert_eq!(result(&r)["isError"], true);
        assert_eq!(result(&r)["content"][0]["text"], "greyed");
        let r = handle(&call("no_such_tool", ""), None, &mut h);
        assert_eq!(r.body.unwrap()["error"]["code"], -32602);
        assert_eq!(h.0, ["run_action", "run_action"]);
    }

    #[test]
    fn unknown_methods_and_versions() {
        let mut h = Fake(vec![]);
        // Older clients: a JSON-RPC error in a 200.
        let r = handle(&req("resources/list", json!({})), None, &mut h);
        assert_eq!(
            (r.status, r.body.unwrap()["error"]["code"].clone()),
            (200, json!(-32601))
        );
        // The new revision: 404.
        let r = handle(&modern("resources/list", json!({})), None, &mut h);
        assert_eq!(r.status, 404);
        // A version we do not speak.
        let mut m = req("tools/list", json!({}));
        m["params"]["_meta"] = json!({ VERSION_KEY: "2099-01-01" });
        let r = handle(&m, None, &mut h);
        assert_eq!(r.status, 400);
        let e = &r.body.unwrap()["error"];
        assert_eq!(e["code"], -32022);
        assert_eq!(e["data"]["supported"], json!([MODERN]));
        // The header and the body disagree.
        let r = handle(&modern("tools/list", json!({})), Some("2025-11-25"), &mut h);
        assert_eq!(r.status, 400);
        assert_eq!(r.body.unwrap()["error"]["code"], -32020);
        // Not JSON-RPC at all.
        assert_eq!(handle(&json!({"x": 1}), None, &mut h).status, 400);
    }
}
