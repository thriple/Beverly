# Gate Loop: design notes

A game planned for Beverly once the editor is reasonably feature complete. This file records the design as it stands. Items marked **open** are not decided yet.

## Story

A government-backed freeway mission. Each system the player reaches is opened for a road through a gate, and the mission is limited only by the logistics train behind it and the resources at hand. It is not a private venture: there is no market, and spending is a logistics budget.

**Open:** the back story beyond this (who the government is, what the gate is for, what the final destination is) is not written yet.

## The loop

1. Design the ship from the parts kit.
2. Stock the ship for an unknown destination, using the Intel and Cargo pages at the station.
3. The catapult launches the ship and the wormhole core to a new system.
4. Build a base.
5. Defend the core.
6. Build robots; the robots build the gate, and the gate is self-sustaining at 25%.
7. Activate the gate. The fleet arrives, secures the system, and the catapult comes through.
8. Repeat.

**Failure branch (7B):** the ship is destroyed before the gate opens. The clone wakes in the previous system. Rebuild, relaunch, recover.

## Core rules

- **One core.** Exactly one core exists at a time. It is issued at a new game and after each successful gate opening. Two or more is a bug.
- **Core states:** aboard the ship (default), detached, in a bunker, or installed in the gate.
- **Core health:** the core has its own health and survives the ship's destruction.
- **Core loss is game over,** at any time, including while en route back from the previous system. The game unloads behind a black Game Over screen, which offers the main menu or loading a previous save.
- **Bunker:** makes the core invulnerable. It is priced to feel expensive early, while few robots harvest, so there is tension until it is built. It can be moved slowly by a robot worker.
- **Loose core:** enemies attack a loose core in proximity, like any other player structure, but do not hunt it at range.
- **Gate installation:** the bunker fits into a slot on the finished gate, and the core installs from there.

## Station

The player walks up to a physical station and interacts with it. A button on the console opens a window with three pages.

### Intel

A map of the system, built from probe and telescope data, or from data the government has bought from friendly aliens.

- **Top-down by default.** Orbit, pan and zoom to switch to a 3D view.
- **Partial data.** The map does not show everything. Major bodies are always shown, since they cannot usually be hidden. Smaller bodies can be hidden behind larger ones on the far side of an orbit. That is a plot hook for a later level.
- **Authoring dependency:** resources and known enemy base locations are placed by hand in the level editor. The Intel page cannot be built until the editor can place them.
- **Open:** whether each piece of data shows its source (probe, telescope, purchased). Leaning yes, since it makes uncertainty visible.

### Cargo

An inventory of the ship's holds.

- Remove cargo, modules or weapons for no fee.
- Add cargo, modules or weapons using units. Units are a logistics budget, not a private currency.
- Cargo space is the constraint that forces the loadout decisions below.

### Modify Ship

A button that opens the ship editor. This is the same editor as the ship design step in the loop.

## Loadout decisions

Each level sets a stake on cargo space, and the player decides what to bring before launch.

- **Radiation level:** a large anti-radiation shield projector protects the ship. It takes cargo space, so the ship carries fewer robots.
- **Enemy-fleet level:** a bombardment shield projector with a finite capacity, large enough to clear the arrival area once. The player fills the rest of the hold with gun drones or slow turret emplacements that clear the area while the ship is sheltered. One or two robots can then start harvesting.
- **Crystal sheet level:** the reels and pins occupy cargo slots, and the sheets are the terrain. See below.

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

1. Does Intel show each datum's source?
2. Does the player have any way to sell at Cargo, or only buy?
3. What is the back story and the final destination?
4. Base placement rules: planet surface, asteroid, orbital station, or combinations?
5. Which special equipment carries between destinations?
6. How does the portable space station work (instant refill, choices, costs)?
7. Repeat deaths: is there a limit, or an escalating cost?
8. Does the player pilot the ship directly, or command from above?
