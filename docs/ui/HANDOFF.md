# Beverly UI — design handoff

**This is how Beverly should look.** Every new window, panel, control or tab must use the frame, colours, icons and controls described here. If something new needs a pattern that isn't covered, build it from the pieces below rather than inventing a new style, and add it to the Components board.

- Live design canvas (source of truth): https://claude.ai/artifact/KsZtJyq5D4aJenRcGLkTEc
- Design brief this grew from: Beverly is a no-code, Windows-only 3D game editor on Bevy, drawn with egui + a custom vector renderer. Everything is vector. The 3D view is the star.

## What's in this folder

| Path | What it is |
|---|---|
| `design/boards/*.dc.html` | The design boards (one per tab window, plus Tabs, Icons and Components). They render on the Claude Design canvas — use the link above to view them; treat the HTML as a precise spec of sizes, colours and structure. |
| `design/canvas.json` | Board layout on the canvas. |
| `tokens/tokens.json` | Every colour, size, radius and state in one place. |
| `tokens/theme.rs` | The same tokens as `egui::Color32` constants and a `TabTheme` table. |
| `icons/svg/*.svg` | All 88 icons, 24 × 24, `stroke="currentColor"`. |
| `icons/icons.json` | The same icons as raw path data, for the vector renderer. |
| `generator/` | The Python that produced the boards. Run `python3 windows2.py`, `boards.py`, `components.py` to regenerate into `generator/out/`. `base.py` holds the shared frame and controls; it's the most readable spec of how pieces combine. |

## Principles

1. **The view is the star.** Chrome is thin and translucent; panels float over the view and collapse.
2. **Words before numbers.** Feel settings are word sliders (Feather … Fixed). Exact numbers live under **More** (at the right of the row under each slider; its caret turns down while open). Only placement (position / turn / size) uses number fields.
3. **Futuristic with intent, minimal.** No decorative filler, no micro-text (nothing under 11 px), no random badges or lines. Every mark must mean something.
4. **Every action has a name and a shortcut.** Show the keycap on primary actions and hints; everywhere else the shortcut lives in the tooltip ("Duplicate  Ctrl D").
5. **Icons always come with a word.** Icon-only buttons are allowed for well-known actions (undo, duplicate, delete, collapse) and must have a tooltip and an accessible name.
6. **No dead ends.** Every warning offers a fix button ("Add Back", "Use “Door 2”").

## The frame (window chrome)

Window 1600 × 960 reference, 10 px outer corner radius, 1 px hairline edge.

- **Top bar, 44 px**: left — `BEVERLY ▾` menu (holds File, Edit, View, Play, Help). Centre — the tabs (eight today; see *Tab groups and the scrolling strip* below for what replaces the fixed eight), each icon + label; the active tab is a solid pill inverted from the frame. Right — Parts button (with `Tab` keycap), Undo, Redo, Snap (grid icon + toggle; value in tooltip), divider, Windows controls (minimise, maximise, close; close hovers `#E81123`).
- **Side and bottom rims, 8 px.**
- **View**: x 8, y 44, 1584 × 908, corner radius 14.
- **Top notch** (dips into the view at top centre, depth 34): the current tab's **topics** as chips (icon + label). Active topic is an inverted pill; not-yet-built topics at 55% opacity with a "Coming soon" tooltip.
- **Bottom notch** (rises into the view at bottom centre, depth 34): `Saved` state · shortcut hints with keycaps · `Timeline T` button.
- **One shape**: the view outline and both notches are a single path; shoulders are cubic curves `(x,0) → c(x+20,0) (x+22,D) → (x+44,D)`. The frame is drawn as window-rect minus that outline (even-odd), filled with the tab's frame colour at its frame alpha (0.86; Level 0.78), plus a white sheen 16% → 0% over the top 64 px, plus a 1 px edge line along the outline. The view is clipped to the same outline.
- The 3D world renders **under the whole window**, so the translucent frame shows it. On 2D tabs (UI/HUD, Screens, Animatics) the view gets a 90% ink backing.

## Colour system

### Tabs — two colours each, two jobs

- **Frame colour = where you are** (proposed: the frame itself goes neutral, see *Tab groups* below; the colour stays on the bezel and the pill): frame, active tab, active topic, selected part outline and tag, active dock view, Play button.
- **Accent = what you can change**: slider fills, toggles (on), playhead, focus ring, changed-value bar, More, status icon.

| Tab | Frame | Frame text | Highlight | Accent | Icon |
|---|---|---|---|---|---|
| Level | `#0E0E10` (dark glass, α .78) | `#F3F3F1` | `#FFD60A` | `#8C7DFF` | mountain |
| Characters | `#FF6EB4` | ink | same as frame | `#2BF0C8` | person |
| Vehicles | `#C6FF1A` | ink | same | `#B34DFF` | car |
| UI/HUD | `#2E4BF5` | white | `#5C78FF` | `#FFB020` | layout |
| Audio | `#19E0FF` | ink | same | `#FF7A59` | wave |
| Effects | `#FF8A1F` | ink | same | `#1EA7FF` | flame |
| Screens | `#B77BFF` | ink | same | `#D8FF4A` | monitor |
| Animatics | `#E9E7E2` (paper) | ink | `#FF2D46` | `#19D3C5` | film |

Every frame colour was checked for **≥ 4.5:1** text contrast in the worst case (translucent frame over the darkest/lightest scene). Check any new tab colour the same way before using it.

### Fixed across all tabs

- **Part categories** (colour + icon, never colour alone): Shapes `#D6D9E0` cube · Triggers `#FF7A1A` bolt · Actions `#3D7BFF` play · Flow `#2BF06A` route · Logic `#B066FF` diamond · Game `#FF3D9A` person · Lights `#FFE11A` bulb · Cameras `#22E5FF` camera.
- **Axes**: X `#FF3B4E`, Y `#39F26A`, Z `#3D7BFF`.
- **Danger** `#FF2D46` (text on dark `#FF8A98`). Only for real problems.
- **Neutrals** (panels never take the tab colour): ink `#0E0E10`, glass panel `rgba(18,18,21,.84)` + 1 px white 10% inset edge, popover `#1A1A1E`, field `#1F1F23`, field hover `#26262B`, selected row `#2A2A2F`, track `#34343A`, text `#F3F3F1` / `#B0B0B6` / `#8E8E95`.

## Tab groups and the scrolling strip (proposed 2026-11-20; not built yet)

Why: the roadmap adds many more workspaces, and module packs can bring their own. Eight fixed tabs, each tinting the whole frame, don't scale. This replaces the eight-tab bar and the per-tab frame colour above once built (roadmap entry `tab-strip`).

**Decided by the user, 2026-11-20**

- Colour coding by group stays: it is the shorthand for where you are.
- The colour does **not** fill the frame. On every tab the frame is the neutral dark glass Level has today, so workspaces stay easy to read. The group colour shows as a thin bezel along the view outline, as the selected tab's pill, and as the group's marker in the strip. Frame text is always light, so the per-tab frame-text contrast check goes away; only the selected pill's text needs the 4.5:1 check.
- The strip scrolls. The mouse wheel scrolls it while the pointer is over the tabs. Tabs squash toward the sides into slices (full label, then icon, then a thin coloured slice). The selected tab stays in the middle unless it is near either end of the list. No looping or wrapping, and the selected tab is never lost.

**Proposed, to confirm**

- Wheel scrolling moves the strip without changing the selected tab. A selected tab scrolled away from the centre stays visible as a pinned slice at the edge. A click selects a tab and brings it back to the centre. The ends are hard stops.
- Slices keep their group colour, so runs of colour show the groups. Hovering a slice widens it with a tooltip (name and group).
- A group's tabs share its colour pair (highlight and accent). A thin divider and the group's name in small caps sit between groups when there is room.
- **Switcher:** Ctrl+K searches all tabs and actions; Ctrl+Tab goes through recent tabs. Right-click a tab to pin, hide or move it. Ctrl+1 to 8 jump to groups.
- **Tabs are data, not a fixed array** (name, icon, group, topics, dock views, view kind 3D or 2D), so modules and packs can add tabs. A tab appears in the strip once its feature is built; there are no dead-end tabs.
- **A tab or a topic?** A tab is a workspace with its own library list, its own view kind (3D, or the 2D tabs on the 90% ink backing) or its own file type. A topic is a filter or a mode of the same view (the Physics view of Level).
- The view outline and notches stay one path (see the frame above); only its fill and the bezel line change.

**First cut of the groups** (workflow order; the strip starts as today's eight)

| Group | Tabs | Stays as topics |
|---|---|---|
| World | Level, Terrain, Water, Foliage, Light and sky, Levels, Modules | Level: All, Physics, Cameras, World settings. Light and sky: Lighting, Sky, Atmosphere, Post-processing |
| Characters | Cast, Rigs, Moves, Brains | Cast: Body, Controls, Display (HUD skins). Rigs: Joints, Clips, Ragdolls |
| Combat | Abilities, Arsenal | Abilities: Attributes, Effects, Status. Arsenal: Weapons, Tools, Defence |
| Vehicles and building | Garage, Build kits | Garage: Assemblies, Seats and mounts. Build kits: Snap rules, Structure strength |
| Interface | HUD, Screens | HUD: Layout, States, Fonts, Bindings. Screens: Flow, Results, Settings |
| Story | Animatics, Dialog, Quests | Animatics: Camera cuts, Subtitles, Recordings |
| Sound | Sounds, Mixer, Music | Sounds: Surfaces, Recordings |
| Effects | Effects, Particles, Weather | Effects: Decals, Fire |
| Systems | Variables, Inventory and economy, Game rules | Game rules: Saves, Packs, Achievements |
| Ship | Export, Player settings | Player settings: Quality, Controls, Accessibility, Languages |

Today's eight map to: Level to World/Level, Characters to Characters/Cast, Vehicles to Vehicles and building/Garage, UI/HUD to Interface/HUD, Audio to Sound/Sounds, Effects to Effects/Effects, Screens to Interface/Screens, Animatics to Story/Animatics. Group colours start from today's: World = Level's yellow on dark glass, Characters pink, Vehicles lime, Interface blue (Screens takes the group colour, giving up its purple), Sound cyan, Effects orange, Story the Animatics paper with red highlight. Combat, Systems and Ship need a colour pair from the palette (not the danger red), checked on the pill text.

Build notes: a tab registry replaces the fixed `theme::TABS` array and the `audio::LEVEL` and `audio::AUDIO` tab numbers; the frame fill becomes neutral with the colour in a 2 px bezel along the outline.

## Type

Roboto 400/500/700; Roboto Mono for numbers and timecodes. Panel title 16/700, group heading 12.5/500, body and labels 12.5, controls 12, smallest caption 11. Tabs 13. Never smaller than 11 px.

## Panels

- **Dock** (right, 352 wide, 12 px inset, radius 16, glass): header is a segmented switch `Inspect · <list> · Assets` (each icon + word) and a collapse chevron. Collapses to a 44 px **rail** of icons (click one to reopen). No key: `[` `]` are the brush size (user, 2026-11-17).
  - **Inspect**: identity header (40 px tinted icon tile, name 16/700, "Type · detail" subtitle, duplicate/delete icons) then **groups**. Each group: coloured icon + heading + collapse chevron; rows 28 px min, 8 px apart; label column 76 px. Rarely-used settings go in a collapsed **Advanced** group showing its count ("4 settings").
  - **List**: search field, small grey section headings, rows with the category icon in category colour; warnings right-aligned in danger text.
  - **Assets**: dashed drop hint ("Drop files anywhere" — the whole window is the drop target), then folder tree.
- **Timeline** (bottom, above the bottom notch, radius 16): resize grip, breadcrumb, transport (to start, Play in highlight colour, loop), timecode in mono, Seconds↔Frames zoom; track names with category icons; diamonds for keys, bars for clips; playhead and its time label in the accent; footer with Auto key toggle. One timeline widget is reused everywhere.
- **Status pills** (top-left of view): glass, icon in accent + short text. Warnings get a danger ring and a fix.
- **Selection tag**: small pill in the highlight colour with the part icon and name, next to the selected thing.
- **Audio tab** (Audio board): the dock is `Inspect · Sounds · Assets`; a **stage bar** (glass, radius 12) sits under the status pill: `On` + Speaker / Walker choices, the surface dropdown (Walker, layers icon), and Play / Stop as the main button (play / square icon, `P` keycap). In the view: the plinth and speaker, the walker's path as a faint dashed ring while it is out, the open sound's range rings in the accent round whichever plays it, and the selection tag on it. Inspect: the sound's identity (Play it once and Copy as icon buttons), then **Recordings** (each a name, its length in mono and a trash button over its waveform; an "Add a recording" dropdown), **On surfaces** (a heading per surface with its own recordings; "Add a surface"), **Sound** (Pick, Volume, Pitch, Vary, Loop), **Reach** (Near, Range), each with its setting icon (shuffle, volume, pitch, vary, loop, near, range), and **Advanced** (At once). The Sounds list: search, "New sound", then "In the level" and "Not used yet".
- **Screens tab**: flow graph — screen cards with previews and one port per button; wires grey, selected wire in highlight; back links dashed; a **Flow check** card lists reachability, depth and dead ends with fixes.

## Controls (see the Components board)

- **Setting icons**: a row may lead with a 14 px icon before its label, in the label's colour (white when changed); the label moves along 21 px. Every row of a group has one or none.
- **Word slider**: 4 px track, accent fill, 16 px white thumb with accent halo, tick per stop, stop labels under the track (current word white/500). Dragging: 6 px track, 20 px thumb, the word rides above in an accent pill. Changed from default: accent bar left of the row + bold label + reset icon on hover. Under it at the right, **More** (12/500, accent) with a 12 px caret that turns from › to ⌄ as it opens the exact numbers.
- **Number fields** (placement only): axis letter chip in axis colour + mono value; hover shows ↔ scrub arrows; drag to scrub, double-click to type (accent ring, value selected); lock icon links size proportions.
- **Colour picker**: opens from a swatch row (swatch + friendly name, e.g. "Warm concrete"). Popover 280 wide: name header, saturation/brightness square, hue strip, opacity strip (checker), eyedropper + hex + %, then **project colours first** and a + to save.
- **Toggle** 30 × 16, on = accent with ink knob; off = `#3A3A40`; mixed (multi-select) = dash knob on half accent. Show its shortcut keycap when it has one.
- **Segmented**: 2–4 options (words or icons-with-tooltips). Five or more → dropdown.
- **Dropdown**: field with a leading icon of what it holds; menu with search past eight items, check on current, "New …" quiet action at the bottom.
- **Alignment**: "Pin to" 3 × 3 dot grid (chosen spot fills with accent); align/spread icon groups appear for 2+ selected parts.
- **Text input**: 30 px, field fill; focus = 1.5 px accent ring; error = danger ring + message + a fix.
- **Buttons**: one main action per panel (highlight fill, ink text); second (`#2A2A2F`); quiet (accent text); danger (danger 16% fill, `#FF8A98` text). Labels are verbs.
- **Indicators**: Saved (green check) / Saving (accent spinner) / Unsaved (highlight dot); count badge (highlight); warning chip; info chip; Recording (danger dot); progress bars always labelled with what's happening and %.
- **Waveform** (a recording, Audio tab): 32 px field-coloured strip, radius 6; one bar per slice from its lowest to its highest, in the accent, scaled to the recording's own loudest; its length in mono beside its name. A thin muted line while it is being read.
- **Brush rings** (in the view, on the surface under the pointer): the accent ring is the brush's size, a fainter inner ring where its edge starts to fade (hardness); pink while erasing (Alt). The brush's panel takes Inspect while it is out: the material it paints, its Preset (S · M · L · XL, keys 1 to 4), word sliders Size · Hardness · Strength, Lays (Add · Subtract · Set), then **Where it lands** (Slope word slider, Only between heights with From / To, Only on a surface tag), its keys, and Done. While placing, the floating label shows how far the wheel turned and sized it (`turned 45° · ×1.21`) with Wheel · Shift · Esc keycaps.
- **Keycaps**: on panels dark `#26262A` with a 2 px bottom edge; on frames solid frame-text colour with frame-colour text.
- **Focus**: 2 px accent ring, 2 px offset, on every interactive element.

## Icons

One family: **24 × 24 grid, 2 px stroke, round caps and joins, ≥ 2 px clear edge, outline only** (never filled). Shared keyline shapes: circle r 9 at centre, square 3.5–20.5 with 1.5 radius. Rendered at 14, 16, 20 or 24 px (stroke 2.25 at ≤ 13 px). Part icons take their category colour; everything else takes the current text colour. New icons must follow the same grid and reuse existing shapes; add them to `icons.json` and the Icons board.

## Adding a new tab / window — checklist

1. Pick a frame colour and a complementary accent (opposite or split-complementary hue, similar brightness). Check ≥ 4.5:1 worst-case contrast for frame text.
2. Pick a tab icon from the set (or draw one to the icon rules).
3. Define 3–6 **topics** for the top notch, each with an icon.
4. Use the same frame, dock (Inspect · list · Assets), status pill, selection tag and bottom notch hints. Only the view content and the inspector groups change.
5. Inspector groups: coloured icon headings, word sliders first, an Advanced group for the rest.
6. Add a board to the canvas and a row to the tab table above.

## Adding a new control — checklist

1. Build it from existing pieces (field, segmented, toggle, slider, popover, chip).
2. Define rest, hover, active/dragging, focus, changed-from-default, and problem states.
3. Use neutrals for the body, the tab **highlight** for "selected/where", the tab **accent** for "value/changing".
4. Add it to the Components board.

## Don'ts

- No new colours outside the system; no tab colour inside panels except highlight/accent roles.
- No text under 11 px, no decorative lines, patterns or badges without meaning.
- No bitmaps — everything vector.
- No filled icons, no mixed stroke weights.
- Blur, drop shadows and backdrop filters are expensive in egui — the design uses translucency (alpha) instead of blur; shadows only under popovers and floating previews.
- No disabled dead-end buttons — hide what can't be used, or explain how to enable it.

## egui notes

- Sizes in the boards are CSS px = egui points at 100% scale.
- The frame outline is one `epaint::PathShape`; fill = tab frame colour × frame alpha; build the cut-out by tessellating window rect minus outline (even-odd) or by painting four regions plus the notch shapes.
- The view clip uses the same outline; clip the 3D/2D content to it.
- Glass panels = solid fill at 0.84 alpha + 1 px inset stroke at white 10%.
- Sheen = a vertical gradient mesh over the frame's top 64 px.

## Motion (the Motion kit, user 2026-11-17)

Everything you see follows this handoff; the Motion kit (`kit/motion-kit.zip`, Beverly's port in
`editor/src/editor/theme.rs`) gives only the movement.

- **Springs**: SPRING for containers resizing, SNAPPY for menus, tooltips and pills, SOFT for toasts.
- **Tooltips**: the handoff's tooltip (light pill, ink text, ink keycap) springs up 4 px from 96% scale.
- **Dock**: the active segment's pill slides between choices, content slides 48 px in the direction of travel and
  fades in, and folding to the rail springs the width.
- **Status**: the status pill at the view's top left behaves as the kit's dynamic island: it rests showing the
  movement mode and springs open to announce each action ("Saved", "Test started"), then settles back. It keeps the
  status pill's look (glass, accent icon).
- **Inputs**: text inputs keep the handoff's field; typed characters spring up as they land and drop away when deleted.
- egui can't blur, so the kit's blur-in is a fade with a slight scale.
