# Changelog

All notable changes to CreativeCraft are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com); versions follow
[SemVer](https://semver.org), loosely — 0.x releases make no stability promises.

## [0.1.0] — 2026-11-11

The first public release: one Windows window hosting seven creative tools, with
the page tabs along the bottom edge.

### Added

- **Edit** — FilmCraft: media pool, timeline, program monitor, markers, export.
- **Effects** — EffectCraft: node-based compositing, keyframes, render queue.
- **Photo** — one design document, three personas: Vector (VectorCraft), Pixel
  (PhotoCraft) and Layout (FigCraft frames, text, auto layout).
- **Audio** — the same cut's audio through FilmCraft, with a hand-off to
  SoundCraft.
- **Render** — FilmCraft export presets and the EffectCraft render queue.
- **AI** — Maestro, a local AI video, image and music studio in its own web view,
  installed and launched from the AI tab.
- **Hand-offs between the apps**: a rendered frame becomes a photo layer, a design
  becomes timeline clips, PhotoCraft layers become EffectCraft layers, and each
  result replaces the shared document rather than stacking on it.
- **OpenFX hosting** in a separate worker process, so a crashing plug-in cannot
  take the window down. Verified with CPU-rendering plug-ins on the build machine
  (ToneLab Chroma, Trioscope Raytracing Relight at 1920×1080).
- **Project store**: one folder per project under
  `%LOCALAPPDATA%\CreativeCraft\projects`, with each app writing its own file, so
  any app can still open its own work alone.
- MIT `LICENSE`, a `NOTICE` covering the bundled apps, and a `README` that states
  what is verified and what is not.

### Changed

- **Opens on an empty project.** A blank 1200×800 canvas in Photo, and an
  "Untitled" project with no clips or sequences in Edit. Nothing is pre-filled;
  the demo projects that the apps used to open automatically were removed, and
  FilmCraft's demo is available only through its own explicit button.

### Not working (stated up front)

- **The 3D tab is a placeholder.** EffectCraft's Advanced 3D panels exist; the tab
  is not a modeller yet.
- **Red Giant** (Magic Bullet, Universe), **Boris FX**, **Topaz Video AI** and the
  **DaVinci Resolve Renderer** do not render here. Details in the README.
- Plug-ins that draw on-screen handles render the picture but do not show the
  handles.
- Third-party plug-ins render one frame per call through one worker process:
  correct, but slower than a built-in effect.
- The first launch scans installed OpenFX plug-ins and can take up to about 90
  seconds; later launches use a cache.

[0.1.0]: https://github.com/its-cybru/creativecraft/releases/tag/v0.1.0
