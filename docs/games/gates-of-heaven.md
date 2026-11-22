# Gates of Heaven: design notes

A game planned for Beverly once the editor is reasonably feature complete. This file records the design as it stands. Items marked **open** are not decided yet.

## Synopsis

The game's universe is full of alien life. An impossibly advanced but misguided species, the Hejich, fired a weapon into the Milky Way's black hole. The hole now throws out matter and radiation instead of consuming it. The wave of destruction should take thousands of years to cross the galaxy, but it is speeding up past light speed.

The same civilisation built a gateway to evacuate its own people to another galaxy, having judged the Milky Way corrupted. Humans and their allies learn only that the gateway may work, and may be usable by them. Their plan is a chain of wormholes, each jump up to 1,000 light years, so that everyone along the route can evacuate too.

The game is the journey from near Sol to the giant gate in orbit around the gateway's builders' home planet, before the wave wipes out galactic life. The last level is the gate itself. The player never lands on the planet. Along the way are hostile species and environments, and the knowledge that the closer the Pact gets to the gate, the closer it gets to the wave. The gate lies toward the galactic core, the same direction the wave comes from.

The **Pact** is humanity together with the alien civilisations that have joined it in trade and mutual agreement.

## Names and presentation

- **In-game name:** the **Milky Way Pact (MWP)**. It is the name the player sees on the ship, the station, the UI and the mission briefings. It is an uplifting name, and it is the coalition's own choice.
- **Historical records:** just **the Pact**, in records, timelines and plaques.
- **Scale:** the coalition is 21 species. Against the rest of the galaxy that is very small, and the name does not claim otherwise.
- **Presentation is visual first.** The Pact is not only humans. Its design choices, from ship hulls and hangar layout to signage, colour, typography and iconography, come from many species, so the look of the game carries the lore. Written text supports it.
- **Where lore appears:** plaques, historical records, decorations, and dialogue.
- **Loading screens:** short lore fragments, one at a time, such as a record, a quote, a dispatch or a fact about a species. Each fragment is a line or two, never a block of exposition. Fragments are drawn from the same records as the plaques, so they add to the world rather than repeat it.

## Background timeline

Not shown to the player directly. It informs the world design.

| Year (AD) | Event |
|---|---|
| 2130 | Climate change unites humanity under a single government, which begins colonising the solar system. |
| 2295 | Some colonies become self-sustaining and branch off from the Earth-centred government, peacefully. Ships stay the domain of the wealthy, corporations and institutions. |
| 2410 | A ring around the Sun begins sending power by relay to the farthest corners of the system. Energy use is no longer capped. Populations grow and science flourishes. |
| 2884 | The first stable artificial wormhole is made: 4 inches long, lasting 1.45 seconds before its power relay overloads. |
| 3199 | Wormhole travel inside Sol is standardised but power-hungry. Cores made at different places never pair, which limits travel to Sol. The Catapult Project starts; the first small prototype is more than a century away. |
| 3210 | Ansibles, instantaneous communicators through the manifold, are developed. The manifold is already full of unidentified transmissions, so life beyond Sol is confirmed. |
| 3255 | Signals cannot be sorted into coherent communication. The manifold becomes a last-ditch emergency channel. On 6 July a total jamming of the manifold lasts about 25 days. |
| 4182 | A multi-generation expedition, carrying a wormhole core, makes contact with the Morsulae, 12 light years from Sol. Trade and a gate follow. The Morsulae report that the galactic core is unravelling. |
| 4403 | Several alien civilisations have joined the Pact. The core's outflow has become a torrent: a plasma torus expanding outward at relativistic speed, and accelerating. The galaxy could be sterilised in about 100 years. |
| 4428 | Deep observation finds an anomaly 20,000 light years toward the core: a gateway with a high-energy trail pointing at a neighbouring galaxy. Plan: a chain of wormholes, through known and unknown space, to evacuate hundreds of systems. |
| 4696 | The new Catapult is complete. It fires a ship in a temporary warp bubble up to 1,000 light years. See Catapult below. |

## Story

The Pact's plan depends on four things: the path can be forged, the gateway's owners agree, the gateway holds, and the wave does not consume the Pact first. Each jump takes the fleet closer to the gate, and therefore closer to the wave.

The wave is there for story and urgency. It is not a timer, and the player is never timed.

## Route and galactic map

- **Hand-plotted:** the route is chosen by the designer. It is not procedural.
- **Gates already open cannot be entered** by the player.
- **Galactic map:** a map of the route, used to plot the next jump. It needs to be built as its own feature, and the level editor must be able to place each system on it.

## The loop

1. Design the ship from the parts kit.
2. Stock the ship for an unknown destination, using the Intel and Cargo pages at the station.
3. The catapult launches the ship and the wormhole core to a new system.
4. Build a base.
5. Defend the core.
6. Build robots; the robots build the gate, and the gate is self-sustaining at 25%.
7. Activate the gate. The fleet arrives, secures the system, and the catapult comes through.
8. Repeat.

**Failure branch (7B):** the ship is destroyed before the gate opens. The clone wakes in the previous system. Rebuild, relaunch, recover. There is no limit on repeated deaths. A difficulty setting that raises the cost may come later.

## Core rules

- **One core.** Exactly one core exists at a time. It is issued at a new game and after each successful gate opening. Two or more is a bug.
- **Core states:** aboard the ship (default), detached, in a bunker, or installed in the gate.
- **Core health:** the core has its own health and survives the ship's destruction.
- **Core loss is game over,** at any time, including while en route back from the previous system. The game unloads behind a black Game Over screen, which offers the main menu or loading a previous save.
- **Bunker:** makes the core invulnerable. It is priced to feel expensive early, while few robots harvest, so there is tension until it is built. It can be moved slowly by a robot worker.
- **Loose core:** enemies attack a loose core in proximity, like any other player structure, but do not hunt it at range.
- **Gate installation:** the bunker fits into a slot on the finished gate, and the core installs from there.
- **Pairing:** cores are made from spacetime as identical pairs at one point, and cores made in different places never pair. Each core must therefore be carried to its destination. This is why the Catapult carries the core.

## Catapult

The Catapult is the Pact's launch vehicle, and it is also the station.

- **One pilot and one ship at a time.** The pilot's life support needs the same surge of power that launches the ship. The substance that makes the technology work is produced by the Morsulae's bodies and cannot be synthesised, so the Morsulae supply only the modules, and the crew is limited to one.
- **Range:** up to 1,000 light years per jump. The last generation managed about 10 without killing its crew.
- **Mass:** the Catapult has a maximum mass threshold for the ship. This is the launch limit, and it bounds the cargo the ship can carry.
- **Power:** its own wormhole shunt collects plasma from a star's surface, so each jump needs a recharge. The recharge timer matters most for 7B.
- **Hangar:** the ship docks in a hangar inside the Catapult between jumps and during refit. After a successful gate opening, the Catapult comes through and the ship docks there again.
- **Pilot memory:** the pilot's memory is streamed back through the shunt to the cloning pod in the previous system. This is flavour only, with no mechanics.
- **Upgrade (last five levels):** the Catapult can be upgraded to carry more cargo and modules, to launch up to five backup ships, or both. Backup ships are AI-controlled and commanded like an RTS. They launch staggered about ten minutes apart.

## Station

The station is in the Catapult's hangar, where the ship docks. It is a **holographic desk**: the player walks up to it and the view settles looking straight ahead over it, with the desk's surface below. There are no menu windows. Everything is in the world (owner, 2026-11-22).

- **Three physical buttons** at the bottom left of the desk's surface: **Intel**, **Cargo** and **Modify Ship**.
- **The surface** also shows the available resources, parts, modules and so on.
- **The galactic map** is a 3D hologram above the desk.
- **Intel** is a 2D map projected above the desk.
- **The ship editor** is in 3D above the desk.
- **The look:** parts, modules and cargo are the literal ship parts, with just a tiny bit of hologram outline and the occasional flicker.

### Intel

A map of the system, built from probe and telescope data, or from data the Pact has bought from friendly species. It is only Intel: it does not say where each piece of data came from.

- **Top-down by default.** Orbit, pan and zoom to switch to a 3D view.
- **Partial data.** The map does not show everything. Major bodies are always shown, since they cannot usually be hidden. Smaller bodies can be hidden behind larger ones on the far side of an orbit. That is a plot hook for a later level.
- **Authoring dependency:** resources and known enemy base locations are placed by hand in the level editor. The Intel page cannot be built until the editor can place them.

### Cargo

An inventory of the ship's holds.

- **Removing** cargo, modules or weapons refunds them in full, with no penalty.
- **Adding** cargo, modules or weapons costs units from the logistics budget. Units are not a private currency, and nothing can be sold for profit. Cargo is only bought.
- Cargo space is the constraint that forces the loadout decisions below.

### Modify Ship (the ship editor)

The same editor as the ship design step in the loop.

- **Where:** at the hangar's holo desk, or at the holo desk inside the ship (in the holo desk room, a Habitation part). The player can modify the ship from inside the ship.
- **Building happens only at holo desks** (owner, 2026-11-22): the ship here, base tiles at the starter base's desk. Nothing is moved around in the 3D world; that may come later.

- **The parts strip:** parts and modules sit in a single scrollable row on the right of the desk's surface, with the categories below the strip.
  - Hovering over a part pops it up in 3D above the desk, slowly spinning.
  - Clicking a part puts the 3D part on the cursor, to place on the ship.
- **Modes, while a part is highlighted.** A part stays highlighted just after it is placed, so the player can change it straight away or switch modes:
  - **1:** toggles between node snap and surface attach;
  - **2:** translate;
  - **3:** rotate.
- **Camera:** the ship editor and the base editor turn (rotate), zoom and pan with the mouse.
- **Dragging:** a part can be dragged elsewhere on the ship, or out into the build space, which works as scratch space.
- **Leaving the editor:**
  - Parts that are not connected are refunded, with no questions.
  - If a mission-critical part is missing, a pop-up says so.
- **X-ray mode** shows the interior passages.
- **The parts list** needs no mask in this game (Beverly's lists can be masked into other shapes).
- **Systems readout:** bar graphs in the upper right corner. They show fuel and uptime, power and draw, how long life support lasts, and cargo space and how full it is. Some modules come in several versions, and some modules do the same thing with different specs, so the readout is how the player compares them.
- **Cargo containers filled with Resource Units** come out of the same overall budget. The player has to fill them last, or they can't buy more ship parts unless a container is refunded.

### Parts

- **Nodes** connect some pieces, such as those with interiors. A node is authored in Prism as a 0.25 m cube named "Node". The player has to care about which way a part is rotated, even with nodes.
- **Surface attach** is for other parts, such as decoration or weapons. The attachment point is the part's origin, for simplicity.
- **No strict rule** like KSP's decides which parts use nodes and which surface attach: it is whatever the owner authors.
- **Marking parts:** assets that are in-game editor parts are marked with special tags in Beverly.
- **Categories for now:**
  - Mission Critical (its own category in the strip)
  - Habitation (includes parts with interiors, such as the cockpit and the holo desk room)
  - Power
  - Thrust
  - Life Support
  - Shields
  - Fuel
  - Structural
  - Weapons
  - Base
  - Robots
  - Accessories (Module Rack, lights, batteries, landing pads, skids and legs, ladders, surface-mounted tanks, cargo bays that hold robots or other deployables in a protected area, and so on)
  - Modules (special items such as the winches and booster packs, and cargo containers of different sizes for Resource Units, to help jump-start base building)

### Probe

A special module, one-time use and probably small. Whether it takes up a robot slot is **open**.

- It pathfinds its way out from the ship to about 50 metres, then fires its main thruster toward the point the player picks on the map.
- It has no maximum range, so it uses no fuel and no power.

### The hangar monitor

The news anchor on the monitor in the hangar kitchen is an audio-only recording with a waveform on the screen. The screen is an asset, not a menu screen. It is a live background element, not a cinematic: ambience.

## Power, fuel and life support

- **Fuel:** tanks hold fuel; thrusters and power sources use it.
- **Power:** weapons, turrets and special modules explicitly don't use power. Life support and shields do.
- **Shields:** the ship needs shield modules, and they use power. 
- **Shield projectors:** the bombardment and anti-radiation shield projectors carried as cargo have their own set damage bars, separate from power. They draw only a minimal basic level of power.
- **No flow:** there is no physical fuel flow from a tank through pipes to an engine or power source. Each system simply draws from the totals of the ship's parts, as the readout shows.
- **Space suit:** the player's suit holds **30 minutes of air** (may be tweaked). It should be comfortable enough that the player isn't in a hurry outside the ship, but not long enough to forget a life support module in the ship.

## Resources

There are only two, and the player handles only one.

- **Resource Units:**
  - Mined directly by robots. (They may have been called Resource Credits earlier.)
  - They pay for ship parts, base modules, and units, ammo and other things made in the bases' manufacturing plants.
- **The gate resource:** a special resource that only the robots can harvest, for the gate.

## Landing pads

- **Refilling:** landing at a starter base's robot pad refills the ship to its maximum, at a medium pace.
- **Robots wait:** a robot coming to drop off has to loiter until the pad is clear.
- **More pads** ease that, depending on how many mining robots the player has built.
- **Dropoffs** go to bases or to the player's ship. Either way they show up in the UI, all invisibly tied together, as in other RTS games.

## Bases

- **The starter base module:**
  - The player gives it a basic move command from the ship. It undocks quickly, flies to the point the player indicates and deploys there. This may need pathfinding.
  - **Range:** a neutral circle from the ship to the cursor, which turns red when the point is outside the module's range.
  - Once deployed it can't be packed up again.
  - It can receive robots for resource dropoff.
- **Hex tiles:**
  - The starter base is hex shaped, with a ring of open hex slots around it for new base tiles.
  - Every new tile has the same ring of available slots, unless a slot is already taken.
- **The base's holo desk:** base tiles are chosen from a holographic desk inside the starter base. Around level 5, a networked remote connection may let players reach any base from the holo desk inside their ship.
- **Base tiles:**
  - Power Plant
  - Connector (an extension two tiles wide that connects only at its ends)
  - Ammo and missile plant
  - Solar power plant
  - Shield generator
  - Turret emplacement
  - Landing pad
  - Robot plant
  - Missile emplacement
  - Sensor emplacement
  - Drone plant
  - Ship module plant
- **Interiors:** base parts have little interior, mostly just the starter base. The rest are open to space.

## Refit and restocking

- **Ammo is automatic.** Any weapon with limited ammo is replenished automatically in the hangar.
- **Weapon types:** most weapons are energy, plasma or missile.

## Loadout decisions

Each level sets a stake on cargo space, and the player decides what to bring before launch.

- **Radiation level:** a large anti-radiation shield projector protects the ship. It takes cargo space, so the ship carries fewer robots.
- **Enemy-fleet level:** a bombardment shield projector with a finite capacity, large enough to clear the arrival area once. The player fills the rest of the hold with gun drones or slow turret emplacements that clear the area while the ship is sheltered. One or two robots can then start harvesting.
- **Crystal sheet level:** the reels and pins occupy cargo slots, and the sheets are the terrain. See below.
- **Special equipment:** what carries between destinations depends on what is left on the ship after a level, and on what the player chooses to keep.

## Piloting

- **Direct control:** the player flies the ship directly.
- **Cockpit view by default.** F4 toggles an optional third-person camera while flying.

## Scale and bases

Levels are at the scale of *Homeworld*: small. Planets can appear in the background, but play happens around very small moons, asteroids and similar bodies. Bases are built on those, and on crystal sheets where a level has them.

## Crystal sheets

A level where the arrival zone is full of large black crystalline mineral sheets. They are shiny, not transparent, and about a dozen times the size of the ship. Their outlines are rough polygons (parallelograms, hexagons, octagons), made messier and more natural.

- **Mining:** sheets can be mined for resources.
- **Surfaces:** bases and weapon emplacements can be built on sheets.
- **Blocking:** sheets block enemy patrol scans, which work by line of sight.
- **Movement:** sheets move very slowly, and have no recoil or wobble that could oscillate into chaos. Stitched sheets settle rather than oscillate.
- **Hiding:** the player stitches sheets together into hiding places such as a shell or a ring.

### Carbon Lattice Reel

A winch bundle of thick metal beams attached to a cargo slot, with a barrel on one end. The player points the ship's reticle at a point on a sheet and fires, then points at a second sheet and fires. The reel pulls the two points together at a **fixed speed**. Sheet mass does not change the speed.

The reel reuses the winch behaviour planned for Last Ascent.

### Grav Pin

An anchor weapon. It fires an energy bubble that anchors a sheet in space around a point that can rotate.

- **Left click** fires a pin at a sheet.
- **Right click** collects a pin back.
- **Sheets only:** pins can only be used on sheets.
- **Storage:** up to 50 pins per cargo slot.
- **Reuse:** pins can be reused indefinitely.

Pins give a way to anchor one edge of a sheet, and the reel gives a predictable way to stitch the other edge. Pairs of pins will probably be used as hinges. Whether the whole idea is viable is to be tested later.

## Enemies

- **Patrols:** heavy enemy patrols in the crystal sheet level. Their scans are line of sight, so sheets block them.
- **Cooperative engagement:** units communicate over short range. When one unit spots the player's ship or base, the squad knows.
- **Teleporting units:** a small ship that can teleport short distances. It can jump inside a shell after the squad has spotted what is inside, so hidden bases are a real threat, not just a hiding place.
- **Base waves:** many levels include waves that target the player's base.
- **Growth while away:** enemies build bases and defences while the player is absent, including during 7B.
- **Engagement is optional.** Enemy units can be left alone with no penalty. The main objective is usually to open the gate, unless the plot demands more.

## Save system

Saves are `.bev` files, plain text, the same kind of file as the rest of a Beverly project. That keeps saves diffable and game state as data that could later be sent over a network. Every save carries a schema version so older saves can be migrated.

**Saves can be made anywhere, including during combat.** This changes three things.

1. A mid-combat save must include everything in motion: projectiles, enemy AI timers and states, squad knowledge, sheet motion, and every pin's anchor and rotation.
2. The fixed time step is what makes that workable. Saving the tick count and the full state makes a load deterministic.
3. Game Over's "load a previous save" can choose any save, not only loop-boundary saves.

### What is saved

**Campaign (carries over)**
- Loop number and the order of destinations visited
- Which systems are secured, lost or abandoned
- Units (currency) and special equipment carried
- Catapult recharge timer, stored as elapsed simulation time rather than wall-clock time
- Core location, checked against the one-core rule on load

**Ship (carries over)**
- Design: parts, attached modules, hull damage
- Cargo contents, including pin count
- Position and velocity within the system, for a mid-system save

**Destination (per system, persists while the player is away)**
- Enemy bases, defences, unit positions and alert states
- Teleporting units, and each squad's shared knowledge of where the player was seen
- Player bases: structures, damage, and the robots working on them
- Robots: each robot's job and position
- Depleted resource deposits
- Wrecks and salvage
- Gate progress and the segments already built
- Crystal sheets: exact transforms, since they move slowly and must reload exactly
- Pins: anchor position, pivot rotation, and which sheet each is on
- Intel revealed so far, including bodies hidden by occlusion

**Generation seed (not the generated content)**
- Store the seed and loop variation, then regenerate the system. Store only the changes made to it. This keeps saves small and the per-loop variation reproducible.

**Clone and death**
- Clone location, rebuild budget, and the wreck's contents
- Game Over record: which save was current, and whether the core was lost

**Simulation clock**
- Fixed-step tick count

### When saves happen

- **Loop-boundary autosave:** on arrival, on gate opening, and on return to the station.
- **Pre-launch checkpoint:** a launch is one-way, so one is always written before the catapult fires.
- **Manual save:** anywhere, at any time.
- **Rolling history:** the last few saves are kept, so Game Over has something to offer.

Per-destination state is the largest part of a save, and it is what the 7B branch depends on. Only the core's location and its single instance are critical. Everything else can be rebuilt from the seed and the saved changes.

## Ship modules

### Booster packs (from about level 10)

- Rocket engines that attach to the ship or to objects, such as derelicts.
- Acceleration is continuous, and it builds the longer the boosters fire.
- Limited fuel.
- Introduced as a manoeuvring tool around level 10. In the finale they are used as weapons.
- **Explosive:** a booster explodes when it is destroyed.
- **Chain damage:** the explosion's damage multiplies when more than one booster is in range of another. A cluster of boosters is far more destructive than the sum of its parts. About six boosters in range of each other are enough to set off the Weapon's explosion.
## Finale

The Hejich built the gate, and then used it to leave. The finale is in their gateway system.

### The Hejich

- An impossibly advanced species, and actively hostile. They want to wipe out life in the Milky Way while they escape.
- They have chosen a dwarf galaxy with no life. By their beliefs it is pure, so they need not worry about other species.
- The player learns this over the game, in fragments: records, dialogue and wreckage.

### The gateway system

- **The Guard:** the Hejich rear guard, a force too large to beat head-on. Their civilisation has already gone through, so the Guard holds the gate alone. It is a fleet of warships of various sizes. The roster of ship types is not yet designed.
- **Logistics ships and arrays:** huge ships and machinery, built and used over decades to ferry people to the gate. Targets, and places to hide.
- **The Weapon:** the same weapon that fired into the black hole. It lies in this system, some distance from the gate, and finding it is part of the level.

### Getting past the Guard

The player must be sneaky. Each option below thins the Guard before the player engages.

1. **Booster derelict:** mount booster packs on a derelict and accelerate it into the Guard. It destroys a swath of ships.
2. **Weapon beam:** disconnect an energy shunt cable from the Weapon, hold it in a manipulator on the ship, and fire a beam of pure energy on a short tether. Recoil is strong and wild, so keeping the aim takes a lot of mouse correction. Boosters on the ship can steady it. The Guard notices the beam and homes in, but it takes a few minutes to reach the attacker, which gives time to thin them. A clever player can place turrets, shield projectors or other defences on the Weapon.
3. **Auto-turret derelict:** a derelict with many auto-turrets, boosted into the Guard.
4. **Booster bomb:** gather about six booster packs in range of each other on the Weapon, and destroy them. The chain of booster explosions sets off the Weapon's explosion. The blast destroys 60 to 90 percent of the Guard. The radius is large, and it kills whoever fires the shot, whether that is the player or a ship they command. The core is not involved.

### How the Guard responds

- Detecting hostility sends a squad to investigate the vector of the attack.
- If the squad finds the player's units, it relays their location.
- The Guard then decides how many forces to send. Against a small group, the squad fights alone.
- The Guard does not simply absorb damage.

### The choice

After the Guard is beaten, the player chooses:

- Send the Weapon through the gate, with some robots, to destroy the dwarf galaxy's black hole as revenge.
- Or do not.

### Ending

Either way, the MWP arrives, reorients the gate to target a different galaxy, and begins evacuating the known Milky Way.

The player never goes through the gate. The last level ends in orbit, where the gate is the size of a large moon.

**End cinematic:** new alien collapsible wormhole gates are fired by catapult. They expand and come to life, and fleets of transport ships fly out of them moments later. A big crescendo of music.

**Hidden media:** a news show anchor on a monitor in the Catapult's hangar kitchen (an audio recording with a waveform; see The hangar monitor) reports that the Catapult technology has been shared with the galactic network because of the impending disaster, and that new designs are starting to appear. This explains the Catapult's improvements in the later levels.

## Open questions

1. **Booster chain damage:** how does the multiplication scale with the number of boosters in range, and what is the range? About six sets off the Weapon, so that is the starting number to tune.
2. **The Guard's roster:** the warship types, sizes and counts have not been designed yet.
3. **The Guard's response:** does the Guard send a fixed force, or does it scale with how much damage it has taken?
4. What does the galactic map look like, and what does the player see on it before a jump?
5. Which special equipment is kept between destinations, and which is lost? The rules for each item are still to be written.
6. **Loop variation:** the save section stores a system's seed and "loop variation". Does a system change each time the player comes back through it, or is the seed only for generated detail such as asteroid fields?
