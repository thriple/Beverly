# CLAUDE.md

Beverly: a standalone, no-code-first 3D game editor for Bevy (Windows). The user is not a programmer; they
build by direct manipulation in 3D. The design lives in `docs/design.html` (keep it current when plans change).

## Rules from the user

- **UI kit:** use the Motion kit (`kit/motion-kit.zip`, ported in `editor/src/theme.rs`) wherever a component
  fits, and build any new UI in its style: its springs (SPRING / SNAPPY / SOFT), fade-and-scale entrances, colours,
  asymmetric corner radii (14/14/14/4) and keycaps for every shortcut.
- Caps Lock is the movement-mode switch only. Tab opens the parts pop-up. Mouse 5 / ] faster, Mouse 4 / [ slower.
- Basic shapes double as colliders (Visible off = invisible collider).
- Projects are folders of plain text (RON) so they work with git.
- Keep it simple: minimum code for what was asked, surgical changes, ask when unclear.
- Never put model names or ids in repository files. Never ask for keys in chat or write them into files.

## Tech

Bevy 0.20.1 + bevy_egui 0.43.1 (egui 0.36), pinned together with the toolchain in `editor/rust-toolchain.toml`.
Windows build: `.github/workflows/windows.yml` (release per push) or `tools/make-zip.ps1` locally.

## Releases

Push work to the repository freely, but only cut a release (commit message containing `[release]`, or a manual
run of the Windows build) for finished features, never half-done work.
