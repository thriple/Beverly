# Vehicles: the ground vehicle editor (spec)

For making vehicles in Beverly's editor; section 10 keeps it reusable for games where players build their own.

Written 2026-11-24 from the user's list, the roadmap (`vehicles`, `assemblies`, `sockets`, `control-target`,
`input-map`, `mounts`, `particles`, `decals`), the Vehicles board of the redesign
(`docs/ui/design/boards/Vehicles.dc.html`) and a short look at how other engines do it. It is meant to be built in
its own session (or sessions) and merged into main without surprises. Read *How it plugs in* before writing code.

Flying, underwater and space vehicles get their own tabs later (see *Later tabs*); nothing here should block them.

---

## 1. The user's questions, answered

None of these is one fixed answer: each is a setting on the vehicle file, with a default. The defaults below are the
recommendation; the user can change any of them per vehicle or per game.

| Question | Answer | Default |
|---|---|---|
| Is it always moving? | A **Throttle scheme** choice. *Hold to go* (W drives, let go and it coasts), *Auto-forward* (always moving; W and S speed up and slow down, as in runner and rail-shooter games), *Lever* (the throttle stays where you set it; Shift and Ctrl step it, Z full, X zero, as in the Gates of Heaven notes) and *Cruise* (hold a speed). | Hold to go |
| Is it point and click? | A **Steering scheme** choice. *Point and click* drives to where you click, on the navigation mesh, steering itself (RTS and top-down games). It needs `navmesh`, so it comes in a later chunk. | Off |
| Is it WASD? | Yes by default: W throttle, S brake then reverse, A and D steer, Space handbrake, E enter and leave. All of it goes through named actions (`input-map`), so gamepads, rebinding and other schemes use the same actions. | WASD |
| How many wheels does it have? | Any number from 1 to 16, in **axles** (a pair) or single wheels (a trike's front, a motorbike, a unicycle). A wheel is a socket on the body; the Body topic adds and mirrors them. | 4 (2 axles) |
| Which wheels steer? | Each axle has a **Steers** setting: *No*, *Yes* or *Opposite* (rear wheels that steer against the front, for tight turns). Wheels on the same axle steer at slightly different angles so they don't scrub (Ackermann; on by default, under Advanced). | Front axle steers |
| Does it have legs? | A **Moves on** choice, *Legs*, comes later. A legged vehicle walks with the same leg solver as characters (`joints`: foot placement, gait), so it waits for that. Last Ascent's mech is planned as a character file; if it becomes a vehicle, it uses this. | — |
| Legs and wheels? | *Legs and wheels* (wheels at the ends of legs that drive on flat ground and step over obstacles) comes after *Legs*. | — |
| Tank treads? | *Tracks*: two tracks, each driven on its own (skid steering: turning slows one side or runs it backward, so a tank turns on the spot). The tracks are drawn as a loop of links that moves with speed; the physics is a row of wheels hidden under each track. | — |
| Does it hover? | *Hover*: thrusters push it up to a ride height over ground and water, with little grip (it slides). Strafing and a drift setting under Advanced. | — |
| Is it on rails? | *Rails*: it follows a pen curve (the Level's pen draws the track). Throttle and brake only; steering picks the branch at a junction. Trams, mine carts, roller coasters, rail shooters. | — |

So **Moves on** has five choices for ground vehicles: Wheels, Tracks, Hover, Rails (all in this spec) and Legs, then
Legs and wheels (later, after `joints`). The board already shows Wheels, Hover and Tracks.

---

## 2. The user's list, and what was missing

The user's list (all in this spec): power off and on, accessory mode, grouping, brush and pen for decals, basic
colours, interior and exterior lights, rigging, steering, grip, physics, basic controls with an advanced toggle,
speed control, throttle control schemes, suspension, exhaust plumes tied to power and throttle, camera rumble tied
to throttle.

**Added** (marked **+** where they appear below):

- **Driving basics:** brakes, handbrake, reverse, gears (automatic by default), engine braking, boost, drift.
- **Moving through the world:** grip by surface (ice, mud, sand, water by tag and material), air control, landing,
  and putting it back on its wheels when it flips.
- **People in it:** seats (driver and passengers), entering and leaving (E), where they get out, a walkable inside
  (a bus, a truck bed) that moves with it, being pushed or run over.
- **Cameras:** chase, bonnet, cockpit and free look, the key that switches them, and a field-of-view kick with speed.
- **Sound:** engine pitch by RPM and load, start and stop, horn, skids, impacts, gear changes and indicator ticks,
  through the Audio tab's sound hooks.
- **Signals:** brake, reverse and indicator lights that follow the driving, a horn, sirens and beacons.
- **Wheel marks:** skid marks as decals that fade, and dust, mud and splash from the wheels by surface.
- **The dashboard:** speed, gear, RPM, fuel or charge and power state as readouts for the HUD and for gauges in the
  cockpit (needles are rigged pivots).
- **Assists**, under the advanced controls: anti-lock brakes, traction control, stability, auto-reverse when braking
  from a stop, auto-upright.
- **Optional systems**, off by default: fuel or charge, damage and breaking off (with `health`), towing and trailers
  (with `assemblies`' joints), doors and bonnets that open, turrets.
- **Feel and access:** gamepad rumble beside camera rumble, a game-wide *Reduce camera shake* setting, hold or
  toggle for every held action, and auto-accelerate as an accessibility choice.
- **Plumbing:** starter vehicles (a buggy, a truck, a tank, a hover sled and a tram), placing a vehicle in the Level,
  instances, NPC drivers along a pen route, multiplayer-safe state, distant vehicles simulated more simply, the MCP
  tools, scripted tests and performance numbers.

---

## 3. Where it lives in the editor

The **Vehicles** tab (lime frame, violet accent; icon `car`), as the board draws it:

- **The stage:** the vehicle on a test patch with a short loop of road, a ramp, a bump strip and a mud and ice
  patch. *Drive it* (the `` ` `` key) drives it right there with the game's controls; *Reset* (R) puts it back;
  *Orbit* with a middle-drag. *Test path* picks the patch. Leaving Drive it puts everything back, as test mode does.
- **The dock:** *Inspect*, the *Garage* (the project's vehicle files and the starters; "+ New" copies one) and
  *Assets*.
- **Kinds:** a row above the topics: **Ground** (this spec) · **Air** · **Water** · **Space**. Air, Water and Space
  show as "Coming later" until they are built (as the Effects tab does with topics that wait).
- **Topics** (the notch), for Ground: **Body · Drive · Power · Controls · Seats · Camera**. The board shows Body,
  Drive, Seats and Camera; Power and Controls are new topics, built from the same pieces (`docs/ui/HANDOFF.md`
  checklists).
- **Tools:** the same kit as everywhere: Place (V), Brush (B), Pen (P), Fill (G), Pick (I). In this tab they work in
  the vehicle's own frame, so what they lay moves with it.
- **Getting here:** double-click a placed vehicle in the Level, right-click *Edit in Vehicles*, or double-click its
  file in Assets; the back chip returns.

Each topic's Inspect groups use word sliders first and keep the numbers in a collapsed **Advanced** group, as the
board shows (Top speed: Slow · Brisk · Fast · Blistering; Pickup: Gentle · Punchy · Instant; Grip: Icy · Loose ·
Firm · Glued; Weight: Light · Medium · Heavy).

---

## 4. The vehicle file

`vehicles/NAME.bev` (RON, plain text, like characters and sounds), loaded with the project and saved beside it. Undo
covers it the way it covers the level (each open file keeps its own undo).

```text
Vehicle {
  format, name, kind: Ground,
  parts: [Part],            // the body, in the vehicle's own frame; the same Part type as the level, with parents
  sockets: [Socket],        // wheels, seats, exits, lights, exhausts, cameras, the tow hitch (from `sockets`)
  body:     Body,           // weight, centre of mass, drag, downforce, paint slots
  drive:    Drive,          // moves on, axles and wheels, steering, grip, suspension, engine and gears
  power:    Power,          // power states, start time, what each state turns on
  controls: Controls,       // scheme presets, or the advanced action table
  seats:    Seats,
  camera:   VehicleCamera,
  rig:      [RigLink],      // pivots driven by channels (see Rigging)
  signals:  [Signal],       // lights, exhausts, rumble and sounds and what drives each
}
```

Every setting struct carries `Setting` attributes like the rest of Beverly (`settings.rs`), so the panels, the MCP
server and later the timeline get them for free. Unset settings are left out of the file (`serde(default)` and skip
when default), as materials and foliage do.

**Placing it in the Level:** a part of kind *Vehicle* names its file (`Part.vehicle: Option<String>`, like `model`).
Placed copies of one file are instances; *Make Unique* copies the file. The Level shows the body; play builds it from
the file. A vehicle placed in the Level is empty and still until someone gets in.

---

## 5. What each part of the list means

### Body: grouping, colours, decals

- **Grouping** (already built as the part hierarchy: `Part.parent`, *Attached to*, the nested scene list, followed
  live). What is missing is an **empty group part** (a *Pivot*: no shape, just a frame with a small cross drawn in
  the editor). Wheels, doors, the steering column and turrets each hang off a pivot, which is what the rig turns.
  Ctrl+G groups the selection under a new pivot; Ctrl+Shift+G ungroups. **Built on main or in the vehicle session**
  (small, shared with prefabs and the Level).
- **Basic colours** **+ paint slots:** parts take a colour as today, or a **paint slot** (*Primary*, *Secondary*,
  *Trim*, *Glass*, *Lights*). Changing a slot recolours every part on it. A placed vehicle can override its slots,
  and a game can let the player choose them.
- **Decals with the brush and pen:** the brush paints material layers onto body parts, and the pen lays strips
  (racing stripes, pinstripes) and fills (a door panel in another colour). Both use today's paint system on parts.
  Their strokes are in the part's own frame, so they move with the vehicle. Two gaps:
  1. **Models aren't paintable yet** (the brush paints basic shapes). Vehicles made from models need model painting
     first, or they get decals only.
  2. **Image stickers** (a logo, a number, the user's own images) are projected decals from the `decals` feature,
     placed with Place onto a surface and kept as a sticker on the part (image, size, turn, where). Bevy 0.20 has
     decals built in.

### Drive: moving on wheels, tracks, hover or rails

- **Physics model (wheels):** a ray, or a short shape cast, from each wheel's socket downward. The suspension is a
  spring and a damper along it. The tyre makes a sideways and a forward force from how much it is slipping, on a
  simple slip curve (rises to a peak, then falls off to a slide). The forces are applied to the vehicle's one Avian
  body at the contact points. This is what PhysX, Unity and most arcade controllers do. It is cheap, steady at speed,
  and easy to tune by feel. No physical wheel bodies.
- **The body:** one Avian dynamic body for the whole vehicle (`assemblies`: mass, centre of mass and inertia worked
  out from its parts, updated when parts come off). The centre of mass is a handle in the stage you can drag down to
  make it harder to roll.
- **Grip:** a word slider for the whole vehicle (Icy, Loose, Firm, Glued), per axle under Advanced (front and rear
  grip set how it understeers or oversteers). **+ Grip by surface:** the part or material under the wheel scales grip
  and adds drag (Slippery tag, ice, mud, sand, water). The table lives with the level's surfaces, so every vehicle
  feels the same ice.
- **Steering:** which axles steer (*No*, *Yes*, *Opposite*), the most it turns, how quickly it turns and returns,
  and **less steering at speed** (on by default). For tracks, turning slows one track or runs it backward. Hover turns
  with a yaw thruster. Rails pick the branch at junctions.
- **Suspension**, per axle: *Ride height*, *Travel*, *Stiffness* (Soft, Medium, Firm), *Damping* (Bouncy,
  Settled, Stiff) and *Anti-roll* (how much it leans in turns). The wheel meshes move up and down with it, and the
  stage shows the springs while you tune.
- **Engine and gears** (the *Speed* group):
  - *Top speed* and *Pickup* are the words.
  - Under Advanced, a power curve over RPM is drawn as a curve, not a table.
  - Gears: automatic by default, with how many, the shift points and the reverse speed. Manual is a choice under
    advanced controls.
  - **+ Driven wheels:** front, rear, or all with a front and rear split.
  - **+ Brakes:** brake strength, a front and rear bias, engine braking and a handbrake that locks the rear wheels.
  - **+ Boost** (off by default): strength, how long it lasts, and how it refills (by time, by drifting, by pickups).
- **Speed control:** *Top speed* (the most it can do), a *Speed limit* switch the player can hold or toggle, *Cruise*
  (hold the current speed) and a reverse speed limit. These are separate from the throttle scheme: a scheme says how
  input becomes throttle, and speed control caps or holds the result.
- **+ In the air:** *Air control* (how much steering and throttle tilt it while airborne, from None to Stunt), a
  landing that compresses the suspension, and *Reset when flipped* (after a few seconds on its roof, or on a key: it
  lifts and turns upright).

### Power: off, accessory, on

A small state machine. Each change is an **event** that sounds, lights, exhausts, readouts and wires can follow:

| State | What it means | Default effects |
|---|---|---|
| **Off** | Nothing runs. It still rolls and can be pushed. | All lights off except those set to *Always*; no exhaust; no rumble. |
| **Accessory** (optional) | Electrics on, engine off. Turned on per vehicle; skipped when off. | Interior and dashboard lights, the radio or music, readouts and gauges awake; no throttle. |
| **Starting** | A short crank (*Start time*: Instant, Quick, Slow). Can fail when out of fuel or charge. | Start sound, a cough of exhaust, a rumble kick. |
| **On** | Engine running. | Idle exhaust, idle rumble, headlights if the switch is on, everything else. |

- **Keys:** one *Power* action steps through Off, Accessory and On. Tap it to go up one state; hold it to switch off.
- **Entering:** the driver getting in can power it on (*Start when entered*: on by default, off for "find the keys"
  games).
- **Wiring:** inputs *Power on*, *Power off* and *Accessory*; outputs *Started*, *Stopped* and *Stalled*. A gate can
  switch on a tram.
- **+ Fuel or charge** (off by default): it runs down with throttle, the vehicle stalls at zero, and the readout shows
  it.

### Lights, inside and out

Lights are today's Point and Spot light parts, attached to the body at sockets. Each light, and each glowing lens
part (an emissive material), has a **Driven by** choice:

- **Exterior:** *Headlights* (a switch with low and high beam), *Tail lights* (with power), *Brake* (on while
  braking), *Reverse* (while reversing), *Indicator left* and *Indicator right* (blinking), *Hazards*,
  *Fog lights*, *Beacon* and *Siren* (turning or flashing patterns).
- **Interior:** *Dome* (on with Accessory and when a door is open), *Dashboard glow* (Accessory and On).
- **Any:** *Always*, *Power state*, or a wire input.

Following the lighting decisions in `docs/architecture.md`, headlights get a short shadow range and a small map, and
only the nearest vehicles cast headlight shadows. Brake, indicator and interior lights have no shadows by default.
They count toward the light budget meter.

### Exhaust plumes

- **Where:** an *Exhaust* socket (one or more) with a particle effect from `particles`: soft puffs with drag, rising
  and fading.
- **Driven by power and throttle:**
  - Off: nothing.
  - Starting: one dark cough.
  - On at idle: a thin, slow stream.
  - With throttle: more puffs, bigger, faster and darker, scaled by RPM.
  - Letting go at high RPM: an optional backfire pop (sound and a flash).
- **Settings:** *Amount*, *Colour* (clean to sooty), *Heat shimmer* (later, with `post`).
- **Other emitters:** the same effect slot drives a hover's down-draft dust and a tank's track dust.
- **If particles aren't built yet:** the vehicle session builds the plume as data and events, and draws nothing
  until `particles` exists. See *How it plugs in*.

### Camera rumble (and gamepad rumble)

- **Throttle rumble:** a small, fast shake of the camera whose strength follows throttle and RPM. *Idle* sets how much
  it trembles standing still; *Strength* sets how much at full throttle. Felt only by the driver's camera.
- **+ Bumps and crashes:** suspension hits and impacts add short kicks in the direction of the hit. *Boost* adds a
  kick and a field-of-view widening.
- **+ Gamepad rumble:** the same signals drive the gamepad motors (with `gamepad`). Low motor for the engine, high
  motor for bumps.
- **+ Reduce camera shake:** a game-wide accessibility setting scales all of it, down to off. Shake never moves the
  vehicle or changes play; it is an effect that reacts to events (the multiplayer rule).

### Rigging

One mechanism for everything that moves on the body: a **rig link** ties a pivot to a **channel** with a range.

- **Channels:** wheel spin, steer angle, a wheel's suspension compression, speed, throttle, brake, RPM, gear, power
  state, a door's open amount, a turret's aim, and any readout.
- **What it does:** turn about an axis, slide along one, or scale, between two ends. Optionally eased with a spring
  (the Motion kit's springs).
- **Made automatically when a wheel socket is added:** spin, steer and compression links for that wheel. The
  steering wheel's link follows steer.
- **Made by hand:** a speedometer needle (speed, from 0 to 270°), a wiper, a radar dish spinning with power on.
- **Doors and bonnets:** pivots with an open angle, driven by *Enter*, *Exit* or a wire. Getting in opens the
  driver's door if the seat says so.

This is separate from `joints`, which rigs characters; legged vehicles will use `joints` for the legs and rig links
for everything else.

### Controls: basic, with an advanced toggle

- **Basic** (the default view): a *Scheme* chooser with presets, each showing its keys:
  - *Arcade* (WASD)
  - *Racing* (analog triggers and stick)
  - *Tank* (W and S for the left track, I and K for the right, or two sticks)
  - *Auto-forward*
  - *Lever throttle*
  - *Rail* (throttle and brake only)
  - *Point and click* (later)

  Underneath, three word sliders: *Steering feel* (Lazy, Normal, Twitchy), *Throttle feel* (Gentle, Normal,
  Instant) and *Assists* (Lots, Some, None).
- **Advanced** (a switch at the top of the Controls topic): the action table.
  - Every vehicle action (Throttle, Brake, Steer, Handbrake, Boost, Horn, Power, Lights, Indicators, Change camera,
    Look, Enter and Leave, Gear up and down, Cruise, Reset) with its keys and gamepad bindings.
  - Per axis: dead zone, response curve and ramp up and down times (how keys become a smooth axis).
  - Hold or toggle for held actions.
  - Each assist on its own: anti-lock brakes, traction control, stability, auto-reverse, auto-upright, steering
    help at speed.
  - Manual gears.
- **Throttle schemes**, in full:
  - *Hold to go*: keys ramp toward full throttle; analog sticks and triggers are taken as they are.
  - *Auto-forward*: there is always a set speed; W raises it and S lowers it.
  - *Lever*: the throttle stays set. Shift and Ctrl step it, Z sets full and X sets zero. W and S may nudge it.
  - *Cruise*: hold a speed, set with a key.
  - Separate from all of these, *S* brakes, then reverses once stopped. *Brake to reverse* can be turned off, which
    gives reverse its own key.
- **Steering schemes:**
  - *Keys and stick*: A and D steer, ramped.
  - *Mouse steer*: steering follows the mouse across the screen, with a dead zone in the middle.
  - *Camera-relative*: push the stick toward a direction and the vehicle turns toward it (good for top-down games).
  - *Point and click*: later, with `navmesh`.
- **Actions:** all of these go through `input-map` named actions, so rebinding, gamepads, scripted tests, NPC
  drivers and multiplayer use the same path. A driven vehicle is a control target (`control-target`): the
  vehicle's action set replaces the walker's while seated.

### Seats, getting in and out

- **Sockets:** a driver seat and any passenger seats.
- **Each seat has:**
  - an *Enter from* spot and reach (walk up, press E);
  - an *Exit to* spot (with a fallback if it is blocked);
  - whether it can shoot (later, with `weapons`);
  - which camera it uses.
- **The driver's seat** hands over control (`control-target`). The others keep their own camera and look controls
  (`mounts`).
- **When the driver leaves:** the vehicle keeps rolling, with the handbrake on unless *Rolls away when left* is on.
  It powers off unless *Leave running* is on.
- **Standing on it:** a walkable floor inside or on top (a bus aisle, a truck bed) is the part hierarchy's
  frame-relative walking, already built.
- **Being hit:** characters hit by a vehicle are pushed (physics). Hurting them comes with `health`.

### Cameras

- **Kinds:**
  - *Chase*: distance, height, lag, how far it looks ahead into turns, and field of view widening with speed.
  - *Bonnet*.
  - *Cockpit*: a camera socket at the driver's eyes, which looks through the interior.
  - *Free look*: hold the middle mouse button or move the right stick; it re-centres after a moment.
- **Switching:** a *Change camera* action (C by default) goes through the kinds the vehicle allows.
- **Per game:** the game chooses which kinds exist and the one it starts in. Nothing is hardwired (the `char-camera`
  rule).
- **On the stage:** the board's *Chase camera* part is the chase camera's socket, shown as a wire pyramid.

### Sounds (through the Audio tab's hooks)

- **Engine:** a loop, or a few loops crossfaded, with pitch and volume following RPM and load.
- **Events:** power events (start, stop, stall); horn; skids (volume by slip); impacts (by force); gear shifts;
  suspension thumps; indicator ticks; doors.
- **Where they go:** each vehicle gets a *Sounds* group naming the sound file for each event, the same pattern as
  parts and characters. Surface sets apply to tyres (gravel and tarmac rolling sounds).

### NPC drivers and traffic (later chunk)

- **Following a route:** a vehicle follows a pen curve, with speed limits along it, stops at wire-driven lights and
  keeps a gap to the vehicle ahead.
- **Brains:** a `Brain` takes over later. The driver sends the same actions a player would, so nothing about the
  vehicle changes.

---

## 6. MCP (what the server gets)

Most of it comes free from the setting descriptions. On top of that:

| Tool | What it does |
|---|---|
| `list_vehicles` | The project's vehicle files and the starters. |
| `get_vehicle` | Every topic's settings, sockets, rig links, signals and the parts. |
| `new_vehicle` | Make a vehicle from a starter or a copy of another. |
| `change_vehicle` | Change settings by label, as `change_part` does. |
| `add_wheel`, `add_seat`, `add_light`, `add_exhaust` | Sockets at positions, with mirroring. |
| `rig` | Make, change or remove a rig link. |
| `add_part` with kind *Vehicle* | Place a vehicle in the Level. |
| `drive` | While playing: set the vehicle's actions for a while (throttle, steer, brake), for checks. |
| `get_play` | Gains the driven vehicle's state: speed, gear, RPM, power state, each wheel's contact, compression and slip, and what is under it. |

Named editor actions: `vehicles.drive_it`, `vehicles.reset`, `vehicles.new`, `vehicles.mirror`.

---

## 7. Checks

- **Library tests:**
  - suspension settles at its ride height;
  - a car under full throttle reaches its top speed and no more;
  - the front wheels steer, and *Opposite* steers the rear ones the other way;
  - grip on ice is lower;
  - the power states step and send their events;
  - throttle schemes turn keys into the right throttle;
  - the file round trips.
- **Scripted tests:** each starter vehicle drives a set path on the fixed clock and checks its position (`tests/buggy`,
  `tests/tank`, `tests/hover`, `tests/tram`), with screenshots, in the editor's player and the game player.
  `scripted.rs` gains steps for entering, leaving and the vehicle's position and speed.
- **The MCP check:** `tests/mcp/check.py` gains a vehicle block: place one, drive it, read its speed, undo.
- **Performance:** each test's `perf:` line, plus one with 50 vehicles driving (distant ones simulated with fewer rays
  and no sound) to measure what the vehicle simulation costs per frame.

---

## 8. Build order (session-sized chunks)

The `vehicles` entry in `editor/features.json` is XL. These chunks replace its four:

1. **The file and the tab:**
   - the vehicle file;
   - the Vehicles tab, with kinds and topics, the Garage, the stage and Drive it;
   - the Body topic: parts, pivots, paint slots, sockets for wheels;
   - placing a vehicle in the Level;
   - the MCP basics.
2. **Wheels:** raycast suspension, tyre forces, steering, grip and grip by surface, engine, gears, brakes,
   handbrake, speed control; the Drive topic; the buggy and the truck as starters.
3. **Power and signals:** power states with accessory, lights with *Driven by*, the rig (wheels spin, steer and
   compress; needles; doors), sounds, throttle and impact camera rumble; the Power topic.
4. **Controls and seats:** schemes, the basic and advanced views, assists, entering and leaving, passengers, cameras;
   the Controls, Seats and Camera topics.
5. **Other drives:** tracks (the tank), hover (the hover sled), rails (the tram).
6. **Livery and effects:** brush and pen decals on the body, image stickers, exhaust plumes, dust and skid marks,
   boost, air control, reset when flipped.
7. **Later:**
   - point and click (needs `navmesh`);
   - NPC drivers along routes;
   - fuel, damage and trailers;
   - legs and legs with wheels (need `joints`).

About 7 to 10 sessions for chunks 1 to 6.

### What must exist on main first

| Needs | Status | Why | If it isn't there yet |
|---|---|---|---|
| `input-map` (named actions) | planned, Foundations | Every control. | **Build on main first** (it is next in Foundations anyway). |
| `sockets` | planned, Foundations | Wheels, seats, lights, exhausts, cameras. | **Build on main first.** |
| `control-target` | planned, Foundations | Driving takes over the controls. | Build on main first, or the vehicle session writes the handover behind a seam and main finishes it. |
| `assemblies` | planned, Build and ride | One body from parts, mass and centre of mass. | Chunk 1 of assemblies is small; the vehicle session may build it on main's terms (it is shared with ships and building). |
| A pivot (empty group part) | not planned yet | Wheel and door pivots. | Small; build on main or in chunk 1. |
| `particles` | planned | Exhaust, dust. | Plumes as data and events until it exists. |
| `decals` | planned (S) | Image stickers and skid marks. | Stickers wait; brush and pen livery works now on basic shapes. |
| Model painting | not planned yet | Livery on model bodies. | Livery on basic-shape bodies only until it exists. |
| `char-modes` | listed as a need of `vehicles` today | — | **Drop it as a need:** a vehicle isn't a character. "A vehicle as a character" is covered by control handover. |

---

## 9. How it plugs in (proposed; the user decides)

Built the way Tessera is, so main never breaks:

- **A branch, `vehicles`**, merged into main part by part after review (each finished chunk). CLAUDE.md needs one
  line adding it as the second agreed exception to "main only".
- **Its own folders:**
  - `editor/src/vehicle/` for what plays: the file, the physics, power, signals and rig, used by both the editor and
    the game player;
  - `editor/src/editor/vehicles/` for the tab and its panels;
  - `tests/<starter>/` for the test projects;
  - `assets` stay the user's own.
- **Few touch points in shared files, each listed in the merge note:**
  - `editor.rs`: the tab's systems;
  - `ui/chrome.rs`: enable the tab and its topics, and drop it from `TAB_LATER`;
  - `level.rs`: `Part.vehicle`;
  - `player.rs`: spawn vehicles;
  - `play.rs`: the control target;
  - `mcp/host.rs` and `protocol.rs`: the new tools;
  - `scripted.rs`: the new steps.
- **Every merge keeps main's behaviour:** all checks pass (`cargo test`, `tools/run-tests.sh`, the MCP check); levels
  without vehicles load and play exactly as before; the tab stays "Vehicles come after characters" until chunk 1 is
  merged.
- **The roadmap:** the branch edits only the `vehicles` entry's note; main runs `tools/roadmap.py` after each merge.
- **Taste calls** go to the user, not guessed. In an interactive session that means asking; in unattended runs they
  go in a JOBS list, as Tessera's do.
- **Follows CLAUDE.md:** the redesign's look, vector-only editor UI, the Motion kit, named actions, a fixed step,
  effects that react to events, file sizes and splits reported, no model names in files.

---

## 10. Building vehicles inside a game (later, kept possible now)

This spec is for making vehicles in Beverly's editor. Some games will want players to build their own (Trailmakers,
Besiege, Kerbal Space Program, Gates of Heaven's ship desk). Nothing extra is built now, but these rules keep that
open:

- **A vehicle is parts on sockets, and its stats come from the parts.** Weight, centre of mass, grip, top speed and
  seats are worked out from what is attached (a wheel part brings its wheel, an engine part its power, a seat its
  seat). The vehicle file only stores tuning on top. Building in a game is then attaching and removing parts, and the
  vehicle updates itself (`assemblies` already plans mass that updates when parts come and go).
- **The working-out lives in the runtime (`editor/src/vehicle/`), never in the tab's code.** The game player has
  everything that turns parts into a driving vehicle, so a game can do it while playing.
- **Placing pieces in a game uses `build-mode`.** That means the editor's own placement code, snapping to sockets,
  green and red feedback, costs, and saving as runtime edits. A built vehicle is saved as a vehicle file in the
  player's save, the same format.
- **Kits:** a game names which parts players may use (wheels, engines, seats, armour) and the rules for what fits
  where (socket types), through `sockets` and build kits.
- **The stage:** the Vehicles tab's stage and Drive it work like a garage in a game. A game's building screen can
  show the same live stats and a test drive.

So a later `vehicle-building` feature is mostly `build-mode` plus a kit and a screen, not a second vehicle system.

### Cargo racks

Cargo is the same node system as everything else (user, 2026-11-24): parts snap together at nodes, which are
Beverly's `sockets`. Gates of Heaven authors them as 0.25 m cubes named "Node" in Prism.

- **A rack** (a roof basket, a truck bed, a hold) is a part with a grid of **cargo nodes** on it.
- **A cargo item** (a crate, a barrel, an ore pod, a robot) is a part with a node of its own.
- **Loading** is snapping the item's node to a free rack node, the same snap as building. The node's type says what
  fits (any, or crate, barrel, ore, robot). The rack's *capacity* caps the weight.
- **Snapped cargo** is attached: it adds its weight, so a loaded truck rolls more and brakes longer, because
  `assemblies` updates the mass and centre of mass.
- **Unsnapped cargo** is just a loose part sitting in the bed. It can slide and fall out on a hard turn or a jump.
  Whether cargo snaps or lies loose is the rack's *Holds cargo* choice: *Snapped* or *Loose*.
- **In play:** a character carries a thing to a rack and presses E (it snaps to the nearest free node). Wires,
  modules and building do the same through the node. Unloading unsnaps it at the rack's *Unload to* spot.
- **Inventory:** `inventory`, when it comes, can list a rack's contents.
- **Events and readouts:** *Loaded*, *Unloaded*, *Fell out* and *Overloaded*; readouts for the load and how full it
  is (Gates of Heaven's cargo readout is the same thing).
- **Why no separate cargo system:** racks, attachments, upgrades that bolt on, and vehicles built in a game are all
  parts meeting at nodes.
- **When:** snapped racks come with chunk 3; loose cargo falling out comes with chunk 6.

## 10b. Upgrades in a game (later, architected now)

Players buying better engines, tyres, armour or paint during a game. Nothing is built now, but the vehicle code is
shaped so upgrades drop in:

- **One place works out a vehicle's settings.** Nothing reads the file's settings directly. Everything goes through
  `effective(vehicle, upgrades)`: the file, then the parts on its sockets, then the upgrades applied in order. Today
  the upgrade list is always empty. Every system (physics, power, signals, the panel's live stats) reads the result,
  so an upgrade can change any setting.
- **An upgrade is data, not code:** a name, an icon, a *slot* (Engine, Tyres, Suspension, Brakes, Armour, Paint,
  Cargo, or one the game names) and a *tier*. Its changes are:
  - setting changes by label (*Top speed* +10%, *Grip* to Firm, *Capacity* +200 kg); and/or
  - a part swapped onto a socket (bigger wheels, a roof rack, a turret).

  Upgrades live in `vehicles/upgrades/*.bev` or inside the vehicle file, so one engine upgrade can fit many vehicles.
  A slot holds one upgrade at a time; tiers can require the tier below.
- **Saved as edits:** a player's vehicle is the vehicle file plus a list of upgrade names, in the player's save
  (`runtime-edits`). Upgrading never changes the authored file, and test mode throws upgrades away.
- **Later, in the editor:** an *Upgrades* topic in the Vehicles tab lists them, with costs (from `economy`) and a
  before-and-after on the stage: live stats and a test drive with the upgrade on. Building vehicles in a game
  (section 10) and upgrades share the same `effective` step: an attached part is an upgrade without a shop.
- **MCP, later:** `list_upgrades`, `new_upgrade`, `apply_upgrade` (in play).

## 11. Later tabs: Air, Water, Space

Kept as kinds in the Vehicles tab ("Coming later"), each with its own roadmap entry. They will reuse the file, the
tab, Power, signals, the rig, Controls, Seats and Camera from this spec. What each adds:

- **Air:** wings and lift, thrust, pitch, roll and yaw, stalls, landing gear, helicopters and VTOL (with `flight`'s
  manoeuvres).
- **Water:** buoyancy points, hull drag, propellers and rudders, planing, and submarines with depth, ballast and
  pressure (with `water-play`, `sailing` and `ocean`).
- **Space:** six-way thrusters, Newtonian drift with flight assist on and off, docking through sockets (with `space`;
  the Gates of Heaven controls are noted there).

The user has these designs in mind; each gets its own planning pass first.

---

## 12. Open questions for the user

1. **The branch:** OK to make `vehicles` a second exception to "main only", as Tessera is?
2. **Order:** build `input-map` and `sockets` on main before the vehicle session starts (recommended), or let the
   vehicle session build them on main's terms?
3. **Power key:** one key that steps through the states (proposed: F), or separate keys for accessory and start?
4. **Physics feel:** arcade by default (forgiving grip, assists on) with sim-like settings under Advanced, or the
   other way round? Proposed: arcade.
5. **The mech:** stays a character file (current plan), or becomes the first legged vehicle?
6. **Upgrades:** a shop screen in each game, or one shared Beverly screen that games restyle?
7. **Building in games:** which game wants it first (Gates of Heaven's ship desk, Last Ascent, another)? That decides when `vehicle-building` comes.

## Sources

- NVIDIA PhysX vehicles guide (raycast per suspension line, forces applied to one rigid body):
  https://docs.nvidia.com/gameworks/content/gameworkslibrary/physx/guide/Manual/Vehicles.html
- Unreal Chaos vehicles: torque curve and differential setup:
  https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/ChaosVehicles/FVehicleEngineConfig and
  https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/ChaosVehicles/FVehicleDifferentialConfig
- An Avian-based Bevy ground vehicle controller (Bevy 0.18; for reference, not as a dependency):
  https://julien-blanchon.github.io/saddle-vehicle-ground-vehicle/
- Raycast suspension with a slip curve (forum write-ups, anecdotal):
  https://devforum.roblox.com/t/a-vehicle-chassis-with-the-pacejka-friction-circle-raycast-suspension-gearbox/2400821
