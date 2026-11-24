# Vehicle editor prototype (standalone HTML)

A small, playable stand-in for the Vehicles tab, so the user can feel the direction early (user, 2026-11-24). One HTML
file, built in about 3 turns, published as a private page to try in the browser. Its pieces mirror Beverly's, so it
translates one to one when the real tab is built (`docs/vehicles.md`).

## Scope: a drivable 4-wheeled car you can tune

**Turn 1: drive it**
- The page: a 3D view (three.js) and a settings panel on the right.
- Flat ground with a grid; a box body with 4 wheels.
- Physics: raycast suspension (spring and damper), tyre grip on a simple slip curve, front-wheel steering, a fixed
  60 Hz step. Its own small code, no physics library.
- Keys: W, A, S, D, Space (handbrake), R (reset).
- A plain camera behind the car.
- **Prism models** (user, 2026-11-24): *Load Prism model* takes a GLB from Prism (`docs/prism-glb.md`). Groups whose
  name, role or tag has the word *wheel* (or tyre, tire) become the wheels; a *Wheels* group holding several is opened
  up. Axles, track, wheel size, ride height and body size come from the model; the front axle steers, the back one
  drives. Collider, node, editor and lod pieces are hidden. The front is -Z as in Beverly, with *Turn around* to flip it.
  Later turns read more words the same way (lights, exhausts, seats).

**Turn 2: tune it**
- The panel is drawn from a settings table with word sliders: Top speed, Pickup, Grip, Weight, Stiffness, Damping,
  Ride height, Steering, and which axles steer.
- Changes apply live while driving.
- Save and load the vehicle as a file, plus a speed readout.

**Turn 3: power and feel**
- Power with F: Off, Accessory, On.
- Head, tail, brake and reverse lights that follow the driving.
- Throttle schemes: Hold, Auto-forward, Lever.
- Camera rumble from throttle.
- Simple exhaust puffs.
- The Basic and Advanced toggle for controls.

**Follow-ups** (only if the 3 turns run out first):
- A follow camera: chase with lag, looking into turns, wider at speed.
- Simple terrain: rolling hills, a ramp, an ice patch, sunlight and shadows.

## Wiring that carries over to Beverly

| Prototype | Beverly |
|---|---|
| The vehicle as JSON, with the field names of `docs/vehicles.md` section 4 | `vehicles/NAME.bev` (RON) |
| A settings table: label, words, low, high, unit, section, help | `Setting` attributes (auto-panels, MCP) |
| Named actions with bindings (Throttle, Brake, Steer, Handbrake, Power, Reset) | `input-map` |
| A fixed-step `step(vehicle, input, dt)` that applies forces | A `FixedUpdate` system with Avian forces |
| Events (Started, Stopped, Braking, Reversing) that lights, puffs and rumble listen to | Bevy events and signals |
| `effective(vehicle)`: the one step that works out the settings | The same step, ready for parts and upgrades |

Metres, kilograms, seconds, Y up, as in Bevy.

## Not in the prototype

Body building from parts, seats, cargo, decals, tracks, hover, rails, legs, sounds, and MCP. These come on the
`vehicles` branch.
