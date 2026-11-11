# CreativeCraft (studio-shell)

One window for the creative workflow, built from the real Craft apps:

| Tab / slot | Runs |
|---|---|
| **Edit** | FilmCraft's Editing workspace |
| **Effects** | EffectCraft (the real compositor) |
| **Photo -> Vector** | **VectorCraft** (the real illustrator) |
| **Photo -> Pixel** | **PhotoCraft** (the real pixel editor) |
| **Photo -> Layout** | **FigCraft** (the real interface designer) |
| **Audio** | **SoundCraft** (the real audio workstation) |
| **Render** | FilmCraft's Export screen, or EffectCraft's Render Queue |
| **Maestro** | Maestro's local AI studio (opened in your browser) |

There is one CreativeCraft window and one custom top bar. The Photo tab keeps
one shared piece of artwork, and the three Photo personas edit it in the real
apps' own interfaces. Edit, Effects and Audio share one FilmCraft project and
timeline.

## Run it

Open PowerShell and paste these two lines:

```powershell
cd D:\Cretive Craft\creativecraft\studio-shell
cargo run
```

The window opens on the **Photo** tab with the Layout persona (FigCraft) showing
a sample pricing design. FilmCraft opens its demo project in the background, so
Edit already has a sequence, media and a playhead; EffectCraft opens its demo
composition for the Effects tab. The first build compiles all six apps and takes
a long time; later starts are quick. SoundCraft starts on its own the first
time you open the Audio tab.

### Closing and reopening: one folder per project

Pressing the window's close button (X) works from **any tab**, and nothing is
lost: the shell saves every open tab first, then closes. There is no "save
changes?" dialog to answer — that prompt belongs to the hosted app, and on other
tabs it was invisible, which is why closing there used to do nothing at all.

Everything a project made goes into one folder named after the project:

```text
%LOCALAPPDATA%\CreativeCraft\projects\CreativeCraft Demo\
    CreativeCraft Demo.fcproj   the cut (Edit, and what Effects is linked to)
    CreativeCraft Demo.ecproj   the EffectCraft project, for File > Open there
    CreativeCraft Demo.scraft   the SoundCraft session
    Audio Files\                the audio the session uses (e.g. cut-audio-000.wav)
```

The demo project arrives from FilmCraft named "FilmCraft Demo"; the shell renames
it (only that exact name) to "CreativeCraft Demo", so the folder and the file
names say CreativeCraft. A project you open yourself keeps its own name.

The next launch reopens that folder automatically: the cut is back in Edit, and
the mix is back in SoundCraft when the Audio tab first starts (the Audio strip
says "Reopened the SoundCraft session from …"). The EffectCraft project is saved
but not auto-opened, because the Effects tab rebuilds its composition from the
cut ("Sync cut"); open the `.ecproj` by hand if you want that version back.

## The three Photo personas

Each persona is the **real app**, drawn inside the CreativeCraft window with its
own tools, panels, menus and document - not a look-alike panel.

- **Vector** is VectorCraft: paths, shapes, gradients, layers, styles, type.
- **Pixel** is PhotoCraft: layers, masks, brushes, adjustments, text, filters.
- **Layout** is FigCraft: pages, frames, auto layout, components, styles,
  prototypes and export.

**One surface colour.** PhotoCraft starts on **Studio (Dark)**, the near-black
theme with the violet accent, because that is now its built-in default in
`photocraft/crates/engine/src/prefs.rs` (`Interface::default` and the `Theme` /
`DarkTheme` choice defaults). A theme picked in Preferences ▸ Interface still
wins for that session, so nothing overrides the user. In CreativeCraft the Pixel
tab therefore matches the other tabs instead of flashing from black to grey.

Every persona has the same four link buttons in its strip:

- **Frame from timeline** - pull the Edit frame under the playhead into the
  shared artwork here.
- **Place on timeline** - render the current art and insert it at the playhead
  on its own FilmCraft track (and build the layered Effects composition).
- **Design to ...** - re-send the shared artwork to this app.
- **... to Photo** - take this app's result back into the shared artwork and
  show it in the Layout persona.

Layer transfers between the apps:

- **Pixel: Layers to Vector** / **Layers to Layout** - PhotoCraft's top-level
  layers open in VectorCraft/FigCraft, one image layer each, at their positions.
- **Vector/Layout: Layers to Pixel** - their layers open in PhotoCraft as one
  image layer each (this is also how you start Pixel with a real multi-layer
  stack instead of the one flat design image).
- **Vector: Layers to Layout** and **Layout: Layers to Vector** - exchange their
  editable SVG layers by hand (the same link Auto uses). These stay shapes and
  text, not images.

**Auto link** (checkbox next to "Shared document") controls the automatic
hand-off: on, entering/leaving a persona sends the work as before; off, only the
strip buttons move work, so each app keeps its document until you ask.

## How the three apps share one design

VectorCraft and FigCraft both speak SVG, so they exchange one **editable**
`artwork.svg`: a shape or text layer deleted in one is really gone in the other,
and it is still editable there. PhotoCraft is a raster editor, so it receives a
flattened PNG and returns a PNG.

- `%LOCALAPPDATA%\CreativeCraft\shared\artwork.svg` is the file both vector apps
  read and write.
- Entering one opens that file when it is current. If the design changed
  elsewhere (a frame from Edit, an edit in PhotoCraft), the design is written
  there first.
- Leaving an app you changed writes its SVG back, then updates the shared
  design's PNG preview (one layer) for PhotoCraft and the timeline.
- The PNG preview **replaces** the old artwork, so a deletion in any app cannot
  reappear from a layer underneath.
- Switching without editing does **no** export and **no** re-import; the first
  entry always opens the artwork.

The apps keep their rich documents and undo history while they are open. The
shared design preview is the common, flattened picture; Vector ↔ Layout keep the
editable original.

## Design into animation

**Place on timeline** in Vector, Layout or Pixel does two things:

1. Inserts the flattened design as a clip at the Edit playhead, so the video
   timeline has it.
2. Renders **each element** to its own PNG, builds a new EffectCraft composition
   with **one layer per element** (in Pixel: one per PhotoCraft layer),
   and opens it in Effects.

There each part can be keyframed, moved, faded and effected on its own. The
composition is named "VectorCraft design" or "FigCraft page". FilmCraft itself
is a video editor and has no design layers: it gets the single clip.

When the animation is ready, press **Render to Edit** in the Effects strip. It
renders the whole composition to an MP4 (H.264), imports that file into
FilmCraft and places it at the playhead as a normal clip, then switches to Edit
so you can play it with the rest of the cut. Rendering takes a moment (roughly
the length of the comp, on the CPU).


## Edit, Effects and Audio
- **Edit** shows FilmCraft's Editing workspace: media pool, timeline, program and
  source monitors.
- **Effects** shows **EffectCraft's own interface**: Project panel, Composition
  viewer, Timeline with per-layer properties, Effect Controls, Effects & Presets,
  masks, keyframes, expressions and the render queue. The first visit exports the
  Edit sequence as **Final Cut Pro 7 XML** and imports it as a composition
  ("Sync cut" re-imports later changes as a new version). Stills are fitted to
  the frame; numbered stills stay one photo.
- **Audio** shows **SoundCraft's own interface**: Edit and Mix windows,
  transport, track and clip lists, mixer, plugins, automation, bounce and its
  video track. It starts the first time you open the tab and opens its own audio
  device, so it can play the session itself. A strip at the bottom, **Edit
  audio**, draws the cut's audio timeline (one row per audio track, with the Edit
  playhead); drag a clip up into SoundCraft, or click it. Its **⋯** menu swaps to
  FilmCraft's Audio workspace (Clip Mixer, Essential Sound and the same
  sequence) when you want it.
- The Photo tab keeps a small read-only strip of that timeline at the bottom.

### OpenFX plug-ins (third-party effects)

EffectCraft can use the **OpenFX** effects installed on this PC, so
Effects & Presets is not limited to its own effects. They appear at the bottom
of the list, in a folder named after the plug-in maker (for example
**Color -> ToneLab Chroma**), and they apply and render like any other effect:
double-click one with a layer selected, then edit its real controls in **Effect
Controls**.

Where they are looked for (the standard OFX places):

- `C:\Program Files\Common Files\OFX\Plugins`
- vendor folders: `C:\Program Files\Red Giant`, `Boris FX`, `GenArts`, `Sapphire`
- anything in the `OFX_PLUGIN_PATH` environment variable

**They are isolated on purpose.** OFX plug-ins are native libraries from other
companies; some corrupt memory, spin forever or pop up dialogs. CreativeCraft
loads and renders them in a **separate helper process** (the window re-runs
itself with `--ofx-worker`). If a plug-in crashes or hangs, only that helper is
killed and the window keeps running. A bundle that does it twice is written to
`%LOCALAPPDATA%\CreativeCraft\ofx-skip.txt` and ignored — but only for a day: a
large plug-in can miss its load timeout on a busy machine once, so it gets one
more try the next day. Delete that file to retry straight away.

**The first start scans, then it is fast.** Finding plug-ins is done on a
background thread, so the window opens immediately, but a machine with many
bundles (Red Giant alone ships over a hundred) can spend a couple of minutes
looking at them. Each answer is remembered: `ofx-unusable.txt` holds bundles that
refused this host (so they are not retried), and `ofx-usable.txt` holds effects
that passed the test render (so a slow plug-in is not re-tested). Both are keyed
to the file's date, so updating a plug-in retests it automatically. Delete the
files (or `ofx-skip.txt` for bundles that hung) to make CreativeCraft look at
everything again.

**What is checked before an effect is offered.** Loading and describing are not
enough: a plug-in can accept both and then refuse to render. On registration,
each effect renders a 32x32 test frame. Only effects that produce that frame are
listed - a dead entry in the menu is worse than no entry.

What was tested on this machine:

| Plug-in | Result |
|---|---|
| **ToneLab Chroma** (ToneLab) | Loads, applies and renders. Its own parameters (Conversion LUT, Intensity, Lift, Gamma, Gain, Temperature, Saturation...) drive the result; verified end to end in the tab. |
| **Raytracing Relight 1.0** (Trioscope SEE 2.1) | Loads and passes the test render. |
| Red Giant (Cosmo II, MB Film, Magic Bullet Looks, Mojo II, Renoiser), Red Giant Universe, Topaz Video AI, DaVinci Resolve Renderer | Load and describe correctly, but their render refuses without a GPU render context or a licence check this host does not provide. They are hidden rather than shown broken. |

Limits, stated plainly:

- Rendering is **CPU-side float RGBA**. Plug-ins that only work on GPU
  textures are not supported, and vendor overlays (on-screen handles drawn by
  the plug-in) are not drawn.
- Plug-ins render **one at a time** through the single helper, and each frame
  crosses a process boundary, so an OFX effect is slower than a built-in one.
- A plug-in sees a **filter context** with `Source` and `Output` clips; extra
  clips, generators, transitions and multi-view are not offered yet.

### SoundCraft in the Audio tab

The hosted SoundCraft is the real application, not a look-alike. Its link to
the cut is audio, because it has no AAF/OMF interchange yet: each side keeps a
normal project and the clips travel as files.

- **Edit audio** under the DAW is the cut's own audio timeline, drawn at the
  same times as Edit: one row per audio track, plus the Edit playhead. A clip
  with a real file is drawn in green; **drag it up into SoundCraft** and it
  opens as a new track at the position it has in the cut, or **click it** to do
  the same. Clicking also selects that clip in Edit, so Edit and the strip agree.
- A generator (the demo project's synthetic footage) has no file to hand over:
  it is drawn in grey, cannot be dragged, and says so by name if clicked.
- **⋯ → Selected clip from Edit** is the same as clicking the clip selected in
  Edit, for the times the strip is too small to click accurately. **⋯ →
  FilmCraft mixer** swaps the tab between SoundCraft and FilmCraft's own Audio
  workspace (Audio Track Mixer, Audio Clip Mixer, Essential Sound for the
  selected clip); the menu entry then reads **Back to SoundCraft**.
- The file itself is handed over, so it must be a format SoundCraft decodes:
  WAV, AIFF, FLAC, MP3, Ogg, AAC/M4A and the audio of MP4/MOV files. Camera
  formats it cannot read (MXF, for instance) still arrive through **Cut audio**,
  which renders a WAV first.
- **Cut audio** renders the whole sequence's audio and opens it in SoundCraft as
  one track at 0, ready to mix.
- **Mix to Edit** bounces the SoundCraft session to a WAV and places it at the
  **Edit playhead** on a new audio track, then switches to Edit. The cut's own
  audio tracks are left alone, so this adds a layer rather than replacing the
  soundtrack. The new clip then appears in the **Edit audio** strip, ready to
  drag back into SoundCraft.
- SoundCraft plays through its own audio device, so it is a real DAW transport:
  press Play in the tab and the session is heard. Leaving the Audio tab (or
  switching to **FilmCraft mixer**) stops its transport, so two applications
  never play at once.
- SoundCraft's own **File > Import > Video** works inside the tab (its file
  dialogs are wired to this window), so the MP4 from **Cut to DAW** can be put
  on its video track for picture sync.
- SoundCraft sessions save as `.scraft` and are independent of the Edit project.
  Re-open one with **File > Open** in the tab; the link does not overwrite it.

### DAW audio link for other apps (WAV/MP4)

These Edit-strip buttons are for DAWs that are **not** hosted in this window
(Lemon Studio, Reaper, Audacity, another SoundCraft install).

- **Cut to DAW** in Edit renders the cut at half size (H.264 + audio) to an MP4
  for a DAW's video track (SoundCraft has one), so picture and sound stay in
  sync while you mix.
- **Audio to DAW** in Edit writes the whole sequence's audio to a WAV mix at
  `%LOCALAPPDATA%\CreativeCraft\audio\cut-audio-NNN.wav`. Open it in SoundCraft,
  Lemon Studio, Reaper or any DAW, edit, then render/bounce the mix.
- **Mix from DAW** imports that bounce (WAV/AIFF/FLAC), puts it at the sequence
  start on a new audio track, and mutes the tracks it replaces so the mix is not
  heard twice.
- This is file interchange, not a shared session. SoundCraft has no AAF/OMF yet;
  Lemon Studio is even earlier (pre-alpha, GPL, `.flp`/WAV only). Do not expect
  an automatic "import at playhead" from Lemon: use the WAV buttons.
- Lemon Studio can render a song to WAV (`File > Export audio`, or its
  `--render` command) and its playlist has audio clips; bring that WAV back with
  **Mix from DAW**. Its own project files remain `.flp`/`.lemon`.

Design -> timeline is live-linked: place the design, then edit it, and the clip's
media refreshes. Frame -> design pulls a still snapshot at full resolution into
the shared artwork.

Interchange files:

- Edit -> Effects XML: `%LOCALAPPDATA%\CreativeCraft\link\`
- VectorCraft: `...\vector\`
- PhotoCraft: `...\pixel\`
- FigCraft: `...\fig\`
- Pulled frames: `...\frames\`
- Rendered animations: `...\render\`
- SoundCraft mixes: `...\audio\`

## Render

The **Render** tab is where the two pictures are rendered, and it holds the two
apps' own output panels rather than a copy of them:

- **FilmCraft Export** — the cut, in FilmCraft's real Export screen: destination,
  Media File, preset, format, video/audio settings, the summary and *its* queue
  (Send to Queue, Start Queue).
- **EffectCraft Render Queue** — the composition, in EffectCraft's real Render
  Queue panel: the entry list, Notify, Render, and the "Rendered / Total" line.

Switch with the two buttons under the tab. Each app writes its own file (the cut
as an MP4, the composition to its own output), so there is no single "render
everything" file to wait for — but both queues keep working: EffectCraft's
render keeps running while you watch FilmCraft's export, because the tab runs its
logic either way.

Both panels at once is deliberately **not** offered: two whole applications
drawing in one frame share egui's widget ids (the red "widget ID" warnings), so
a click could land in the wrong app. Two clean full-width panels were better
than a cramped split that misbehaves.

## Maestro

Maestro is a local AI video, image and music studio, installed through
[Pinokio](https://pinokio.computer). It is **not** a Rust app and has no library
this window can draw: its Studio, Director and Editor are a web page served by a
local Python server. Two consequences, stated plainly:

- **Where the models go.** Maestro downloads its own weights on first use of each
  model, into its Pinokio installation, usually
  `%USERPROFILE%\pinokio\api\Maestro.git` (150–500 GB for a full set). The
  **Model folder** button in the Maestro tab opens that place; CreativeCraft never
  downloads, moves or deletes those models, and cannot pick a different download
  directory for Maestro — that is Maestro's own setting.
- **What the tab does today.** It keeps Maestro's address and its install folder,
  **starts Maestro's own backend** (`app/launch.py`, on a free port, no Pinokio
  needed at run time), reads the address Maestro prints when it is ready, and
  opens that page in your browser. **Open** reopens the last address; **Model
  folder** takes you to the install folder; **Stop Maestro** stops the backend it
  started (closing the window does too).
- **Installation is still a one-time step.** Pinokio's *Install* is the easy way
  to create Maestro's Python environment (or follow the steps in Maestro's own
  README). After that, point the tab at that folder — the default is
  `%USERPROFILE%\pinokio\api\Maestro.git` — and Start Maestro works without
  opening Pinokio again.
- **Install Maestro** (shown while it is missing, and as **Install / repair**
  afterwards) always asks first: a warning lists what it needs — an NVIDIA GPU
  (with the one it found), Python 3.11, Node.js + npm, git, about 10 GB for the
  packages plus disk for models — and only downloads when **Yes, install** is
  pressed. It installs the missing tools itself (`git`, Python 3.11, Node LTS via
  `winget`), clones Maestro, builds its Python 3.11 environment, installs the
  packages and the **CUDA build of torch**, builds the React interface, and ends
  with `torch … cuda True/False`. Model weights are not part of this: Maestro
  downloads those itself the first time a model is used. Finished steps are kept,
  so a stopped install resumes where it left off.
- **The page in the tab.** The tab puts Maestro's own page in a WebView2 view —
  the Edge engine Windows already has — on a thread of its own, because WebView2
  needs a single-threaded apartment and the GPU backend owns COM as MTA on the
  main thread (creating it there failed with `E_INVALIDARG`). While the backend
  is not running the tab says so instead of showing an error page; **Open** always
  opens the same address in your normal browser. If the in-window view cannot
  start on a machine, the strip reports it and the browser path still works.
- **Address and folder are remembered** in
  `%LOCALAPPDATA%\CreativeCraft\maestro-url.txt` and `maestro-folder.txt`.

### The file link (both ways)

Maestro has its own library, so work travels as files — the same model as the
Photo tab's frame buttons:

- **Frame from Edit** — writes the frame under the Edit playhead as a PNG.
- **Design from Photo** — writes the shared design as a PNG.
- **Audio from Edit** — writes the cut's whole audio as a WAV.
  These three land in `%LOCALAPPDATA%\CreativeCraft\maestro\in\` with numbered
  names (`frame-001.png`, `cut-audio-002.wav`, …); the tab shows the last path and
  **Copy path**, so it can be dragged or pasted into Maestro's page.
- **Newest on timeline** — takes the newest video, image or audio Maestro wrote
  (its `outputs`, `app/outputs`, `app/gallery` or `output` folder, two levels
  deep) and places it on the Edit timeline: a video as a clip, an image as a still
  at the playhead, audio at the playhead on its own track.
- Nothing is copied twice: both directions are the files themselves, so editing
  the still or re-exporting the mix in Maestro never edits the cut behind your
  back. Press the button again to bring the new version in.

### Why the page is not inside the tab yet

The tab was built to host Maestro's page in a WebView2 child window over the tab
(the `wry` crate). The dependency builds, but WebView2 refuses to create the child
view in this window on this machine — `E_INVALIDARG (0x80070057)`, with bounds,
without bounds, and with its own data folder. Rather than ship a tab that shows an
error where Maestro should be, the embed was removed and the tab keeps the
launcher. The remaining work is a diagnostic on the WebView2 side (parent HWND,
DPI context, WebView2 Runtime version), then the page can be dropped into the
same area.

A second attempt opened Maestro as a chrome-less **Edge app window** placed over
the tab. Edge ignored app mode, attached to the normal browser profile and opened
its own Settings page on top of the app, so that button was removed rather than
shipping something that takes over the browser.

## File and export

- **Save** / `Ctrl+S` and **Open** / `Ctrl+O` use the shared design project.
- **Export PNG** / `Ctrl+E` flattens the shared canvas at its real size.
- **Undo** / `Ctrl+Z`, **Redo** / `Ctrl+Shift+Z` cover object edits in the shared
  design; each hosted app has its own undo inside its document.
- FigCraft's **Export page as PNG** and the strip's **Place on timeline** both
  render its current page.

## Honest limits

- **Vector ↔ Layout stays editable** through the shared SVG. PhotoCraft is
  raster: once the artwork goes through Pixel, shapes come back as pixels. The
  shared design preview is always one flattened image.
- A deletion made in PhotoCraft replaces the whole vector artwork with that
  image in the vector apps too, so their shapes are gone (this is the honest
  cost of mixing a raster editor with vector editors).
- The apps do not share an in-memory document. SVG/PNG interchange is the link,
  just as FCP7 XML is the link between Edit and Effects.
- **Render to Edit** writes a normal MP4 and places it as a clip. It is not
  live-linked to the EffectCraft composition: animating further and rendering
  again adds/places a new clip, so delete the old one if you want to keep just
  the latest. Rendering is CPU work and blocks the window while it runs.
- Edit -> Effects is one-way until you press **Render to Edit**: the XML carries
  timing, transforms, opacity and basic audio, not every effect from either app.
- EffectCraft decodes on the CPU: heavy 1080p60 H.264 playback is not real-time
  (about 8 fps at full size in testing). Fast Previews uses Adaptive quality.
- The Photo timeline strip is read-only and FilmCraft audio output is not
  connected yet, so playback is silent.
- The Edit/Photo design link is session-only; it is not saved inside `.fcproj`
  yet. A pulled frame is a still, not a live clip.
- **SoundCraft and FilmCraft do not share a session.** A clip dragged from the
  **Edit audio** strip arrives in SoundCraft as a copied file on its own track,
  and **Mix to Edit** comes back as a WAV. Edits made in one do not appear in
  the other until you press one of the strip's buttons again.
- The root media of the demo project is synthetic (generators), so nothing in
  the **Edit audio** strip can be dragged until the project has real files
  (import in Edit, or place a SoundCraft mix with **Mix to Edit**).
- **Saving on close is one-way and automatic**: there is no "discard changes"
  button, so a project keeps what was open when the window closed. A big
  project (the EffectCraft file plus SoundCraft's audio) can take a few seconds
  to write, and the window closes as soon as that finishes.
- Only the project folder is written on close. The shared design artwork still
  lives in `%LOCALAPPDATA%\CreativeCraft\shared\` and is rewritten as you edit.
- Hosted apps install their own fonts and colours on the shared egui context.
  CreativeCraft reinstalls the active app's families before drawing, but a
  tab can look slightly different right after another app was in front.
- FilmCraft, EffectCraft, VectorCraft, PhotoCraft and FigCraft are consumed from
  the local checkouts at `D:\Cretive Craft\creativecraft\...`, not copied here.
  Updating one and rebuilding is a deliberate step.

## Next steps

- Persist the Photo design link inside the project so it survives restarts.
- Drag a design straight from Photo onto the timeline.
- Keep **Render to Edit** in sync: replace the previous render clip instead of
  adding another one.
- Send the cut as a video with audio to SoundCraft for picture sync, not only the
  WAV mix.
- Seed PhotoCraft with the design's individual layers, so Pixel starts with a real
  stack instead of the one flat image the design hands over.
