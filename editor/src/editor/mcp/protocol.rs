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

const INSTRUCTIONS: &str = "Beverly is a 3D game editor. get_project describes the open project and get_level the level's lighting (sun or Interior, \
    fill light) and light budget, which change_level changes; list_parts and \
    get_part show the level's parts and every setting they have (with ranges and units); list_part_kinds shows what \
    can be placed. add_part, change_part and delete_part edit the level, wire connects one part's output to \
    another's input. screenshot shows the window or the level from any point; get_camera and move_camera move the \
    view. start_test plays the level as the user's ` key does; press_keys holds keys in the game and get_play \
    reads the player, the moving and the loose parts; blast and pull shove loose parts; stop_test goes back to editing. run_job runs the project's scripted tests \
    or exports the game in the background (get_job checks on it). list_actions shows what can run now and run_action runs one exactly as its menu item or key would. Every \
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
            "description": "The open project: its name and folder, the level, how many parts and wires, the selection, whether there are unsaved changes, how many steps Undo and Redo can take, whether test mode is on, Beverly's build, and the graphics card it renders with (name, backend, kind, and whether it can run virtual geometry), and the performance meter's numbers (frames a second, frame time, the card's time, triangles, meshes in view, lights, entities, memory), each with a green, amber or red grade (a number the card can't give is null), and the bake cache (entries, size, and what is being built with its percent).",
            "inputSchema": none,
        },
        {
            "name": "get_level",
            "title": "Describe the level",
            "description": "The level's own settings as its panel shows them with nothing selected: Lighting (Interior: no sun; Fill and Fill colour: the flat light from every side) and the sun (Direction: where the sun is round the level in degrees, 0 toward +z and 90 toward +x; Height above the horizon in degrees; Colour, Brightness in lux, Shadows), with ranges; the light budget: the most shadowed lights reaching one place (worst, with their names) against the budget the meter in the corner shows; and the navigation area (where characters such as enemies can walk, across floors, stairs and ramps): its settings (Width, Height, Step, Slope of the characters), whether it is built (run_action level.build_navigation) and out of date, how many square metres are walkable and in how many separate areas.",
            "inputSchema": none,
        },
        {
            "name": "find_path",
            "title": "Find a path",
            "description": "The way a character would walk from `from` to `to` along the level's navigation area (build it first with run_action level.build_navigation; get_level says whether it is out of date): its corners from the start to the end and its length in metres, or why there is none (a point off the walkable area, or no way between them, such as onto a block with no stairs). The path is also drawn in the view, as Try a path draws it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                    "to": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                },
                "required": ["from", "to"],
            },
        },
        {
            "name": "change_level",
            "title": "Change the level's settings",
            "description": "Change the level's settings (as get_level lists them, by label or field, as change_part takes them): lighting in settings, such as {\"Interior\": true, \"Fill\": 200} or {\"Height\": 15, \"Direction\": 270}; the navigation area's in navigation, such as {\"Step\": 0.3, \"Width\": 1.2}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "settings": {"type": "object", "description": "Lighting: setting label or field → new value."},
                    "navigation": {"type": "object", "description": "Navigation: setting label or field → new value."},
                    "walker_sounds": {"type": "object", "description": "The walker's sounds (until characters have their own): Step, Jump and Land → a sound file, such as {\"Step\": \"footsteps\"}; null for none."},
                },
            },
        },
        {
            "name": "list_parts",
            "title": "List the level's parts",
            "description": "The level's parts: id, name, kind (shape), position, size (or scale for models), the model file for models, whether it moves, and its surface tags if it has any. Filter with an optional query matched against the name and kind.",
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
            "description": "The project's asset files by folder (textures, materials, models, sounds, shaders, other), as the Assets panel shows them.",
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
            "description": "Place a new part and select it. Without a position it goes where the editor's view is looking, standing on what is there. Settings are set as change_part sets them, so a part can be named, sized, coloured and turned in one call. Lights (Point light, Spot light) have a Light setting: {\"Light\": {\"Brightness\": 400000, \"Reach\": 8, \"Cone\": 40, \"Shadows\": false, \"Colour\": \"#ffd8a0\"}}; a spot shines away from the surface it stands on, or at aim. Tags mark what a part's surface is: {\"Tags\": [\"Climbable\", \"Slippery\"]} (get_part lists the choices); the walker slides on Slippery parts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "description": "From list_part_kinds, such as Box, Ramp or Player start; Model for a model file."},
                    "model": {"type": "string", "description": "For a Model: its file in the project, such as assets/models/tower.glb (list_assets)."},
                    "sound": {"type": "string", "description": "For a Sound: the sound file it plays (get_sound), or a recording such as birds.ogg (it gets a sound file of its own)."},
                    "position": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Its middle in metres [x, y, z], Y up (a model's origin)."},
                    "settings": {"type": "object", "description": "Settings by label or field, as change_part takes them."},
                    "aim": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "For a Spot light: the point [x, y, z] it shines at (it reaches that far). For a camera: the point it looks at (an Orbit camera's target, which it circles)."},
                    "sound": {"type": "string", "description": "For a Sound part: the sound file it plays, or a recording."},
                    "sounds": {"type": "object", "description": "Sounds it plays when things happen to it (its events, as get_part lists them: Pressed, Starts, Arrived, Returned, Hit), such as {\"Pressed\": \"click\"}; null for none."},
                    "sounds_like": {"type": ["string", "null"], "description": "What its surface sounds like underfoot (Stone, Wood, Metal, Grass, Dirt, Sand, Gravel, Water, Snow, Carpet; null: nothing special): steps play their recordings for it."},
                    "parent": {"type": ["string", "integer", "null"], "description": "The part (name or id) this one is attached to: when that one moves or turns, this one goes with it, and it stays where it is now. null lets go. A part can't be attached to itself or to anything attached to it."},
                },
                "required": ["kind"],
            },
        },
        {
            "name": "change_part",
            "title": "Change a part",
            "description": "Change any of a part's settings (the ones get_part lists), by label or field name in any case: numbers (kept in range), [x, y, z] for Position and Size (a model's Size is its scale), degrees about the vertical or a quaternion for Turn, [r, g, b] from 0 to 1 or \"#rrggbb\" for Colour, true/false for toggles, a choice by name, and for an optional part true, false or an object of its own settings, such as {\"Moves\": {\"By\": [0, 4, 0], \"Speed\": 1.5}}. Parts with the same contents (what is inside the box: shape, colour, textures, Visible, Collision) are instances of each other; name, position, turn and size are each part's own. All or nothing: one bad setting changes none.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"},
                    "settings": {"type": "object", "description": "Setting label or field → new value."},
                    "parent": {"type": ["string", "integer", "null"], "description": "The part (name or id) this one is attached to: when that one moves or turns, this one goes with it, and it stays where it is now. null lets go. A part can't be attached to itself or to anything attached to it."},
                    "aim": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "For a Spot light: the point [x, y, z] it shines at (it reaches that far). For a camera: the point it looks at (an Orbit camera's target, which it circles)."},
                    "instances": {"type": "string", "enum": ["all", "this one"], "description": "Needed when the change is to what is inside the box (shape, colour, textures, Visible, Collision) of a part with instances (get_part's instances): \"all\" changes them all, \"this one\" makes this part Unique and changes only it."},
                },
                "required": [],
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
            "name": "lay_texture",
            "title": "Lay a texture on a part",
            "description": "Lay a texture from assets/textures (list_assets) on a basic shape: on one face by name (get_part lists its faces, such as Top or Front) or all over. It goes through a material in assets/materials made for that texture (the same one every time, so surfaces share it); or give material to lay one you made. tile is how big one copy is in metres [across, down]; offset slides it [across, down]. Without texture, changes the tile or offset of the texture already there; with remove: true, takes it off. On a part with instances, say instances.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"},
                    "texture": {"type": "string", "description": "Such as assets/textures/brick.png, or just brick.png."},
                    "material": {"type": "string", "description": "Lay a material instead (get_material), such as brick or assets/materials/brick.bev."},
                    "face": {"type": "string", "description": "A face name from get_part, or \"all\" (the default)."},
                    "tile": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2},
                    "offset": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2},
                    "remove": {"type": "boolean"},
                    "instances": {"type": "string", "enum": ["all", "this one"], "description": "Needed when the change is to what is inside the box (shape, colour, textures, Visible, Collision) of a part with instances (get_part's instances): \"all\" changes them all, \"this one\" makes this part Unique and changes only it."},
                },
            },
        },
        {
            "name": "paint_stroke",
            "title": "Paint a brush stroke",
            "description": "Paint a material onto a part with the brush, along points on its surface (in the world): size in metres, hardness 0 (soft) to 1 (hard), strength 0 to 1, lays add (a little more each pass), subtract (takes that material away, leaving others) or set (exactly its strength), and rules for where it lands: slope (the steepest surface in degrees, 90 any), band ([lowest, highest] world height, or null) and on (a surface tag the part must have). The brush's settings when left out. Later strokes cover earlier ones; erase: true takes all paint away instead; clear: true removes every stroke. Visible basic shapes only. Kept as strokes, one undo step each.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"},
                    "material": {"type": "string", "description": "Such as moss or assets/materials/moss.bev (list_assets)."},
                    "points": {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3}},
                    "size": {"type": "number"},
                    "hardness": {"type": "number"},
                    "strength": {"type": "number"},
                    "lays": {"type": "string", "enum": ["add", "subtract", "set"]},
                    "rules": {"type": "object", "properties": {"slope": {"type": "number"}, "band": {"type": ["array", "null"], "items": {"type": "number"}}, "on": {"type": ["string", "null"]}}},
                    "erase": {"type": "boolean"},
                    "clear": {"type": "boolean"},
                    "instances": {"type": "string", "enum": ["all", "this one"]},
                },
            },
        },
        {
            "name": "list_shapes",
            "title": "List the pen's curves",
            "description": "Every curve drawn with the pen (P): its id, name, whether it is closed, its length in metres, its path's stretch and steepest ground, and its points. A point is its place in the world and its two handles as offsets from it (handle_out leads on toward the next point, handle_in back toward the one before; [0, 0, 0] makes a corner), its own stretch (null: as the path says) and its sag.",
            "inputSchema": none,
        },
        {
            "name": "draw_shape",
            "title": "Draw a curve with the pen",
            "description": "Draw a curve as the pen does: points in the world (on surfaces; list_parts and get_part give where things are), each [x, y, z] for a corner or {at: [x, y, z], handle_in: [x, y, z], handle_out: [x, y, z]} with the handles as offsets from the point (a smooth point has them opposite). A point can also say stretch: how the line leaving it treats what it meets: ground (lies on walkable surfaces, passing over props), over (follows any geometry), air (ignores geometry) or fall (leaves along its handle and drops until it hits a surface); left out or null, it does as the path's stretch says (ground unless set). A point whose stretch is air can say sag: how far (m, up to 10) that stretch hangs down in its middle, like a rope. stretch on the curve itself sets the path's default, and steepest the steepest surface (degrees, default 60) that ground lines lie on and fills paint. closed: true joins the last point back to the first. Give shape (an id or name from list_shapes) to redraw that curve instead of making a new one, and delete: true to remove it. strip paints a material along the line, as the brush would, kept in step with the curve: {material, width (m), edge (0 soft to 1 hard), strength}; null takes it away. fill paints a material over the ground the curve encloses (an open curve is closed by a straight line), with a feathered edge, also kept in step with the curve and laid under a strip: {material, feather (m, how far in from the edge it fades), strength}; null takes it away. The fill goes on the walkable ground inside the outline, not on props standing on it. One undo step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "strip": {"type": ["object", "null"], "properties": {"material": {"type": "string"}, "width": {"type": "number"}, "edge": {"type": "number"}, "strength": {"type": "number"}}},
                    "fill": {"type": ["object", "null"], "properties": {"material": {"type": "string"}, "feather": {"type": "number"}, "strength": {"type": "number"}}},
                    "shape": {"type": ["integer", "string"], "description": "An existing curve's id or name, to redraw or delete."},
                    "name": {"type": "string", "description": "A new curve's name."},
                    "points": {"type": "array", "items": {"anyOf": [
                        {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                        {"type": "object", "properties": {"at": {"type": "array", "items": {"type": "number"}}, "handle_in": {"type": "array", "items": {"type": "number"}}, "handle_out": {"type": "array", "items": {"type": "number"}}, "stretch": {"type": ["string", "null"], "enum": ["ground", "over", "air", "fall", null]}, "sag": {"type": "number"}}, "required": ["at"]},
                    ]}},
                    "closed": {"type": "boolean"},
                    "delete": {"type": "boolean"},
                    "stretch": {"type": "string", "enum": ["ground", "over", "air", "fall"], "description": "How the path's lines go unless a point says otherwise."},
                    "steepest": {"type": "number", "description": "The steepest surface (degrees) ground lines lie on and fills paint; default 60."},
                },
            },
        },
        {
            "name": "bucket_fill",
            "title": "Fill with the bucket",
            "description": "Fill as the bucket (G) does: start from a part (id or name; point, a place on it in the world, and face, a face name such as Top or Front, say where; else its top) and lay a material over whole faces of everything that counts as the same: fills material (every face with the same material laid on it; with none laid there, the same colour), colour (every face of the same colour within tolerance, 0 exact to 1 any), tag (every part with the surface tag given, or the part's first), part (all of that part) or floor (the walkable faces at that height). connected: true keeps it to parts touching the one clicked, part to part. strength 0 to 1 and lays add, subtract or set, as the brush. material null or erase: true erases instead. The fill stays a rule: change what was clicked and what it fills follows. Filling the same faces again replaces the earlier fill; delete: a fill's id removes it. Visible basic shapes only. One undo step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "name": {"type": "string"},
                    "point": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                    "face": {"type": "string"},
                    "fills": {"type": "string", "enum": ["material", "colour", "tag", "part", "floor"]},
                    "tolerance": {"type": "number"},
                    "tag": {"type": "string"},
                    "connected": {"type": "boolean"},
                    "material": {"type": ["string", "null"]},
                    "erase": {"type": "boolean"},
                    "strength": {"type": "number"},
                    "lays": {"type": "string", "enum": ["add", "subtract", "set"]},
                    "delete": {"type": "integer"},
                },
            },
        },
        {
            "name": "list_fills",
            "title": "List the bucket fills",
            "description": "Every bucket fill in the level, oldest first (later ones lie on top): its id, the part it started from, what it fills, its tolerance, tag and connected, the material it lays (empty: it erases), strength and lays, and the parts it reaches now with how many faces of each.",
            "inputSchema": none,
        },
        {
            "name": "get_sound",
            "title": "Describe a sound",
            "description": "A sound file (list_assets lists them under sounds, beside the recordings): its recordings, those it plays on particular surfaces instead (on_surfaces), how many Sound parts play it, and its settings as its panel shows them (Pick, Volume, Pitch, Vary, Near, Range, Loop, At once, with ranges).",
            "inputSchema": {"type": "object", "properties": {"sound": {"type": "string", "description": "Such as birds, birds.bev or assets/sounds/birds.bev."}}, "required": ["sound"]},
        },
        {
            "name": "change_sound",
            "title": "Change a sound",
            "description": "Change a sound file's settings (as get_sound lists them, by label or field) and/or its recordings (the whole list, files in assets/sounds). Every Sound part playing it changes. Saved with the project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "sound": {"type": "string"},
                    "settings": {"type": "object", "description": "Setting label or field → new value, such as {\"Loop\": true, \"Range\": 80}."},
                    "recordings": {"type": "array", "items": {"type": "string"}, "description": "Such as [\"step 1.ogg\", \"step 2.ogg\"]; one plays each time."},
                    "sets": {"type": "object", "description": "Recordings it plays on particular surfaces instead (Stone, Wood, Metal, Grass, Dirt, Sand, Gravel, Water, Snow, Carpet), such as {\"Grass\": [\"grass 1.ogg\"]}; an empty list goes back to its usual ones there."},
                },
                "required": ["sound"],
            },
        },
        {
            "name": "new_sound",
            "title": "Make a sound",
            "description": "Make a new sound file, from recordings or as a copy of another (copy_of), with settings if given; answers with its path for add_part (kind Sound) or change_part (sound).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Its name (a number is added), such as Footsteps."},
                    "copy_of": {"type": "string"},
                    "recordings": {"type": "array", "items": {"type": "string"}},
                    "sets": {"type": "object"},
                    "settings": {"type": "object"},
                },
            },
        },
        {
            "name": "play_sound",
            "title": "Play a sound",
            "description": "Play a sound file once for the user to hear (as its panel's Play does), or the one a Sound part plays (part). Nothing is changed.",
            "inputSchema": {"type": "object", "properties": {"sound": {"type": "string"}, "part": {"type": ["string", "integer"], "description": "A Sound part's name or id."}, "ground": {"type": "string", "description": "Play it as on this surface (its recordings for there), such as Grass."}}},
        },
        {
            "name": "get_material",
            "title": "Describe a material",
            "description": "A material (list_assets lists them under materials): its texture, how many surfaces and parts use it, and its settings as the Materials panel shows them (Tint, Opacity, Metal, Roughness, Glow, with ranges).",
            "inputSchema": {"type": "object", "properties": {"material": {"type": "string", "description": "Such as brick, brick.bev or assets/materials/brick.bev."}}, "required": ["material"]},
        },
        {
            "name": "change_material",
            "title": "Change a material",
            "description": "Change a material's settings (as get_material lists them, by label or field, as change_part takes them) and/or its texture (a file in assets/textures, or null for none). Every surface using it changes. Saved with the project.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "material": {"type": "string"},
                    "settings": {"type": "object", "description": "Setting label or field → new value, such as {\"Glow\": 3, \"Tint\": \"#ff8000\"}."},
                    "texture": {"type": ["string", "null"]},
                    "sounds_like": {"type": ["string", "null"], "description": "What it sounds like underfoot wherever it's laid (Stone, Wood, Metal, Grass…), over what the part says; null: nothing special."},
                    "shader": {"type": ["string", "null"], "description": "A shader in assets/shaders (list_assets) that draws the material, such as stripes or stripes.wesl; null: the standard look. Choosing one clears the old one's values."},
                    "shader_values": {"type": "object", "description": "The shader's settings (get_shader lists them) → value: a number, true/false for a toggle, or [r, g, b] (0 to 1) for a colour. A setting not given keeps the shader's default."},
                },
                "required": ["material"],
            },
        },
        {
            "name": "get_shader",
            "title": "Read a shader",
            "description": "A custom shader (list_assets lists them under shaders): its source, the settings its header declares (name, label, kind, range, default), the materials using it, and any problem (`problem`). A shader is a WESL fragment shader for Bevy's standard material, like the paint layers' (copy an existing one: its imports and `fragment` are only for the main pass, so they sit under `@if(!PREPASS_PIPELINE)`, which lets the depth and shadow passes use just its raster hooks). Header comment lines tell Beverly what it asks for: `// @setting speed \"Speed\" 0..5 = 1` (a slider), `// @setting glow colour \"Glow\" = 1 0.5 0.2`, `// @setting waves toggle \"Waves\" = on`, `// @see-through`, `// @raster vertex_offset 0.5` (the most it moves a vertex, in metres), `// @raster alpha_mask`. The hooks are functions the shader defines: `fn vertex_offset(world: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>) -> vec3<f32>` (how far to move a vertex, in world space) and `fn alpha_mask(world: vec3<f32>, uv: vec2<f32>) -> f32` (below 0.5 the pixel is cut away; the shader's own fragment calls it and discards too). They also run in the depth and shadow passes, so shadows follow. `@see-through` makes an opaque material blend, and `alpha_mask` makes it cut out. Beverly adds a function for each setting after the code: setting_speed() is an f32, setting_glow() a vec3<f32>, setting_waves() a bool.",
            "inputSchema": {"type": "object", "properties": {"shader": {"type": "string", "description": "Such as stripes, stripes.wesl or assets/shaders/stripes.wesl."}}, "required": ["shader"]},
        },
        {
            "name": "write_shader",
            "title": "Write a shader",
            "description": "Make or replace assets/shaders/NAME.wesl with this source (see get_shader for what a shader is and its header). The header is checked first (a bad line is refused with its line number). The file is read again at once: materials using it change as it is saved, and a shader that doesn't parse keeps its last good version on screen. Mistakes in the shader's code show in the editor's island with the line, and get_shader's `problem` says what is wrong once a material uses it. Assign it with change_material.",
            "inputSchema": {"type": "object", "properties": {"name": {"type": "string", "description": "The shader's name, such as stripes (letters, digits, - and _)."}, "source": {"type": "string"}}, "required": ["name", "source"]},
        },
        {
            "name": "new_material",
            "title": "Make a material",
            "description": "Make a new material, plain or as a copy of another (copy_of), with settings and a texture if given; answers with its path for lay_texture.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Its name (a number is added), such as Glass."},
                    "copy_of": {"type": "string"},
                    "texture": {"type": "string"},
                    "settings": {"type": "object"},
                },
            },
        },
        {
            "name": "screenshot",
            "title": "Screenshot",
            "description": "A picture of Beverly's window as the user sees it; or, given `from` (and `look_at`), of the level seen from that point; or, given `camera`, through that camera part (16:9, with its View), `at` seconds into its move (an Orbit camera circling, a Follow camera with the player at the player start). The user's view doesn't move, and the editor's lines and handles are left out.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "Where to look from, [x, y, z] in metres."},
                    "look_at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": "What to look at (default: the selected part, or the middle of the level)."},
                    "camera": {"type": ["integer", "string"], "description": "A camera part's id or name, to see through it."},
                    "at": {"type": "number", "description": "With camera: seconds into its move (default 0)."},
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
            "name": "start_test",
            "title": "Start testing",
            "description": "Play the level, as the user's ` key does: the player character drops in and the editor waits. Answers with the play state (as get_play).",
            "inputSchema": {
                "type": "object",
                "properties": {"from": {"description": "\"player_start\" (the default), \"view\" (where the editor's view is looking) or [x, y, z] for the feet."}},
            },
        },
        {
            "name": "press_keys",
            "title": "Press keys in the game",
            "description": "While testing: hold keys together for `secs` (default 0.1), optionally turning the view by `look` [dx, dy] in mouse pixels (about 0.14° each), then let go and answer with the play state. Keys: letters, digits, Space, Shift, Ctrl, Alt, Tab, Enter, Escape, Up, Down, Left, Right, MouseLeft, MouseRight. The walker: W A S D walk, Shift runs, Space jumps, E presses a button nearby.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "keys": {"type": "array", "items": {"type": "string"}},
                    "secs": {"type": "number", "description": "How long to hold them, 0.02 to 30."},
                    "look": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2},
                },
            },
        },
        {
            "name": "get_play",
            "title": "Play state",
            "description": "While testing: the player (feet, and for the built-in walker its velocity, whether it stands on something, which way it faces and the part it stands on with that part's surface tags), where every moving part is now, and every loose part (where it is, how far it has moved, whether it is asleep: at rest until something touches it).",
            "inputSchema": none,
        },
        {
            "name": "blast",
            "title": "Set off a blast",
            "description": "While testing: a blast at `at` throws the loose parts within `radius` metres (default 4) away from it, by an impulse of `strength` kg·m/s (default 200) at the middle fading to none at the edge, so light parts fly further. Answers with get_play's state `secs` later (default 1).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "at": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                    "radius": {"type": "number"},
                    "strength": {"type": "number"},
                    "secs": {"type": "number"},
                },
                "required": ["at"],
            },
        },
        {
            "name": "pull",
            "title": "Winch a loose part",
            "description": "While testing: pull a loose part (id or name) toward `toward` with `force` newtons (default 500) for `secs` seconds (default 1), as a winch would. Answers with get_play's state when the pull ends.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "part": {"type": ["integer", "string"]},
                    "toward": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3},
                    "force": {"type": "number"},
                    "secs": {"type": "number"},
                },
                "required": ["part", "toward"],
            },
        },
        {
            "name": "stop_test",
            "title": "Stop testing",
            "description": "Back to editing, with the view where it was.",
            "inputSchema": none,
        },
        {
            "name": "run_job",
            "title": "Run a job",
            "description": "Start long work in the background on the saved project (save first): kind \"test\" runs the project's scripted tests (tests/*.ron, listed by get_project; all of them, or one given by `test`) with the game player; kind \"export\" writes the Windows game to exports/<project>.zip in the project folder. Answers with a job id for get_job.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": ["test", "export"]},
                    "test": {"type": "string", "description": "One test's name (without .ron)."},
                },
                "required": ["kind"],
            },
        },
        {
            "name": "get_job",
            "title": "Check a job",
            "description": "A job's status (running, passed or failed), how long it has taken, and its output: for tests, each test's report with every check.",
            "inputSchema": {"type": "object", "properties": {"id": {"type": "integer"}}, "required": ["id"]},
        },
        {
            "name": "list_jobs",
            "title": "List jobs",
            "description": "The jobs started this session, newest last, with their status.",
            "inputSchema": none,
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
                "get_level",
                "find_path",
                "change_level",
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
                "lay_texture",
                "paint_stroke",
                "list_shapes",
                "draw_shape",
                "bucket_fill",
                "list_fills",
                "get_sound",
                "change_sound",
                "new_sound",
                "play_sound",
                "get_material",
                "change_material",
                "get_shader",
                "write_shader",
                "new_material",
                "screenshot",
                "get_camera",
                "move_camera",
                "start_test",
                "press_keys",
                "get_play",
                "blast",
                "pull",
                "stop_test",
                "run_job",
                "get_job",
                "list_jobs",
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
