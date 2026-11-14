Beverly - a simple 3D game editor for Bevy (Windows build)

Run Beverly.exe. Nothing to install. A new project folder is made in Documents\Beverly.
Keep beverly-player.exe next to it: every exported game runs on it.

Moving around
  Caps Lock     switch between Editor movement and Builder movement
                (in Beverly, Caps Lock never turns on capitals)

  Editor movement (free cursor)
    middle-drag   orbit around the point in the middle of the view
    Space         hold and move the mouse to pan
    wheel         zoom toward the point under the cursor

  Builder movement (fly)
    mouse         look; W A S D fly where you look
    Mouse 5 / ]   faster      Mouse 4 / [   slower   (1/8x to 400x)
    Alt           hold for a cursor
    click         acts at the reticle

Building
  Tab           parts pop-up: pick a shape, then click a surface to drop it
                (Shift keeps it upright; Esc cancels)
  click         select; drag the arrows to move, the ring to turn (snaps 0.25 m / 15 degrees; Ctrl for free)
  F             frame the selection
  Delete        delete          Ctrl+D  duplicate
  Ctrl+Z / Ctrl+Y   undo / redo
  Visible off   the shape becomes an invisible collider

Testing
  `             play the level as the walker, starting at the reticle (` again: back to editing)
  Shift+`       start at the Player start instead (Tab > Game > Player start)
  in test       mouse looks, W A S D walk, Shift run, Space jump

Models
  Drag .glb / .gltf files onto the window; click one in Assets > Models, then click a surface to place it.
  Scale it in the bottom panel. Models are solid for the walker unless you set Collision to Walk-through.

Files
  Ctrl+S save   Ctrl+Shift+S save as   Ctrl+O open   Ctrl+N new   Ctrl+Q quit
  Drag files from Explorer onto the window to import them into the project's assets.
  File > Export Windows game   makes <project>.zip: unzip anywhere and run <project>.exe
