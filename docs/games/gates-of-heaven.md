# Gates of Heaven: design notes

A game planned for Beverly once the editor is reasonably feature complete. This file records the design as it stands. Items marked **open** are not decided yet.

## Synopsis

The game's universe is full of alien life. An impossibly advanced but misguided species fired a weapon into the Milky Way's black hole. The hole now throws out matter and radiation instead of consuming it. The wave of destruction should take thousands of years to cross the galaxy, but it is speeding up past light speed.

The same civilisation built a gateway to evacuate its own people to another galaxy, having judged the Milky Way corrupted. Humans and their allies learn only that the gateway may work, and may be usable by them. Their plan is a chain of wormholes, each jump up to 1,000 light years, so that everyone along the route can evacuate too.

The game is the journey from near Sol to the home planet of the gateway's builders, before the wave wipes out galactic life. Along the way are hostile species and environments, and the knowledge that the closer the Pact gets to the gate, the closer it gets to the wave. The gate lies toward the galactic core, the same direction the wave comes from.

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

## Station

The station is in the Catapult's hangar, where the ship docks. The player walks up to the console and presses its button to open a window with three pages.

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

### Modify Ship

A button that opens the ship editor. This is the same editor as the ship design step in the loop.

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

## Open questions

1. What does the galactic map look like, and what does the player see on it before a jump?
2. Which special equipment is kept between destinations, and which is lost? (Depends on the loadout and the player's choice, so the rules for each item are still to be written.)
3. What is the far end of the gate, and what is the gateway's own system like?
