# How CreativeCraft is built

This is the short version of how one window ends up running seven applications.
It is written for someone reading the source, and for the question "how does this
differ from just launching the apps separately?"

## The shape of it

```
creativecraft/
  studio-shell/      the window: tabs, hosting, hand-offs, project store
  filmcraft/         Edit, Audio and Render
  effectcraft/       Effects, and the OFX plug-in host
  photocraft/        Photo, Pixel persona
  vectorcraft/       Photo, Vector persona
  figcraft/          Photo, Layout persona
  soundcraft/        audio editing
  solidcraft/        not hosted (different egui version)
  libs/              shared crates used by the apps
```

`studio-shell` is the only crate that produces the application. Everything else
is a library it links against, which is why the portable build is a single `.exe`
with nothing to install beside it.

## Hosting, not wrapping

Each tab owns a long-lived host. The host is a small adapter, not a copy of the
app:

| Module | What it adapts |
|---|---|
| `filmcraft_host.rs` | FilmCraft's `Session` and its egui app: project, sequence, timeline view, playhead |
| `effectcraft_host.rs` | EffectCraft's engine and UI: composition, node graph, render queue, OFX |
| `photocraft_host.rs` | PhotoCraft's session and layer stack |
| `vectorcraft_host.rs` | VectorCraft's document and layer export |
| `figcraft_host.rs` | FigCraft's pages, frames and text |
| `soundcraft_host.rs` | SoundCraft's session |
| `maestro_*.rs` | The local AI studio, installed and opened as its own web view |

The window itself is small: a menu bar, a status bar, and `workflow_bar` as a
**bottom** panel, which is what puts the page tabs along the bottom edge the way
DaVinci Resolve does.

A host is created once and kept. Switching tabs does not tear down and rebuild an
application, so an EffectCraft composition that took a minute to load stays
loaded while you look at the timeline.

## Why the tabs share more than a frame

The tabs are not screenshots of each other, and they are not separate processes
either. They share state through a handful of explicit hand-offs, each of which
moves real data:

- **Design → apps.** The Photo tab's design document is the single source. It is
  exported to SVG for VectorCraft and FigCraft, and each app's result comes back
  as a PNG that *replaces* the shared artwork rather than stacking on top of it.
  That replacement rule matters: without it, a deletion in one app only hides a
  copy and the old picture underneath brings it back.
- **Timeline → design.** A frame from FilmCraft's program monitor becomes a photo
  layer in the design.
- **Design → timeline.** A design becomes one or more clips on FilmCraft's
  sequence, and the design keeps a live link to them.
- **Layers → composition.** PhotoCraft's layers become individual EffectCraft
  layers, so a design can be animated rather than flattened to a movie.
- **Composition → design.** A rendered EffectCraft frame returns to Photo as a
  layer.
- **Audio.** The Audio tab reads the same FilmCraft session, so it is the same
  cut, not a re-import of it.

## The project store

`project_store.rs` keeps one folder per project under
`%LOCALAPPDATA%\CreativeCraft\projects`, with each app writing its own file inside
it (`cut.fcproj`, `cut.ecproj`, `cut.scraft`…). The shell reopens the last project
on launch.

The format of each file still belongs to the app that writes it, so any of them
can be opened by that app alone. The shell is a convenience over the apps, not a
replacement for their file formats.

## Plug-ins

OpenFX plug-ins are hosted by EffectCraft's `effectcraft-ofx` crate, but they run
in a **separate worker process**, started from the same executable with the
`--ofx-worker` flag:

- a plug-in that crashes cannot take the window down with it;
- a plug-in that hangs can be abandoned;
- the GUI process keeps its own memory and its own GPU context.

The cost is that third-party effects render one frame per call through that
worker, so they are correct but slower than a built-in effect.

## The coupling, stated honestly

`studio-shell/Cargo.toml` links **19 path dependencies** across the bundled apps,
including their `*-ui-egui` crates, `effectcraft-host` and `effectcraft-ofx`.

Those are not a published, stable API. They are the applications' own crates, at
whatever state this repository carries. Updating an app upstream can therefore
break the shell, and there is no version negotiation: a build is pinned to the
sources in this tree.

This is the main reason the project is careful about what it claims. It is a
shell around specific sources, not an integration layer with a contract.

## Building

```powershell
cd creativecraft\studio-shell
cargo build            # debug
cargo build --release  # the portable build
cargo test             # 63 tests
```

The first build needs roughly 25 GB of disk in `creativecraft\studio-shell\target`.
That folder is scratch space, is ignored by git, and can be deleted at any time.
