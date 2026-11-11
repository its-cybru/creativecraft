//! The real EffectCraft compositor, hosted in the Effects tab.
//!
//! This is not a FilmCraft workspace wearing an Effects label. EffectCraft's
//! own `EffectcraftApp` draws here: its menu bar, Project, Composition viewer,
//! Timeline, Effect Controls, Effects & Presets, keyframes and render queue.
//!
//! The link to Edit is real interchange, the same mechanism the two apps use
//! to talk to each other: the FilmCraft sequence is exported as Final Cut Pro
//! 7 XML and EffectCraft imports that document as a composition. That is the
//! Premiere + After Effects model — two views, one cut — without pretending a
//! second timeline edits the first one live.

use effectcraft_ui_egui::EffectcraftApp;
use eframe::App as _;
use eframe::egui;
use serde_json::json;

/// One hosted compositor with the cut it last imported.
pub struct EffectcraftHost {
    app: EffectcraftApp,
    /// The title EffectCraft asked the window to show. EffectCraft titles the
    /// window after its project; the shell keeps "CreativeCraft" and restores
    /// it whenever EffectCraft changes its own title.
    host_title: Option<String>,
    /// What the last import represents, so an unchanged cut is not re-sent.
    #[cfg(feature = "filmcraft")]
    linked_cut: Option<String>,
    /// How many cuts have been imported this session (the Project panel keeps
    /// every version, so the user can compare or go back).
    #[cfg(feature = "filmcraft")]
    imports: u32,
}

impl EffectcraftHost {
    /// Start EffectCraft on its demo project so the tab has a composition,
    /// layers and a timeline before anything is linked.
    pub fn new() -> Result<Self, String> {
        let mut session = effectcraft_host::session();
        session
            .execute("file.openDemoProject", json!({}))
            .map_err(|error| format!("EffectCraft could not open its demo project: {error}"))?;

        let mut app = EffectcraftApp::new(session);
        // The shell already has a top bar; EffectCraft's Home screen would
        // cover the composition it was opened for.
        app.ui.start_screen = false;
        app.set_workspace("Effects");
        // Heavy footage (1080p60) decodes at ~8 fps at full size, so let the
        // viewer drop to quarter resolution while you drag or scrub, like
        // After Effects' Fast Previews; it returns to full quality when idle.
        let _ = app
            .session
            .execute("view.fastPreviewMode", json!({ "mode": "adaptive" }));
        // File > Import, Open, Save As and the export paths ask the host for a
        // picker; without these the hosted app's menu items did nothing.
        install_file_dialogs(&mut app);

        Ok(Self {
            app,
            host_title: None,
            #[cfg(feature = "filmcraft")]
            linked_cut: None,
            #[cfg(feature = "filmcraft")]
            imports: 0,
        })
    }

    /// EffectCraft's own interface, drawn inside the shell's Effects area.
    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.draw(ui, frame);
    }

    /// EffectCraft's logic pass: compositor start-up, preview prefetch, render
    /// queue and auto-save run here.
    pub fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        // EffectCraft titles the window after its project ("EffectCraft ...
        // .ecproj *"). The window is CreativeCraft's, so put our title back as
        // soon as EffectCraft changes its own.
        let wanted = effectcraft_ui_egui::panels::unsaved::window_title(&self.app);
        if self.host_title.as_deref() != Some(wanted.as_str()) {
            self.host_title = Some(wanted);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("CreativeCraft".to_string()));
        }
    }

    /// Add a 3D primitive layer (cube/sphere/plane/torus/cone/cylinder). Kept for
    /// the 3D tab's own tools once a real 3D app is hosted.
    #[allow(dead_code)]
    pub fn add_3d_primitive(&mut self, kind: &str) -> Result<(), String> {
        self.app
            .session
            .execute("layer.new3dPrimitive", json!({ "kind": kind }))
            .map(|_| ())
            .map_err(|error| format!("EffectCraft could not add the 3D {kind}: {error}"))
    }

    /// Import a 3D model file (.gltf/.glb/.obj/.stl/.fbx) as a layer in the
    /// active composition: EffectCraft's own Advanced 3D path.
    pub fn add_3d_model(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.app
            .session
            .execute("layer.newModel", json!({ "path": path.to_string_lossy() }))
            .map(|_| ())
            .map_err(|error| format!("EffectCraft could not import {}: {error}", path.display()))
    }

    /// Bring EffectCraft's own Render Queue forward: the real panel, the one
    /// its Window menu opens, over the composition the cut was imported into.
    pub fn show_render_queue(&mut self) {
        self.app
            .show_panel(effectcraft_ui_egui::dock::PanelKind::RenderQueue);
    }

    /// Close EffectCraft's session cleanly (recovery sentinel, workspace state).
    pub fn on_exit(&mut self) {
        self.app.on_exit();
    }

    /// Write the whole EffectCraft project to `path` (Save As), so the shell can
    /// keep it beside the cut in the project folder.
    pub fn save_project(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.app
            .session
            .execute("file.saveAs", json!({ "path": path.to_string_lossy() }))
            .map(|_| ())
            .map_err(|error| format!("EffectCraft could not save {}: {error}", path.display()))
    }

    /// Put EffectCraft's interface fonts back if another embedded app replaced
    /// them on the shared egui context (VectorCraft installs its own families).
    pub fn ensure_fonts(&mut self, ctx: &egui::Context) {
        let installed = ctx.fonts(|fonts| {
            fonts
                .definitions()
                .families
                .contains_key(&egui::FontFamily::Name("semibold".into()))
        });
        if !installed {
            effectcraft_ui_egui::theme::install(ctx, &self.app.tokens, "en");
        }
    }

    /// Build a new composition with one layer per design element.
    ///
    /// This is the "design into animation" step: VectorCraft/FigCraft render
    /// each part, and EffectCraft gets a normal comp where every part can be
    /// keyframed, offset and faded on its own.
    pub fn animate_layers(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        layers: &[crate::layer::LayerPng],
    ) -> Result<(usize, u64), String> {
        if layers.is_empty() {
            return Err("There are no layers to send to EffectCraft.".to_string());
        }
        let comp =
            self.app
                .session
                .execute(
                    "comp.new",
                    json!({
                        "name": name,
                        "width": width.max(1),
                        "height": height.max(1),
                        "duration": 10.0,
                    }),
                )
                .map_err(|error| {
                    format!("EffectCraft could not create the composition: {error}")
                })?["comp"]
                .as_u64()
                .ok_or_else(|| "EffectCraft did not return a composition.".to_string())?;

        for layer in layers {
            let imported = self
                .app
                .session
                .execute(
                    "file.import",
                    json!({ "paths": [layer.path.to_string_lossy()] }),
                )
                .map_err(|error| format!("EffectCraft could not import {}: {error}", layer.name))?;
            let item = imported["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|id| id.as_u64())
                .ok_or_else(|| format!("EffectCraft imported no footage for {}.", layer.name))?;
            let mut params = json!({
                "comp": comp,
                "item": item,
                "time": 0.0,
                "duration": 10.0,
            });
            if let Some((x, y)) = layer.position {
                params["position"] = json!([x, y]);
            }
            self.app
                .session
                .execute("layer.addItem", params)
                .map_err(|error| format!("EffectCraft could not add {}: {error}", layer.name))?;
        }

        let _ = self
            .app
            .session
            .execute("comp.open", json!({ "comp": comp }));
        let _ = self.app.session.execute("time.set", json!({ "time": 0.0 }));
        Ok((layers.len(), comp))
    }

    /// Bring a composition to the front (after the tab switch imported the cut
    /// and opened another one).
    pub fn open_comp(&mut self, comp: u64) -> Result<(), String> {
        self.app
            .session
            .execute("comp.open", json!({ "comp": comp }))
            .map(|_| ())
            .map_err(|error| format!("EffectCraft could not open the composition: {error}"))
    }

    /// Render the open composition, animation included, to a video file.
    ///
    /// This is the return path to Edit: EffectCraft renders the comp it is
    /// showing, then the shell imports that file and places it on the FilmCraft
    /// timeline. Rendering runs synchronously (the button can take a while).
    pub fn render_active_comp(&mut self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        self.app
            .session
            .execute(
                "render.saveCurrentPreview",
                json!({ "path": path.to_string_lossy() }),
            )
            .map(|_| ())
            .map_err(|error| format!("EffectCraft could not render the composition: {error}"))
    }

    /// How many layers the open composition has (for tests).
    #[cfg(test)]
    pub fn active_comp_layers(&self) -> Option<usize> {
        self.app.session.active_comp().map(|comp| comp.layers.len())
    }

    /// The composition EffectCraft opened last, for the shell's strip.
    pub fn open_composition(&self) -> Option<String> {
        let id = self.app.session.active_comp_id()?;
        self.app
            .session
            .project
            .item(id)
            .map(|item| item.name.clone())
    }

    /// Render the frame under EffectCraft's playhead and return it as a PNG.
    ///
    /// This is the same renderer the Composition panel draws with, so Photo
    /// receives exactly what the viewer shows, effects and all.
    pub fn current_frame_png(&self) -> Result<(Vec<u8>, u32, u32), String> {
        let session = &self.app.session;
        let comp = session
            .active_comp_id()
            .ok_or_else(|| "Open a composition in EffectCraft first.".to_string())?;
        let time = session.time();
        let (width, height, rgba) = session
            .render_rgba8(comp, time, 0)
            .map_err(|error| format!("EffectCraft could not render the frame: {error}"))?;
        let image = image::RgbaImage::from_raw(width, height, rgba)
            .ok_or_else(|| "EffectCraft returned a malformed frame.".to_string())?;
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|error| format!("Could not encode the EffectCraft frame: {error}"))?;
        Ok((png, width, height))
    }

    /// Import the FilmCraft cut as a composition.
    ///
    /// `force` imports even when the cut has not changed (the manual "Sync
    /// cut" button). Every import is a new composition, so effect work done on
    /// an earlier version is never destroyed.
    #[cfg(feature = "filmcraft")]
    pub fn sync_from_edit(
        &mut self,
        filmcraft: &mut filmcraft_engine::Session,
        force: bool,
    ) -> Result<LinkReport, String> {
        let cut = cut_signature(filmcraft);
        if !force && self.linked_cut.as_deref() == Some(cut.as_str()) {
            return Ok(LinkReport {
                summary: "Already matches the Edit timeline".to_string(),
                detail: "The composition was imported from this cut and the timeline \
                         has not changed since. Use Sync cut to import it again anyway."
                    .to_string(),
            });
        }

        let name = sequence_name(filmcraft).unwrap_or_else(|| "Edit timeline".to_string());
        let path = link_path(&name)
            .map_err(|error| format!("CreativeCraft could not prepare the link folder: {error}"))?;

        // FilmCraft writes the same interchange file Premiere Pro would read.
        filmcraft
            .execute(
                "file.exportFcp7Xml",
                json!({ "path": path.to_string_lossy() }),
            )
            .map_err(|error| format!("FilmCraft could not export the cut: {error}"))?;

        // EffectCraft imports it as a composition: tracks become layers, clips
        // become footage, transforms and opacity keep their keyframes.
        let result = self
            .app
            .session
            .execute(
                "file.importTimeline",
                json!({ "path": path.to_string_lossy(), "format": "xml" }),
            )
            .map_err(|error| format!("EffectCraft could not import the cut: {error}"))?;

        self.linked_cut = Some(cut);
        self.imports = self.imports.saturating_add(1);

        let comps = result["comps"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default();
        let missing = result["missing"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default();
        let warnings = result["warnings"]
            .as_array()
            .map(|items| items.len())
            .unwrap_or_default();
        let version = if self.imports > 1 {
            format!(" (version {})", self.imports)
        } else {
            String::new()
        };

        if comps == 0 {
            return Err("The cut imported no composition.".to_string());
        }
        // Photos come in at their own size; fit them to the frame the way
        // FilmCraft's monitor shows them.
        unsequence_stills(&mut self.app.session);
        fit_stills_to_frame(&mut self.app.session);
        // Open on a frame that has a picture, preferring real media (a photo or
        // footage the user placed) over the cut's other layers. A cut that
        // starts with a fade would otherwise show frame 0 — black — and look
        // like an empty viewer.
        let open_at = self
            .app
            .session
            .active_comp()
            .map(|comp| open_time_and_layer(comp))
            .unwrap_or((0.0, None));
        if open_at.0 > 0.0 {
            let _ = self
                .app
                .session
                .execute("time.set", json!({ "time": open_at.0 }));
        }
        // Select a visible layer so Effect Controls has something to show too.
        let selection = open_at
            .1
            .map(|index| format!("#{}", index + 1))
            .unwrap_or_else(|| "#1".to_string());
        let _ = self
            .app
            .session
            .execute("layer.select", json!({ "layers": [selection] }));
        let media = if missing == 0 {
            "all media found".to_string()
        } else {
            format!("{missing} media file(s) missing — they show as placeholders")
        };
        let notes = if warnings == 0 {
            String::new()
        } else {
            format!(
                " · {warnings} import note{}",
                if warnings == 1 { "" } else { "s" }
            )
        };
        Ok(LinkReport {
            summary: format!("Imported {name}{version} · {media}{notes}"),
            detail: format!(
                "Imported \"{name}\"{version} from the Edit timeline as a composition ({media}). \
                 {notes_hint}\
                 Edit it here and render from EffectCraft's render queue.",
                notes_hint = if warnings == 0 {
                    String::new()
                } else {
                    format!(
                        "{warnings} item(s) had no interchange equivalent and came in as \
                         placeholders (FilmCraft's demo clips are generators, so they arrive \
                         as solids). "
                    )
                }
            ),
        })
    }
}

/// What one cut import produced: a one-line summary for the strip and the full
/// explanation for its tooltip.
#[cfg(feature = "filmcraft")]
pub struct LinkReport {
    pub summary: String,
    pub detail: String,
}

/// Pick the time an imported composition opens on, and the layer to select
/// there: one second into the first layer that carries real media (a photo or
/// footage), falling back to the first layer of any kind.
#[cfg(feature = "filmcraft")]
fn open_time_and_layer(comp: &effectcraft_engine::project::Comp) -> (f64, Option<usize>) {
    use effectcraft_engine::project::LayerSource;

    let earliest = |only_footage: bool| -> Option<f64> {
        comp.layers
            .iter()
            .filter(|layer| !only_footage || matches!(layer.source, LayerSource::Footage { .. }))
            .map(|layer| layer.in_point.seconds())
            .fold(None, |acc, start| {
                Some(acc.map_or(start, |a: f64| a.min(start)))
            })
    };
    let start = earliest(true).or_else(|| earliest(false)).unwrap_or(0.0);
    let time = (start + 1.0).min(comp.duration.seconds().max(0.0));
    let visible = |layer: &effectcraft_engine::project::Layer| {
        layer.in_point.seconds() <= time && time <= layer.out_point.seconds()
    };
    let layer = comp
        .layers
        .iter()
        .position(|layer| visible(layer) && matches!(layer.source, LayerSource::Footage { .. }))
        .or_else(|| comp.layers.iter().position(visible));
    (time, layer)
}

/// The host's file pickers for EffectCraft's own menus (File > Import, Open,
/// Save As, and exports). The standalone app installs these rfd dialogs; a host
/// without them makes those menu items silently do nothing.
fn install_file_dialogs(app: &mut EffectcraftApp) {
    app.hooks.pick_files = Some(Box::new(|exts: &[&str]| {
        rfd::FileDialog::new()
            .add_filter("Media", exts)
            .pick_files()
            .unwrap_or_default()
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect()
    }));
    app.hooks.pick_save = Some(Box::new(|name: &str| {
        rfd::FileDialog::new()
            .add_filter("EffectCraft Project", &["ecproj"])
            .set_file_name(name)
            .save_file()
            .map(|path| path.to_string_lossy().to_string())
    }));
    app.hooks.pick_open_project = Some(Box::new(|| {
        rfd::FileDialog::new()
            .add_filter("EffectCraft Project", &["ecproj", "ecprojx"])
            .pick_file()
            .map(|path| path.to_string_lossy().to_string())
    }));
    app.hooks.pick_folder = Some(Box::new(|| {
        rfd::FileDialog::new()
            .pick_folder()
            .map(|path| path.to_string_lossy().to_string())
    }));
    app.hooks.pick_save_file = Some(Box::new(|name: &str, ext: &str| {
        let path = std::path::Path::new(name);
        let mut dialog = if ext.is_empty() {
            rfd::FileDialog::new()
        } else {
            rfd::FileDialog::new().add_filter(ext, &[ext])
        };
        if let Some(dir) = path.parent().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        let file = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        dialog
            .set_file_name(file)
            .save_file()
            .map(|path| path.to_string_lossy().to_string())
    }));
}

/// Stop EffectCraft from treating a still from the cut as an image sequence.
///
/// The interchange importer probes every referenced file with sequence grouping
/// on (its default), so a still whose name ends in digits — a screenshot called
/// `Screenshot 2026-10-09 084529.png`, or `IMG_1234.jpg` — gets grouped with its
/// numbered neighbours into an image sequence. The layer then shows the wrong
/// frames, and missing ones draw as EffectCraft's bars/missing-media slate. A
/// cut references one file per clip, so read each still exactly as that file.
#[cfg(feature = "filmcraft")]
fn unsequence_stills(session: &mut effectcraft_engine::Session) {
    use effectcraft_engine::project::{FootageKind, ItemKind, LayerSource};

    let Some(comp_id) = session.active_comp_id() else {
        return;
    };
    let Some(comp) = session.project.comp(comp_id) else {
        return;
    };
    let items: Vec<_> = comp
        .layers
        .iter()
        .filter_map(|layer| match layer.source {
            LayerSource::Footage { item } => Some(item),
            _ => None,
        })
        .collect();
    let _ = session.edit("Read stills as stills", None, |project, _state| {
        for item in &items {
            let name = project
                .item(*item)
                .map(|it| it.name.clone())
                .unwrap_or_default();
            let Some(ItemKind::Footage(footage)) = project.item_mut(*item).map(|it| &mut it.kind)
            else {
                continue;
            };
            if footage.kind != FootageKind::Sequence || !is_still_path(&footage.path) {
                continue;
            }
            // The group's path is its first file ("shot 1001"), not the file the
            // cut referenced. The item keeps that file's name, so pick it out.
            if let Some(path) = footage
                .sequence
                .iter()
                .find(|file| same_file_name(file, &name))
                .cloned()
            {
                footage.path = path;
            }
            footage.kind = FootageKind::Still;
            footage.sequence.clear();
            footage.alphabetical = false;
        }
        Ok(())
    });
}

/// Whether `path` names the file `name` (Windows paths compare case-insensitively).
#[cfg(feature = "filmcraft")]
fn same_file_name(path: &str, name: &str) -> bool {
    let file = std::path::Path::new(path)
        .file_name()
        .and_then(|file| file.to_str())
        .unwrap_or_default();
    file.eq_ignore_ascii_case(name)
}

/// Whether a path is a single image file (not a movie).
#[cfg(feature = "filmcraft")]
fn is_still_path(path: &str) -> bool {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "webp"
            | "tif"
            | "tiff"
            | "bmp"
            | "gif"
            | "tga"
            | "exr"
            | "dpx"
            | "psd"
            | "svg"
    )
}

/// Fit stills imported from FilmCraft to the composition frame.
///
/// Final Cut Pro XML carries each layer at its own size with a 100 % scale, so
/// a 1200 × 800 design lands as a small card in the middle of a 1920 × 1080
/// composition — it reads as "the photo is missing". FilmCraft's monitor fits a
/// still to the sequence; this makes the composition agree, like Premiere's
/// "Set to Frame Size". Clips that already carry a scale other than 100 %
/// (scaled by hand in FilmCraft) are left exactly as they are.
#[cfg(feature = "filmcraft")]
fn fit_stills_to_frame(session: &mut effectcraft_engine::Session) {
    use effectcraft_engine::project::{ItemKind, LayerSource, Value};

    let Some(comp_id) = session.active_comp_id() else {
        return;
    };
    let Some(comp) = session.project.comp(comp_id) else {
        return;
    };
    let (frame_w, frame_h) = (f64::from(comp.width), f64::from(comp.height));
    let mut fits: Vec<(u64, f64)> = Vec::new();
    for layer in comp.layers.iter() {
        let LayerSource::Footage { item } = layer.source else {
            continue;
        };
        let Some(ItemKind::Footage(footage)) = session.project.item(item).map(|it| &it.kind) else {
            continue;
        };
        if footage.width == 0 || footage.height == 0 {
            continue;
        }
        let scale =
            (frame_w / f64::from(footage.width)).min(frame_h / f64::from(footage.height)) * 100.0;
        if !scale.is_finite() || scale <= 0.0 || (scale - 100.0).abs() < 0.01 {
            continue;
        }
        let current = layer
            .transform()
            .and_then(|transform| transform.get("scale"))
            .and_then(|property| match &property.value {
                Value::Vec3(values) => Some(values[0]),
                _ => None,
            });
        if current.is_some_and(|value| (value - 100.0).abs() > 0.01) {
            continue;
        }
        fits.push((layer.id.0, scale));
    }
    for (layer, scale) in fits {
        let _ = session.execute(
            "prop.set",
            json!({
                "comp": comp_id.0,
                "layer": layer,
                "path": "transform/scale",
                "value": [scale, scale, 100.0],
            }),
        );
    }
}

/// A stable fingerprint of the active FilmCraft cut. Any clip, name, timing or
/// track change gives a different signature, so "Sync cut" knows when there is
/// something new to import.
#[cfg(feature = "filmcraft")]
fn cut_signature(session: &filmcraft_engine::Session) -> String {
    let mut out = sequence_name(session).unwrap_or_default();
    let Some(sequence) = session.active_sequence() else {
        return out;
    };
    out.push_str(&format!("|{}", sequence.duration().0));
    for track in sequence.video_tracks.iter().chain(&sequence.audio_tracks) {
        out.push_str(&format!("|t{}", track.items.len()));
        for item in &track.items {
            out.push_str(&format!(
                "|{}:{}:{}{}",
                item.name,
                item.start.0,
                item.end().0,
                if item.effects.is_empty() { "" } else { "+fx" }
            ));
        }
    }
    out
}

#[cfg(feature = "filmcraft")]
fn sequence_name(session: &filmcraft_engine::Session) -> Option<String> {
    let id = session.state.active_sequence?;
    session.project.items.get(&id).map(|item| item.name.clone())
}

/// Where the interchange file lives between the two apps.
#[cfg(feature = "filmcraft")]
fn link_path(name: &str) -> std::io::Result<std::path::PathBuf> {
    use std::sync::atomic::{AtomicU32, Ordering};
    // One file per sync: two syncs running at once (tests, or a fast
    // double-click) must not overwrite each other's export between writing and
    // reading it.
    static NEXT: AtomicU32 = AtomicU32::new(1);
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("link");
    std::fs::create_dir_all(&base)?;
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    Ok(base.join(format!("{}-{n}.xml", safe_file_name(name))))
}

#[cfg(feature = "filmcraft")]
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "edit-timeline".to_string()
    } else {
        cleaned
    }
}

#[cfg(all(test, feature = "filmcraft"))]
mod tests {
    use super::*;

    fn demo_session() -> filmcraft_engine::Session {
        let mut session = filmcraft_engine::Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("the FilmCraft demo project opens");
        session
    }

    #[test]
    fn the_cut_signature_changes_when_the_timeline_changes() {
        let mut session = demo_session();
        let before = cut_signature(&session);
        session.set_playhead(filmcraft_engine::time::Tick::from_seconds_f64(1.0));
        assert_eq!(
            before,
            cut_signature(&session),
            "moving the playhead is not a change to the cut"
        );
        assert!(!before.is_empty(), "the signature identifies the sequence");
    }

    #[test]
    fn the_link_file_is_a_safe_xml_path() {
        let path = link_path("My Cut: v2/final?").expect("link folder");
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("xml"));
        assert!(path.to_string_lossy().contains("CreativeCraft"));
    }

    /// The heart of the Dynamic Link claim: the Edit cut, exported as Final Cut
    /// Pro XML, opens in EffectCraft as a composition with the cut's layers.
    #[test]
    fn the_edit_cut_becomes_a_composition_with_its_layers() {
        let mut film = demo_session();
        let path =
            std::env::temp_dir().join(format!("creativecraft-cut-link-{}.xml", std::process::id()));
        film.execute(
            "file.exportFcp7Xml",
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("FilmCraft exports its sequence");
        assert!(path.exists(), "the export wrote a document");

        let mut fx = effectcraft_host::session();
        let result = fx
            .execute(
                "file.importTimeline",
                json!({ "path": path.to_string_lossy(), "format": "xml" }),
            )
            .expect("EffectCraft imports the exported sequence");
        let _ = std::fs::remove_file(&path);

        let comps = result["comps"].as_array().expect("a list of comp ids");
        assert_eq!(comps.len(), 1, "one sequence becomes one composition");

        let comp = fx.active_comp().expect("the import opens its composition");
        assert!(comp.duration.0 > 0, "the composition has the cut's length");
        assert!(comp.layers.len() > 1, "every clip of the cut is a layer");
        // The demo project's clips are FilmCraft generators, which the format can
        // only carry as placeholders; the layer names and timing still match.
        assert!(
            comp.layers.iter().all(|layer| !layer.name.is_empty()),
            "each layer keeps its clip's name"
        );
        assert!(
            comp.layers
                .iter()
                .all(|layer| layer.in_point <= comp.duration && layer.out_point <= comp.duration),
            "the layers sit inside the composition's length"
        );
        for layer in comp.layers.iter().take(2) {
            println!(
                "{} video={} in={} out={} start={} opacity={:?}",
                layer.name,
                layer.switches.video,
                layer.in_point.seconds(),
                layer.out_point.seconds(),
                layer.start_time.seconds(),
                layer
                    .transform()
                    .and_then(|t| t.get("opacity"))
                    .map(|p| p.value.clone())
            );
        }
    }

    /// Real media files (not FilmCraft's generators) arrive as footage, so the
    /// compositor can render and effect them.
    #[test]
    fn imported_media_reaches_effectcraft_as_footage() {
        let media = std::env::temp_dir().join(format!(
            "creativecraft-link-media-{}.png",
            std::process::id()
        ));
        image::RgbaImage::from_pixel(64, 48, image::Rgba([20, 120, 200, 255]))
            .save(&media)
            .expect("a test image is written");

        let mut film = demo_session();
        let imported = film
            .execute("file.import", json!({ "paths": [media.to_string_lossy()] }))
            .expect("FilmCraft imports the test image");
        let item = imported["items"][0]
            .as_u64()
            .expect("the import returns the new project item");
        film.execute(
            "timeline.place",
            json!({ "item": item, "track": "V1", "seconds": 0.0, "insert": true }),
        )
        .expect("the image is placed on the Edit timeline");

        let path = std::env::temp_dir().join(format!(
            "creativecraft-cut-media-{}.xml",
            std::process::id()
        ));
        film.execute(
            "file.exportFcp7Xml",
            json!({ "path": path.to_string_lossy() }),
        )
        .expect("FilmCraft exports the sequence");

        let mut fx = effectcraft_host::session();
        let result = fx
            .execute(
                "file.importTimeline",
                json!({ "path": path.to_string_lossy(), "format": "xml" }),
            )
            .expect("EffectCraft imports the sequence");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&media);

        assert!(
            result["missing"].as_array().is_some_and(Vec::is_empty),
            "the test image resolves at import time: {}",
            result["missing"]
        );
        let comp = fx.active_comp().expect("the import opens its composition");
        let footage = comp
            .layers
            .iter()
            .filter_map(|layer| match layer.source {
                effectcraft_project::LayerSource::Footage { item } => Some(item),
                _ => None,
            })
            .count();
        assert!(
            footage > 0,
            "the placed file became a footage layer, not a placeholder"
        );
    }

    /// Opening a cut must show a picture, not frame 0 of a fade, and leave a
    /// layer selected so Effect Controls is not empty.
    #[test]
    fn importing_a_cut_opens_on_a_frame_with_a_picture() {
        let mut film = demo_session();
        let mut host = crate::effectcraft_host::EffectcraftHost::new().expect("EffectCraft starts");
        host.sync_from_edit(&mut film, false)
            .expect("the cut imports into the hosted app");

        let time = host.app.session.time().seconds();
        let comp = host.app.session.active_comp().expect("composition open");
        let earliest = comp
            .layers
            .iter()
            .map(|layer| layer.in_point.seconds())
            .fold(f64::INFINITY, f64::min);
        assert!(time > 0.0, "the playhead moved off the black frame 0");
        assert!(
            time >= earliest,
            "the playhead sits at or after the first layer"
        );
        assert!(
            time <= comp.duration.seconds(),
            "the playhead stays inside the composition"
        );
    }

    /// A photo placed in Edit must fill the composition the way FilmCraft's
    /// monitor shows it, not sit as a small card in the middle of the frame.
    #[test]
    fn a_photo_from_the_cut_fills_the_frame() {
        use image::{Rgba, RgbaImage};

        // 16:9, so fitting it to a 1920 × 1080 comp touches every edge.
        let (red, blue) = (Rgba([220, 30, 30, 255]), Rgba([30, 60, 220, 255]));
        let mut photo = RgbaImage::from_pixel(640, 360, red);
        for y in 0..360u32 {
            for x in 320..640u32 {
                photo.put_pixel(x, y, blue);
            }
        }
        let media = std::env::temp_dir().join(format!(
            "creativecraft-photo-fit-{}.png",
            std::process::id()
        ));
        photo.save(&media).expect("the test photo saves");

        let mut film = demo_session();
        let imported = film
            .execute("file.import", json!({ "paths": [media.to_string_lossy()] }))
            .expect("FilmCraft imports the photo");
        let item = imported["items"][0].as_u64().expect("an item id");
        // A still needs a length on the timeline.
        film.execute(
            "timeline.place",
            json!({
                "item": item,
                "track": "V1",
                "seconds": 0.0,
                "insert": true,
                "duration": { "seconds": 5.0 },
            }),
        )
        .expect("the photo is placed on the cut");

        let mut host = crate::effectcraft_host::EffectcraftHost::new().expect("EffectCraft starts");
        host.sync_from_edit(&mut film, false)
            .expect("the cut imports");

        let _ = host.app.session.execute("time.set", json!({ "time": 1.0 }));
        let comp_id = host
            .app
            .session
            .active_comp_id()
            .expect("a composition is open");
        let time = host.app.session.time();
        let (w, h, rgba) = host
            .app
            .session
            .render_rgba8(comp_id, time, 0)
            .expect("the composition renders");
        assert_eq!((w, h), (1920, 1080));
        let pixel = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };
        let _ = std::fs::remove_file(&media);
        assert_eq!(
            pixel(6, 6),
            [red.0[0], red.0[1], red.0[2]],
            "the photo reaches the top-left corner instead of leaving black around it"
        );
        assert_eq!(
            pixel(w - 6, h - 6),
            [blue.0[0], blue.0[1], blue.0[2]],
            "the photo reaches the bottom-right corner"
        );
    }

    /// EffectCraft's own File > Import must work inside the shell: the host has
    /// to supply the pickers, and a photo must land in the project and comp.
    #[test]
    fn effectcraft_imports_a_photo_into_its_project() {
        use image::{Rgba, RgbaImage};

        let photo = std::env::temp_dir().join(format!(
            "creativecraft-fx-import-{}.png",
            std::process::id()
        ));
        RgbaImage::from_pixel(48, 32, Rgba([12, 200, 90, 255]))
            .save(&photo)
            .expect("the test photo saves");

        let mut host = crate::effectcraft_host::EffectcraftHost::new().expect("EffectCraft starts");
        let before = host.app.session.active_comp().map(|comp| comp.layers.len());
        let result = host
            .app
            .session
            .execute(
                "file.import",
                json!({
                    "paths": [photo.to_string_lossy()],
                    "addToComp": true,
                }),
            )
            .expect("EffectCraft imports the photo");
        let _ = std::fs::remove_file(&photo);

        assert!(
            result["items"]
                .as_array()
                .is_some_and(|items| !items.is_empty()),
            "the photo became a project item: {result}"
        );
        assert!(
            result["errors"].as_array().is_some_and(Vec::is_empty),
            "no import errors: {result}"
        );
        let after = host.app.session.active_comp().map(|comp| comp.layers.len());
        assert_eq!(
            after,
            before.map(|n| n + 1),
            "the photo was added to the open composition"
        );
    }

    /// A numbered still from the cut — "Screenshot 2026-10-09 084529.png",
    /// "IMG_1234.jpg" — must stay one photo. EffectCraft's interchange probe
    /// groups numbered neighbours into an image sequence, which made the layer
    /// play other files and draw missing frames as its bars slate.
    #[test]
    fn a_numbered_still_from_the_cut_stays_one_photo() {
        use image::{Rgba, RgbaImage};

        let dir = std::env::temp_dir().join(format!("creativecraft-seq-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let shots = [
            (1001u32, [220, 30, 30, 255]),
            (1002, [30, 200, 60, 255]),
            (1003, [30, 60, 220, 255]),
        ];
        for (number, colour) in shots {
            RgbaImage::from_pixel(320, 180, Rgba(colour))
                .save(dir.join(format!("shot {number}.png")))
                .expect("the test photo saves");
        }
        // The middle file, as a screenshot's timestamp would be.
        let media = dir.join("shot 1002.png");

        let mut film = demo_session();
        let imported = film
            .execute("file.import", json!({ "paths": [media.to_string_lossy()] }))
            .expect("FilmCraft imports the photo");
        let item = imported["items"][0].as_u64().expect("an item id");
        film.execute(
            "timeline.place",
            json!({
                "item": item,
                "track": "V2",
                "video": true,
                "audio": false,
                "seconds": 0.0,
                "duration": { "seconds": 5.0 },
            }),
        )
        .expect("the photo is placed");

        let mut host = crate::effectcraft_host::EffectcraftHost::new().expect("EffectCraft starts");
        host.sync_from_edit(&mut film, false)
            .expect("the cut imports");

        let comp_id = host
            .app
            .session
            .active_comp_id()
            .expect("a composition is open");
        let comp = host.app.session.project.comp(comp_id).expect("the comp");
        let (path, kind) = comp
            .layers
            .iter()
            .find_map(|layer| match layer.source {
                effectcraft_engine::project::LayerSource::Footage { item } => host
                    .app
                    .session
                    .project
                    .item(item)
                    .and_then(|it| match &it.kind {
                        effectcraft_engine::project::ItemKind::Footage(footage) => {
                            Some((footage.path.clone(), footage.kind))
                        }
                        _ => None,
                    }),
                _ => None,
            })
            .expect("the photo is in the composition");
        assert!(
            path.contains("shot 1002"),
            "the composition holds the placed photo, not another file: {path}"
        );
        assert_eq!(
            kind,
            effectcraft_engine::project::FootageKind::Still,
            "the numbered file is a still, not an image sequence"
        );

        let _ = host.app.session.execute("time.set", json!({ "time": 1.0 }));
        let time = host.app.session.time();
        let (w, h, rgba) = host
            .app
            .session
            .render_rgba8(comp_id, time, 0)
            .expect("the composition renders");
        let centre = {
            let i = ((h / 2 * w + w / 2) * 4) as usize;
            [rgba[i], rgba[i + 1], rgba[i + 2]]
        };
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            centre,
            [30, 200, 60],
            "the composition shows shot 1002 itself, not a neighbour or bars"
        );
    }

    /// The frame the Composition panel shows can be handed to Photo as a PNG.
    #[test]
    fn the_composition_frame_becomes_a_png() {
        let mut host = crate::effectcraft_host::EffectcraftHost::new().expect("EffectCraft starts");
        let _ = host.app.session.execute("time.set", json!({ "time": 1.0 }));
        let (png, width, height) = host
            .current_frame_png()
            .expect("the hosted compositor renders a frame");
        assert_eq!((width, height), (1920, 1080), "a full-size frame");
        assert!(png.starts_with(b"\x89PNG"), "Photo can read it as a PNG");
        assert!(
            png.len() > 1000,
            "the frame has real pixels, not an empty header"
        );
    }

    /// The whole "design into animation" bridge: each FigCraft element becomes
    /// its own layer in a real EffectCraft composition.
    #[test]
    fn design_layers_become_an_effectcraft_composition() {
        let dir = std::env::temp_dir().join(format!(
            "creativecraft-animate-layers-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let seed = dir.join("design.svg");
        crate::design::DesignState::sample()
            .export_svg_to(&seed)
            .expect("SVG");
        let mut fig = crate::figcraft_host::FigcraftHost::new().expect("FigCraft starts");
        fig.open(&seed).expect("open the design");
        let layers = fig
            .export_layer_pngs(&dir.join("layers"))
            .expect("FigCraft renders each element");

        let mut fx = crate::effectcraft_host::EffectcraftHost::new().expect("EffectCraft starts");
        let (count, comp_id) = fx
            .animate_layers("Design animation", 1200, 800, &layers)
            .expect("the composition is built");
        fx.open_comp(comp_id).expect("the comp opens");
        let comp = fx.app.session.active_comp().expect("the comp opens");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(count, layers.len());
        assert_eq!(
            comp.layers.len(),
            layers.len(),
            "one comp layer per element"
        );
        assert_eq!((comp.width, comp.height), (1200, 800));
    }
}
