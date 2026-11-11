//! CreativeCraft: the studio shell.
//!
//! Two-level navigation, matching the DaVinci + Affinity reference:
//!
//!   Workflow bar:  Edit | Effects | Photo | Audio
//!   Photo opens:   Vector | Pixel | Layout   (the Affinity-style personas)
//!
//! The workflow bar answers "what am I doing?" (cut, effects, design, mix).
//! The persona bar answers "how am I editing this design?" (vectors, pixels, layout).
//!
//! Edit and Audio are FilmCraft's real workspaces over one shared project and
//! timeline. Effects is the real EffectCraft compositor, linked to that same
//! cut through Final Cut Pro XML interchange — the Premiere + After Effects
//! model inside one window. Photo is our shared design document. "Place on
//! timeline" turns the design into a clip on that same FilmCraft timeline.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod design;
#[cfg(feature = "effectcraft")]
mod effectcraft_host;
#[cfg(feature = "figcraft")]
mod figcraft_host;
#[cfg(feature = "filmcraft")]
mod filmcraft_host;
mod layer;
mod maestro_install;
mod maestro_launch;
mod maestro_link;
mod maestro_uninstall;
#[cfg(feature = "photocraft")]
mod photocraft_host;
#[cfg(feature = "filmcraft")]
mod project_store;
#[cfg(feature = "soundcraft")]
mod soundcraft_host;
#[cfg(feature = "vectorcraft")]
mod vectorcraft_host;

use design::Persona;
use eframe::egui;
use egui::{Color32, RichText};

const BAR_BG: Color32 = Color32::from_rgb(24, 24, 28);
const WORKSPACE_BG: Color32 = Color32::from_rgb(32, 32, 38);
const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);
const TEXT_DIM: Color32 = Color32::from_rgb(158, 158, 170);
const TEXT_BRIGHT: Color32 = Color32::from_rgb(240, 240, 245);

/// One tab per workflow: a counterpart for each Craft app, plus Render.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Workspace {
    Edit,
    Effects,
    Photo,
    Audio,
    /// Rendering: FilmCraft's Export screen or EffectCraft's Render Queue. The
    /// panels are the apps' own.
    Render,
    /// 3D: EffectCraft's own Advanced 3D (models, primitives, cameras).
    ThreeD,
    /// Maestro's own web page: the local AI video, image and music studio.
    Maestro,
}

impl Workspace {
    const ALL: [Workspace; 7] = [
        Workspace::Edit,
        Workspace::Effects,
        Workspace::Photo,
        Workspace::Audio,
        Workspace::ThreeD,
        Workspace::Render,
        Workspace::Maestro,
    ];

    fn icon(self) -> &'static str {
        match self {
            Workspace::Edit => "🎞",
            Workspace::Effects => "✨",
            Workspace::Photo => "🎨",
            Workspace::Audio => "🔊",
            Workspace::Render => "🎬",
            Workspace::ThreeD => "🧊",
            Workspace::Maestro => "🎼",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Workspace::Edit => "Edit",
            Workspace::Effects => "Effects",
            Workspace::Photo => "Photo",
            Workspace::Audio => "Audio",
            Workspace::Render => "Render",
            Workspace::ThreeD => "3D",
            Workspace::Maestro => "AI",
        }
    }

    fn app(self) -> &'static str {
        match self {
            Workspace::Edit => "CreativeCraft · Edit",
            Workspace::Effects => "CreativeCraft · Effects",
            Workspace::Photo => "CreativeCraft · Photo",
            Workspace::Audio => "CreativeCraft · Audio",
            Workspace::Render => "CreativeCraft · Render",
            Workspace::ThreeD => "CreativeCraft · 3D",
            Workspace::Maestro => "CreativeCraft · Maestro",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Workspace::Edit => "Video editing on the shared timeline.",
            Workspace::Effects => {
                "EffectCraft: compositions, layers, keyframes and the render queue."
            }
            Workspace::Photo => "The design workspace: one document, three ways to edit it.",
            Workspace::Audio => "SoundCraft audio workstation - the same sequence, mixed",
            Workspace::Render => "Render the cut and the animation: both apps' own output panels.",
            Workspace::ThreeD => "3D: coming soon.",
            Workspace::Maestro => {
                "Maestro: the local AI video, image and music studio, in its own page."
            }
        }
    }
}

/// Which studio the AI tab shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum AiStudio {
    /// ArtCraft's own AI studio — not built yet, so the tab says so.
    Studio,
    /// Maestro: the local AI video, image and music studio.
    #[default]
    Maestro,
}

/// Which app's output panel the Render tab shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RenderSide {
    /// FilmCraft's Export screen: the cut, its presets and queue.
    #[default]
    Cut,
    /// EffectCraft's Render Queue: the composition.
    Animation,
}

/// What a clip dragged out of the Audio tab's Edit timeline carries into
/// SoundCraft. It must be `Send + Sync` because egui stores drag payloads in
/// the context, not in the widget.
#[derive(Clone, Debug)]
struct AudioDrag {
    name: String,
    path: std::path::PathBuf,
    start: f64,
    source_in: f64,
}

struct StudioShell {
    workspace: Workspace,
    persona: Persona,
    design: design::DesignState,
    #[cfg(feature = "filmcraft")]
    filmcraft: Option<filmcraft_host::FilmcraftHost>,
    #[cfg(feature = "filmcraft")]
    filmcraft_error: Option<String>,
    #[cfg(feature = "filmcraft")]
    overlay_serial: u32,
    /// The PNG and media item behind the placed design. One snapshot per
    /// session, so every clip of this design updates together.
    #[cfg(feature = "filmcraft")]
    design_link: Option<DesignLink>,
    /// Names the PNGs handed from the timeline/Effects into Photo.
    frame_serial: u32,
    /// What the last frame hand-off did, shown in the Edit strip.
    frame_status: String,
    /// The real EffectCraft compositor behind the Effects tab.
    #[cfg(feature = "effectcraft")]
    effects: Option<effectcraft_host::EffectcraftHost>,
    #[cfg(feature = "effectcraft")]
    effects_error: Option<String>,
    /// What the link strip says about the last import.
    #[cfg(feature = "effectcraft")]
    effects_status: String,
    /// The full import report, shown as the strip's tooltip.
    #[cfg(feature = "effectcraft")]
    effects_detail: String,
    /// While a visit to Effects is starting: (guard ends at, the window was
    /// maximized before). EffectCraft maximizes a window it thinks is too big
    /// for the screen; that is wrong inside the shell's window.
    #[cfg(feature = "effectcraft")]
    effects_window_guard: Option<(f64, bool)>,
    /// The real VectorCraft behind the Vector persona.
    #[cfg(feature = "vectorcraft")]
    vectorcraft: Option<vectorcraft_host::VectorcraftHost>,
    #[cfg(feature = "vectorcraft")]
    vectorcraft_error: Option<String>,
    /// What the design ↔ VectorCraft link did last.
    #[cfg(feature = "vectorcraft")]
    vector_status: String,
    /// Whether the Vector persona is in front (for the automatic hand-off).
    #[cfg(feature = "vectorcraft")]
    vector_active: bool,
    /// The design revision VectorCraft last opened. When the shared artwork
    /// changes in another app, this is behind and the design is re-opened here.
    #[cfg(feature = "vectorcraft")]
    vector_seen: u64,
    /// The real FigCraft behind the Layout persona.
    #[cfg(feature = "figcraft")]
    figcraft: Option<figcraft_host::FigcraftHost>,
    #[cfg(feature = "figcraft")]
    figcraft_error: Option<String>,
    #[cfg(feature = "figcraft")]
    fig_status: String,
    #[cfg(feature = "figcraft")]
    fig_active: bool,
    #[cfg(feature = "figcraft")]
    fig_seen: u64,
    /// The real PhotoCraft behind the Pixel persona.
    #[cfg(feature = "photocraft")]
    photocraft: Option<photocraft_host::PhotocraftHost>,
    #[cfg(feature = "photocraft")]
    photocraft_error: Option<String>,
    #[cfg(feature = "photocraft")]
    pixel_status: String,
    #[cfg(feature = "photocraft")]
    pixel_active: bool,
    #[cfg(feature = "photocraft")]
    pixel_seen: u64,
    /// The real SoundCraft DAW behind the Audio tab.
    #[cfg(feature = "soundcraft")]
    soundcraft: Option<soundcraft_host::SoundcraftHost>,
    #[cfg(feature = "soundcraft")]
    soundcraft_error: Option<String>,
    /// What the audio link did last, shown in the Audio strip.
    #[cfg(feature = "soundcraft")]
    soundcraft_status: String,
    /// On: the Audio tab is SoundCraft. Off: FilmCraft's own Audio workspace,
    /// for the clip mixer and Essential Sound, which SoundCraft does not have.
    #[cfg(feature = "soundcraft")]
    audio_daw: bool,
    /// Bumped whenever the shared artwork changes. Each hosted app records the
    /// revision it last opened, so switching between them only re-imports when
    /// the artwork really changed (no pointless exports on every tab change).
    design_rev: u64,
    /// The design revision that `artwork.svg` represents. While it is current,
    /// VectorCraft and FigCraft exchange that **editable** SVG instead of a flat
    /// image, so shapes and text stay editable in both.
    vector_art_rev: u64,
    /// On: entering/leaving a persona hands the work over automatically.
    /// Off: only the strip's buttons move work between apps.
    auto_link: bool,
    /// True once the window was asked to close: everything is saved and the
    /// close is let through, so a hosted app cannot hold it with a prompt that
    /// is not even on screen.
    closing: bool,
    /// True until the last project has been reopened. The reopen runs in the
    /// first frame rather than in `new`: building five hosted apps already uses
    /// most of the main thread's stack, and opening a project on top of that
    /// overflowed it (the app died before its window appeared).
    #[cfg(feature = "filmcraft")]
    restore_pending: bool,
    /// Which output panel the Render tab shows.
    render_side: RenderSide,
    /// Which studio the AI tab shows (ArtCraft Studio, or Maestro).
    ai_studio: AiStudio,
    /// The removal of Maestro's folder, while it runs.
    maestro_uninstall: Option<maestro_uninstall::MaestroUninstall>,
    /// The uninstall warning is showing.
    maestro_uninstall_confirm: bool,
    /// The address of Maestro's local server, typed in the Maestro tab.
    maestro_url: String,
    /// What the Maestro tab last did, shown in the tab.
    maestro_status: String,
    /// Where Maestro is installed (its folder with `app/` inside).
    maestro_folder: String,
    /// Maestro's backend, while this tab started it.
    maestro_launch: Option<maestro_launch::MaestroLaunch>,
    /// The optional one-time install of Maestro, while it runs.
    maestro_install: Option<maestro_install::MaestroInstall>,
    /// The pre-install warning is showing (nothing is downloaded until Yes).
    maestro_confirm: bool,
    /// What `nvidia-smi` said about the GPU, once checked.
    maestro_gpu: Option<String>,
    /// The last file this tab wrote for, or took from, Maestro.
    maestro_last_file: String,
    /// Numbers those files: frame-001.png, cut-audio-002.wav, ...
    maestro_serial: u32,
    /// The tab's own area, for placing the app window over it.
    maestro_tab_area: Option<egui::Rect>,
    /// The SoundCraft session to reopen when the DAW starts (it starts lazily,
    /// after the last project was restored).
    #[cfg(feature = "soundcraft")]
    soundcraft_restore: Option<std::path::PathBuf>,
}

#[cfg(feature = "filmcraft")]
struct DesignLink {
    path: std::path::PathBuf,
    item: u64,
    /// Which design document this snapshot belongs to.
    serial: u64,
}

impl StudioShell {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        #[cfg(feature = "filmcraft")]
        let (filmcraft, filmcraft_error) = match filmcraft_host::FilmcraftHost::new(cc) {
            Ok(host) => (Some(host), None),
            Err(error) => (None, Some(error)),
        };
        #[cfg(feature = "effectcraft")]
        let (effects, effects_error) = match effectcraft_host::EffectcraftHost::new() {
            Ok(host) => (Some(host), None),
            Err(error) => (None, Some(error)),
        };
        #[cfg(feature = "vectorcraft")]
        let (vectorcraft, vectorcraft_error) = match vectorcraft_host::VectorcraftHost::new() {
            Ok(host) => (Some(host), None),
            Err(error) => (None, Some(error)),
        };
        #[cfg(feature = "photocraft")]
        let (photocraft, photocraft_error) = match photocraft_host::PhotocraftHost::new() {
            Ok(host) => (Some(host), None),
            Err(error) => (None, Some(error)),
        };
        #[cfg(feature = "figcraft")]
        let (figcraft, figcraft_error) = match figcraft_host::FigcraftHost::new() {
            Ok(host) => (Some(host), None),
            Err(error) => (None, Some(error)),
        };
        let shell = Self {
            workspace: Workspace::Photo,
            persona: Persona::Layout,
            design: design::DesignState::new(),
            #[cfg(feature = "filmcraft")]
            filmcraft,
            #[cfg(feature = "filmcraft")]
            filmcraft_error,
            #[cfg(feature = "filmcraft")]
            overlay_serial: 0,
            #[cfg(feature = "filmcraft")]
            design_link: None,
            frame_serial: 0,
            frame_status: String::new(),
            #[cfg(feature = "effectcraft")]
            effects,
            #[cfg(feature = "effectcraft")]
            effects_error,
            #[cfg(feature = "effectcraft")]
            effects_status: "Open Edit, make a cut, then come back to sync it.".to_string(),
            #[cfg(feature = "effectcraft")]
            effects_detail: "The Effects tab is the real EffectCraft compositor. It imports \
                             the Edit sequence as a composition through Final Cut Pro XML."
                .to_string(),
            #[cfg(feature = "effectcraft")]
            effects_window_guard: None,
            #[cfg(feature = "vectorcraft")]
            vectorcraft,
            #[cfg(feature = "vectorcraft")]
            vectorcraft_error,
            #[cfg(feature = "vectorcraft")]
            vector_status: "The Vector persona is the real VectorCraft, linked to this \
                            design by SVG."
                .to_string(),
            #[cfg(feature = "vectorcraft")]
            vector_active: false,
            #[cfg(feature = "vectorcraft")]
            vector_seen: 0,
            #[cfg(feature = "photocraft")]
            photocraft,
            #[cfg(feature = "photocraft")]
            photocraft_error,
            #[cfg(feature = "photocraft")]
            pixel_status: "The Pixel persona is the real PhotoCraft, linked to this design."
                .to_string(),
            #[cfg(feature = "photocraft")]
            pixel_active: false,
            #[cfg(feature = "photocraft")]
            pixel_seen: 0,
            #[cfg(feature = "figcraft")]
            figcraft,
            #[cfg(feature = "figcraft")]
            figcraft_error,
            #[cfg(feature = "figcraft")]
            fig_status: "The Layout persona is the real FigCraft, linked to this design."
                .to_string(),
            #[cfg(feature = "figcraft")]
            fig_active: false,
            #[cfg(feature = "figcraft")]
            fig_seen: 0,
            #[cfg(feature = "soundcraft")]
            soundcraft: None,
            #[cfg(feature = "soundcraft")]
            soundcraft_error: None,
            #[cfg(feature = "soundcraft")]
            soundcraft_status: "Select an audio clip in Edit, then send it here.".to_string(),
            #[cfg(feature = "soundcraft")]
            audio_daw: true,
            design_rev: 1,
            vector_art_rev: 0,
            auto_link: true,
            closing: false,
            #[cfg(feature = "filmcraft")]
            restore_pending: true,
            render_side: RenderSide::Cut,
            ai_studio: AiStudio::Maestro,
            maestro_uninstall: None,
            maestro_uninstall_confirm: false,
            maestro_url: read_setting("maestro-url"),
            maestro_status: String::new(),
            maestro_folder: read_setting("maestro-folder"),
            maestro_launch: None,
            maestro_install: None,
            maestro_confirm: false,
            maestro_gpu: None,
            maestro_last_file: String::new(),
            maestro_serial: 0,
            maestro_tab_area: None,
            #[cfg(feature = "soundcraft")]
            soundcraft_restore: None,
        };
        shell
    }

    /// Switch workflows and point FilmCraft at its matching workspace.
    fn select_workspace(&mut self, workspace: Workspace) {
        if self.workspace == workspace {
            return;
        }
        // Leaving Photo is the natural sync point: the design is live-linked to
        // its timeline clip, so push the latest pixels before showing the edit.
        #[cfg(feature = "filmcraft")]
        if self.workspace == Workspace::Photo {
            self.sync_design_to_timeline();
        }
        // Leaving the Audio tab stops SoundCraft: two applications must never
        // play at once, and a hidden DAW is easy to forget about.
        #[cfg(feature = "soundcraft")]
        if self.workspace == Workspace::Audio
            && workspace != Workspace::Audio
            && let Some(host) = self.soundcraft.as_mut()
        {
            host.stop();
        }
        self.workspace = workspace;
        #[cfg(feature = "filmcraft")]
        if let Some(host) = self.filmcraft.as_mut() {
            match workspace {
                Workspace::Edit => {
                    host.set_workspace("Editing");
                    // Leaving Render puts the timeline back on screen.
                    host.show_editing();
                }
                Workspace::Audio => {
                    host.set_workspace("Audio");
                    host.focus_workspace("Audio");
                    host.show_editing();
                }
                // The Render tab IS FilmCraft's Export screen: same app, same
                // project, its real preset list and queue.
                Workspace::Render => {
                    host.set_workspace("Editing");
                    host.show_export();
                }
                Workspace::Effects | Workspace::Photo | Workspace::Maestro | Workspace::ThreeD => {
                    host.show_editing()
                }
            }
        }
        // EffectCraft's own Render Queue is the other half of the Render tab.
        #[cfg(feature = "effectcraft")]
        if workspace == Workspace::Render
            && let Some(host) = self.effects.as_mut()
        {
            host.show_render_queue();
        }
        // Effects is EffectCraft, not another FilmCraft workspace: send it the
        // cut the first time the tab opens. Later cuts are pushed with the
        // strip's "Sync cut" button so edits made here are never thrown away.
        #[cfg(all(feature = "filmcraft", feature = "effectcraft"))]
        if workspace == Workspace::Effects {
            self.sync_effects_from_edit(false);
        }
        // SoundCraft is a whole application with an audio device, so it starts
        // when the Audio tab first opens, not with the rest of the shell.
        #[cfg(feature = "soundcraft")]
        if workspace == Workspace::Audio {
            self.ensure_soundcraft();
        }
    }

    /// Start SoundCraft the first time the Audio tab opens.
    #[cfg(feature = "soundcraft")]
    fn ensure_soundcraft(&mut self) {
        if self.soundcraft.is_some() || self.soundcraft_error.is_some() {
            return;
        }
        match soundcraft_host::SoundcraftHost::new() {
            Ok(mut host) => {
                // The last close saved this session in the project folder:
                // reopen it so the mix is there, not an empty DAW.
                if let Some(path) = self.soundcraft_restore.take() {
                    match host.open_session(&path) {
                        Ok(()) => {
                            self.soundcraft_status = format!(
                                "Reopened the SoundCraft session from {} ({} track(s))",
                                path.file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                                host.track_count()
                            );
                        }
                        Err(error) => {
                            self.soundcraft_status =
                                format!("Could not reopen the last SoundCraft session: {error}");
                        }
                    }
                }
                self.soundcraft = Some(host);
            }
            Err(error) => self.soundcraft_error = Some(error),
        }
    }

    /// The folder this project is kept in: one folder per project name, with
    /// each app's own file inside it.
    #[cfg(feature = "filmcraft")]
    fn project_folder(&self) -> std::path::PathBuf {
        let name = self
            .filmcraft
            .as_ref()
            .map(filmcraft_host::FilmcraftHost::project_name)
            .unwrap_or_else(|| "Untitled".to_string());
        project_store::folder_for(&name)
    }

    /// Reopen the folder the last close saved, so the cut and the mix are there
    /// when the window comes back. A project that no longer opens is not fatal:
    /// the window keeps its empty project.
    #[cfg(feature = "filmcraft")]
    fn restore_last_project(&mut self) {
        let Some(folder) = project_store::remembered() else {
            return;
        };
        let mut reopened: Vec<&str> = Vec::new();
        if let Some(path) = project_store::find(&folder, "fcproj")
            && let Some(host) = self.filmcraft.as_mut()
        {
            match host.open_project(&path) {
                Ok(()) => reopened.push("Edit"),
                Err(error) => {
                    eprintln!("CreativeCraft could not reopen {}: {error}", path.display());
                }
            }
        }
        // SoundCraft starts later, when the Audio tab first opens: hand the
        // session over and it is opened then.
        #[cfg(feature = "soundcraft")]
        if let Some(path) = project_store::find(&folder, "scraft") {
            self.soundcraft_restore = Some(path);
            reopened.push("Audio");
        }
        if !reopened.is_empty() {
            self.frame_status = format!(
                "Reopened {} from {}",
                reopened.join(" + "),
                folder.display()
            );
        }
    }

    /// Save every open tab into the project's folder. Runs when the window is
    /// closed: there is no dialog to answer, so the window never goes away
    /// without the work. Only open apps are written — an app that never started
    /// has nothing of its own to save.
    #[cfg(feature = "filmcraft")]
    fn save_everything(&mut self) {
        let folder = self.project_folder();
        if let Err(error) = std::fs::create_dir_all(&folder) {
            eprintln!(
                "CreativeCraft could not create {}: {error}",
                folder.display()
            );
            return;
        }
        let stem = project_store::sanitize(
            &self
                .filmcraft
                .as_ref()
                .map(filmcraft_host::FilmcraftHost::project_name)
                .unwrap_or_else(|| "Untitled".to_string()),
        );
        let mut saved: Vec<&str> = Vec::new();
        let mut failures: Vec<String> = Vec::new();

        if let Some(host) = self.filmcraft.as_mut() {
            let path = folder.join(format!("{stem}.fcproj"));
            match host.save_project(&path) {
                Ok(()) => saved.push("Edit"),
                Err(error) => failures.push(format!("Edit: {error}")),
            }
        }
        #[cfg(feature = "effectcraft")]
        if let Some(host) = self.effects.as_mut() {
            let path = folder.join(format!("{stem}.ecproj"));
            match host.save_project(&path) {
                Ok(()) => saved.push("Effects"),
                Err(error) => failures.push(format!("Effects: {error}")),
            }
        }
        #[cfg(feature = "soundcraft")]
        if let Some(host) = self.soundcraft.as_mut() {
            let path = folder.join(format!("{stem}.scraft"));
            match host.save_session(&path) {
                Ok(()) => saved.push("Audio"),
                Err(error) => failures.push(format!("Audio: {error}")),
            }
        }

        // Wrong or right, this folder is where the next launch looks.
        project_store::remember(&folder);
        eprintln!(
            "CreativeCraft saved {} to {}{}",
            if saved.is_empty() {
                "nothing".to_string()
            } else {
                saved.join(" + ")
            },
            folder.display(),
            if failures.is_empty() {
                String::new()
            } else {
                format!("; failed: {}", failures.join("; "))
            }
        );
    }

    /// The Render tab: FilmCraft's Export screen and EffectCraft's Render
    /// Queue, side by side or one at a time. Both are the apps' real panels —
    /// the same ones their own menus open — over the same cut.
    fn render_tab(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let mut picked = None;
        egui::Panel::top("render_bar")
            .exact_size(36.0)
            .frame(egui::Frame::new().fill(WORKSPACE_BG))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Render")
                            .strong()
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.add_space(10.0);
                    for (side, label, hint) in [
                        (
                            RenderSide::Cut,
                            "FilmCraft Export",
                            "The cut, in FilmCraft's own Export screen: presets, output settings \
                             and its queue.",
                        ),
                        (
                            RenderSide::Animation,
                            "EffectCraft Render Queue",
                            "The composition, in EffectCraft's own Render Queue panel.",
                        ),
                    ] {
                        let active = self.render_side == side;
                        let text = RichText::new(label).size(11.0).color(if active {
                            TEXT_BRIGHT
                        } else {
                            TEXT_DIM
                        });
                        if ui
                            .add(egui::Button::new(text).selected(active))
                            .on_hover_text(hint)
                            .clicked()
                        {
                            picked = Some(side);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(
                                "Each app renders its own file; a queue keeps running while \
                                 the other side is shown",
                            )
                            .size(10.0)
                            .color(TEXT_DIM),
                        );
                    });
                });
            });
        if let Some(side) = picked {
            self.render_side = side;
            self.focus_render_side();
        }

        match self.render_side {
            RenderSide::Cut => self.render_cut_panel(ui, frame),
            RenderSide::Animation => self.render_animation_panel(ui, frame),
        }
    }

    /// Point the two apps at their output panels. Called when the tab opens and
    /// when a side is picked, so a panel closed in one app comes back.
    fn focus_render_side(&mut self) {
        #[cfg(feature = "filmcraft")]
        if self.render_side == RenderSide::Cut
            && let Some(host) = self.filmcraft.as_mut()
        {
            host.set_workspace("Editing");
            host.show_export();
        }
        #[cfg(feature = "effectcraft")]
        if self.render_side == RenderSide::Animation
            && let Some(host) = self.effects.as_mut()
        {
            host.show_render_queue();
        }
    }

    fn render_cut_panel(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        #[cfg(feature = "filmcraft")]
        if let Some(host) = self.filmcraft.as_mut() {
            // A child panel, not the window's root: FilmCraft lays its own
            // header out from the top, and given the root Ui it would paint
            // over the workflow bar.
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| host.ui(ui, frame));
            return;
        }
        #[cfg(feature = "filmcraft")]
        let error = self
            .filmcraft_error
            .as_deref()
            .unwrap_or("FilmCraft is not available in this build.");
        #[cfg(not(feature = "filmcraft"))]
        let error = "FilmCraft support is not compiled in.";
        unavailable_panel(ui, Workspace::Render, error);
    }

    fn render_animation_panel(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        #[cfg(feature = "effectcraft")]
        if let Some(host) = self.effects.as_mut() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| host.ui(ui, frame));
            return;
        }
        #[cfg(feature = "effectcraft")]
        let error = self
            .effects_error
            .as_deref()
            .unwrap_or("EffectCraft is not available in this build.");
        #[cfg(not(feature = "effectcraft"))]
        let error = "EffectCraft support is not compiled in.";
        unavailable_panel(ui, Workspace::Render, error);
    }

    /// The Maestro tab. Maestro is a local web app (Python + a React page), so
    /// the tab starts its own backend, opens it — in the browser or as a
    /// chrome-less app window over this one — and links files both ways.
    fn maestro_tab(&mut self, ui: &mut egui::Ui) {
        // The AI tab holds two studios: ours (not built yet) and Maestro.
        let mut studio_pick = None;
        egui::Panel::top("ai_studios")
            .exact_size(36.0)
            .frame(egui::Frame::new().fill(WORKSPACE_BG))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new("AI").strong().size(12.0).color(TEXT_BRIGHT));
                    ui.add_space(10.0);
                    for (studio, label) in [
                        (AiStudio::Studio, "ArtCraft Studio"),
                        (AiStudio::Maestro, "Maestro"),
                    ] {
                        let active = self.ai_studio == studio;
                        let text = RichText::new(label).size(11.0).color(if active {
                            TEXT_BRIGHT
                        } else {
                            TEXT_DIM
                        });
                        if ui.add(egui::Button::new(text).selected(active)).clicked() {
                            studio_pick = Some(studio);
                        }
                    }
                });
            });
        if let Some(studio) = studio_pick {
            self.ai_studio = studio;
        }
        if self.ai_studio == AiStudio::Studio {
            let mut studio_web = false;
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("ArtCraft Studio")
                                .strong()
                                .size(18.0)
                                .color(TEXT_BRIGHT),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(
                                "Coming soon: our own AI studio for video, images and music. \
                                 Maestro is the studio next to this one.",
                            )
                            .size(12.0)
                            .color(TEXT_DIM),
                        );
                        ui.add_space(16.0);
                        if ui
                            .button(
                                RichText::new("Open app.getartcraft.com in your browser")
                                    .size(12.0),
                            )
                            .clicked()
                        {
                            studio_web = true;
                        }
                    });
                });
            if studio_web {
                open_in_browser("https://app.getartcraft.com");
            }
            return;
        }

        let mut open = false;
        let mut start = false;
        let mut stop = false;
        let mut uninstall = false;
        let install = false;
        let mut confirm = false;
        let mut stop_install = false;
        let mut folder = false;
        let mut frame_in = false;
        let mut design_in = false;
        let mut audio_in = false;
        let mut newest_in = false;
        let mut copy_path = false;
        egui::Panel::top("maestro_bar")
            .exact_size(104.0)
            .frame(egui::Frame::new().fill(WORKSPACE_BG))
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Maestro")
                            .strong()
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.add_space(10.0);
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.maestro_url)
                            .desired_width(260.0)
                            .hint_text("http://127.0.0.1:PORT"),
                    );
                    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        open = true;
                    }
                    if ui.button(RichText::new("Open").size(11.0)).clicked() {
                        open = true;
                    }
                    let running = self
                        .maestro_launch
                        .as_mut()
                        .is_some_and(maestro_launch::MaestroLaunch::running);
                    if running {
                        if ui
                            .button(RichText::new("Stop Maestro").size(11.0))
                            .clicked()
                        {
                            stop = true;
                        }
                    } else if ui
                        .button(RichText::new("Start Maestro").size(11.0))
                        .on_hover_text(
                            "Run Maestro's own backend from its folder. No Pinokio needed once \
                             it is installed.",
                        )
                        .clicked()
                    {
                        start = true;
                    }
                    // Install only when it is not there yet: nothing is bundled,
                    // and this is the one optional, on-demand step.
                    if self.maestro_install.is_some() {
                        if ui
                            .button(RichText::new("Stop install").size(11.0))
                            .clicked()
                        {
                            stop_install = true;
                        }
                    } else if ui
                        .button(RichText::new("Install / repair").size(11.0))
                        .on_hover_text(
                            "Set Maestro up, or repair a half-finished setup: clone it if the \
                             folder is empty, create its Python environment, install its packages \
                             (torch and the rest) and build its interface. Models download later, \
                             by Maestro itself, when a model is first used.",
                        )
                        .clicked()
                    {
                        confirm = true;
                    }
                    if self.maestro_uninstall.is_some() {
                        ui.label(RichText::new("Removing...").size(10.0).color(TEXT_DIM));
                    } else if self.maestro_folder_path().is_dir()
                        && ui
                            .button(RichText::new("Uninstall").size(11.0))
                            .on_hover_text(
                                "Delete Maestro's folder: the app, its Python environment and \
                                 every model it downloaded. This cannot be undone.",
                            )
                            .clicked()
                    {
                        uninstall = true;
                    }
                    if ui
                        .button(RichText::new("Model folder").size(11.0))
                        .clicked()
                    {
                        folder = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new("Install folder").size(10.0).color(TEXT_DIM));
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.maestro_folder)
                            .desired_width(520.0)
                            .hint_text("%USERPROFILE%\\pinokio\\api\\Maestro.git"),
                    );
                    if field.lost_focus() {
                        save_setting("maestro-folder", &self.maestro_folder);
                    }
                    if let Some(launch) = &self.maestro_launch {
                        let tail = launch.tail();
                        if !tail.is_empty() {
                            ui.label(RichText::new(tail).size(9.0).color(TEXT_DIM));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new("To Maestro").size(10.0).color(TEXT_DIM));
                    if ui
                        .button(RichText::new("Frame from Edit").size(11.0))
                        .on_hover_text("Export the frame under the Edit playhead as a PNG.")
                        .clicked()
                    {
                        frame_in = true;
                    }
                    if ui
                        .button(RichText::new("Design from Photo").size(11.0))
                        .on_hover_text("Export the shared design as a PNG.")
                        .clicked()
                    {
                        design_in = true;
                    }
                    if ui
                        .button(RichText::new("Audio from Edit").size(11.0))
                        .on_hover_text("Export the cut's whole audio as a WAV.")
                        .clicked()
                    {
                        audio_in = true;
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new("From Maestro").size(10.0).color(TEXT_DIM));
                    if ui
                        .button(RichText::new("Newest on timeline").size(11.0))
                        .on_hover_text(
                            "Place the newest video, image or audio Maestro wrote on the Edit \
                             timeline (its outputs folder is searched).",
                        )
                        .clicked()
                    {
                        newest_in = true;
                    }
                    if !self.maestro_last_file.is_empty() {
                        if ui
                            .button(RichText::new("Copy path").size(11.0))
                            .on_hover_text(&self.maestro_last_file)
                            .clicked()
                        {
                            copy_path = true;
                        }
                    }
                });
            });
        if open {
            let url = self.maestro_url.trim().to_string();
            if url.is_empty() {
                self.maestro_status =
                    "Type or start Maestro first: its address appears here when it is ready."
                        .to_string();
            } else {
                save_setting("maestro-url", &url);
                open_in_browser(&url);
            }
        }
        if start {
            self.start_maestro();
        }
        if confirm {
            // Nothing is downloaded until the warning is answered.
            self.maestro_gpu = Self::nvidia_summary();
            self.maestro_confirm = true;
        }
        if install {
            self.install_maestro();
        }
        if uninstall {
            self.maestro_uninstall_confirm = true;
        }
        if stop_install {
            if let Some(mut running) = self.maestro_install.take() {
                running.stop();
            }
            self.maestro_status = "Install stopped.".to_string();
        }
        if stop {
            if let Some(mut launch) = self.maestro_launch.take() {
                launch.stop();
            }
            self.maestro_status = "Maestro stopped.".to_string();
        }
        if folder {
            let path = self.maestro_folder_path();
            if path.is_dir() {
                let _ = open_in_explorer(&path);
            } else {
                self.maestro_status = format!(
                    "No Maestro folder at {}. Install it once, then set the folder above.",
                    path.display()
                );
            }
        }
        if frame_in {
            self.export_to_maestro("frame");
        }
        if design_in {
            self.export_to_maestro("design");
        }
        if audio_in {
            self.export_to_maestro("audio");
        }
        if newest_in {
            self.place_newest_from_maestro();
        }
        if copy_path {
            ui.ctx().copy_text(self.maestro_last_file.clone());
            self.maestro_status = "Path copied - paste or drag it into Maestro.".to_string();
        }

        let area = ui.available_rect_before_wrap();
        self.maestro_tab_area = Some(area);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("Maestro runs as a local web app")
                            .strong()
                            .size(16.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Start Maestro runs its own backend. Files go both ways through one \
                             folder: the buttons above write a frame, the design or the cut's \
                             audio there, and Newest on timeline brings Maestro's own output back \
                             to Edit. If Start says a module is missing, the setup was stopped \
                             early - press Install / repair and let it finish.",
                        )
                        .size(12.0)
                        .color(TEXT_DIM),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "In-window view: coming soon. For now Maestro opens in your browser, \
                             started by this tab.",
                        )
                        .size(11.0)
                        .color(ACCENT),
                    );
                    ui.add_space(6.0);
                    if !self.maestro_last_file.is_empty() {
                        ui.label(
                            RichText::new(format!("Last: {}", self.maestro_last_file))
                                .size(10.0)
                                .color(TEXT_DIM),
                        );
                    }
                    if !self.maestro_status.is_empty() {
                        ui.label(RichText::new(&self.maestro_status).size(11.0).color(ACCENT));
                    }
                });
            });

        if self.maestro_confirm {
            let mut answer = None;
            egui::Window::new("Install Maestro?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.set_max_width(470.0);
                    ui.label(
                        RichText::new("This sets Maestro up on this PC. It needs:")
                            .strong()
                            .size(13.0),
                    );
                    ui.add_space(6.0);
                    ui.label(format!(
                        "- NVIDIA GPU: {}",
                        self.maestro_gpu.as_deref().unwrap_or("not checked")
                    ));
                    ui.label("- Python 3.11: installed for you if missing");
                    ui.label("- Node.js + npm: installed for you if missing");
                    ui.label("- git: installed for you if missing");
                    ui.label(
                        "- about 10 GB for the packages and the interface, plus disk for models \
                         (tens of GB; Maestro downloads those itself when a model is first used)",
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "It downloads several GB and can take a long time. You can stop it \
                             at any point and press Install / repair later: finished steps are \
                             kept.",
                        )
                        .size(11.0),
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Yes, install").clicked() {
                            answer = Some(true);
                        }
                        if ui.button("Not now").clicked() {
                            answer = Some(false);
                        }
                    });
                });
            if let Some(yes) = answer {
                self.maestro_confirm = false;
                if yes {
                    self.install_maestro();
                }
            }
        }

        if self.maestro_uninstall_confirm {
            let mut answer = None;
            let folder = self.maestro_folder_path();
            egui::Window::new("Uninstall Maestro?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.set_max_width(470.0);
                    ui.label(RichText::new("This deletes Maestro's folder:").strong());
                    ui.label(
                        RichText::new(folder.display().to_string())
                            .size(11.0)
                            .color(ACCENT),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        "It removes the app, its Python environment and every model it \
                         downloaded (that can be tens of GB). It cannot be undone.",
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            answer = Some(true);
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(false);
                        }
                    });
                });
            if let Some(yes) = answer {
                self.maestro_uninstall_confirm = false;
                if yes {
                    self.start_maestro_uninstall();
                }
            }
        }
    }

    /// Export one thing from this window into the folder Maestro picks files up
    /// from: the frame under the playhead, the shared design, or the cut's audio.
    fn export_to_maestro(&mut self, what: &str) {
        let folder = maestro_link::inbox();
        if let Err(error) = std::fs::create_dir_all(&folder) {
            self.maestro_status = format!("Could not create {}: {error}", folder.display());
            return;
        }
        let written = match what {
            "audio" => {
                self.maestro_serial += 1;
                let path = folder.join(format!("cut-audio-{:03}.wav", self.maestro_serial));
                #[cfg(feature = "filmcraft")]
                {
                    match self.filmcraft.as_mut() {
                        Some(host) => host.export_audio_mix(&path).map(|()| path),
                        None => Err("FilmCraft is not available.".to_string()),
                    }
                }
                #[cfg(not(feature = "filmcraft"))]
                {
                    Err("FilmCraft is not in this build.".to_string())
                }
            }
            "design" => {
                self.maestro_serial += 1;
                let path = folder.join(format!("design-{:03}.png", self.maestro_serial));
                self.design.export_png_to(&path).map(|()| path)
            }
            _ => {
                self.maestro_serial += 1;
                let path = folder.join(format!("frame-{:03}.png", self.maestro_serial));
                #[cfg(feature = "filmcraft")]
                {
                    match self.filmcraft.as_mut() {
                        Some(host) => host.export_frame_png(&path).map(|_| path),
                        None => Err("FilmCraft is not available.".to_string()),
                    }
                }
                #[cfg(not(feature = "filmcraft"))]
                {
                    Err("FilmCraft is not in this build.".to_string())
                }
            }
        };
        match written {
            Ok(path) => {
                self.maestro_status = format!(
                    "Wrote {} - drag it onto Maestro, or use Copy path.",
                    path.display()
                );
                self.maestro_last_file = path.to_string_lossy().into_owned();
            }
            Err(error) => self.maestro_status = error,
        }
    }

    /// Place the newest file Maestro wrote on the Edit timeline: a video as a
    /// clip, an image as a still, audio at the playhead.
    fn place_newest_from_maestro(&mut self) {
        let root = self.maestro_folder_path();
        let Some(path) = maestro_link::newest_export(&root) else {
            self.maestro_status = format!(
                "No video, image or audio found in {} (looked in its outputs folders).",
                root.display()
            );
            return;
        };
        self.maestro_last_file = path.to_string_lossy().into_owned();
        #[cfg(feature = "filmcraft")]
        {
            let Some(host) = self.filmcraft.as_mut() else {
                self.maestro_status = "FilmCraft is not available.".to_string();
                return;
            };
            let placed = match maestro_link::kind(&path) {
                maestro_link::Kind::Video => host.place_render(&path).map(|_| ()),
                maestro_link::Kind::Image => host.place_design(&path).map(|_| ()),
                maestro_link::Kind::Audio => {
                    let at = host
                        .timeline_view()
                        .map(|view| view.playhead)
                        .unwrap_or(0.0);
                    host.place_audio_at(&path, at).map(|_| ())
                }
                maestro_link::Kind::Other => {
                    Err("That file is not a video, image or audio file.".to_string())
                }
            };
            match placed {
                Ok(()) => {
                    self.maestro_status =
                        format!("Placed {} on the Edit timeline.", path.display());
                }
                Err(error) => self.maestro_status = error,
            }
        }
        #[cfg(not(feature = "filmcraft"))]
        {
            self.maestro_status = "FilmCraft is not in this build.".to_string();
        }
    }

    /// A one-line summary of the NVIDIA GPU for the install warning.
    fn nvidia_summary() -> Option<String> {
        let output = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total,driver_version",
                "--format=csv,noheader",
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() || text.is_empty() {
            Some("No NVIDIA GPU found — Maestro is CUDA-only.".to_string())
        } else {
            Some(text)
        }
    }

    /// Delete Maestro's folder in the background (backend and any install stop
    /// first, so no file is in use).
    fn start_maestro_uninstall(&mut self) {
        if let Some(mut launch) = self.maestro_launch.take() {
            launch.stop();
        }
        if let Some(mut install) = self.maestro_install.take() {
            install.stop();
        }
        let folder = self.maestro_folder_path();
        self.maestro_uninstall = Some(maestro_uninstall::MaestroUninstall::start(&folder));
        self.maestro_status = format!("Removing {} ...", folder.display());
    }

    /// Say when the removal is finished, keeping the window awake meanwhile.
    fn poll_maestro_uninstall(&mut self, ctx: &egui::Context) {
        let Some(removal) = &self.maestro_uninstall else {
            return;
        };
        match removal.finished() {
            Some(Ok(())) => {
                self.maestro_uninstall = None;
                self.maestro_status =
                    "Maestro removed. Install / repair brings it back.".to_string();
                save_setting("maestro-url", "");
            }
            Some(Err(error)) => {
                self.maestro_uninstall = None;
                self.maestro_status = error;
            }
            None => ctx.request_repaint_after(std::time::Duration::from_millis(500)),
        }
    }

    /// Optional, one-time: run Maestro's own setup steps into its folder.
    fn install_maestro(&mut self) {
        let folder = self.maestro_folder_path();
        match maestro_install::MaestroInstall::start(&folder) {
            Ok(install) => {
                save_setting("maestro-folder", &folder.to_string_lossy());
                self.maestro_install = Some(install);
                self.maestro_status =
                    "Installing Maestro: tools, clone, Python 3.11, packages, CUDA torch, \
                     interface. This is the one-time step."
                        .to_string();
            }
            Err(error) => self.maestro_status = error,
        }
    }

    /// Follow the installer: show its last line, and say when it is done.
    fn poll_maestro_install(&mut self) {
        let Some(install) = self.maestro_install.as_mut() else {
            return;
        };
        let tail = install.tail();
        if !tail.is_empty() {
            self.maestro_status = tail;
        }
        if let Some(ok) = install.finished() {
            self.maestro_install = None;
            self.maestro_status = if ok {
                "Maestro is installed - press Start Maestro.".to_string()
            } else {
                "The install stopped with an error (the last line above). Fix it, then press \
                 Install again."
                    .to_string()
            };
        }
    }

    /// Start Maestro's backend from its install folder, if it is not running.
    fn start_maestro(&mut self) {
        if self
            .maestro_launch
            .as_mut()
            .is_some_and(maestro_launch::MaestroLaunch::running)
        {
            self.maestro_status = "Maestro is already running.".to_string();
            return;
        }
        let folder = self.maestro_folder_path();
        match maestro_launch::MaestroLaunch::start(&folder) {
            Ok(launch) => {
                save_setting("maestro-folder", &folder.to_string_lossy());
                self.maestro_launch = Some(launch);
                self.maestro_status =
                    "Starting Maestro... the first start of a model takes a while.".to_string();
            }
            Err(error) => self.maestro_status = error,
        }
    }

    /// The Maestro install folder: the one typed, or Pinokio's usual place.
    fn maestro_folder_path(&self) -> std::path::PathBuf {
        let typed = self.maestro_folder.trim();
        if typed.is_empty() {
            default_maestro_folder()
        } else {
            std::path::PathBuf::from(typed)
        }
    }

    /// Take the address Maestro printed once it is ready, and open it.
    fn poll_maestro(&mut self, ctx: &egui::Context) {
        let Some(launch) = self.maestro_launch.as_mut() else {
            return;
        };
        // Maestro may re-launch itself in a child process, so take the printed
        // address when there is one and, failing that, the port answering.
        let address = launch.address().or_else(|| {
            launch
                .listening()
                .then(|| format!("http://127.0.0.1:{}/", launch.port()))
        });
        match &address {
            Some(found) => {
                if self.maestro_url != *found {
                    self.maestro_url = found.clone();
                    save_setting("maestro-url", found);
                    self.maestro_status = format!("Maestro is ready at {found}");
                    open_in_browser(found);
                }
            }
            None => {
                // Nothing to show yet: keep the window awake so this check runs
                // even when the pointer is still (egui otherwise stops
                // repainting and the address is only noticed on a mouse move).
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
        }
        if address.is_none() && !launch.running() {
            let tail = launch.tail();
            self.maestro_status = if tail.is_empty() {
                "Maestro stopped before printing an address. Check its folder and Python \
                 environment."
                    .to_string()
            } else {
                // torch (and the rest) missing is the common half-finished
                // setup: say what to press instead of only echoing the traceback.
                let missing_module =
                    tail.contains("No module named") || tail.contains("ModuleNotFoundError");
                if missing_module {
                    format!(
                        "Maestro stopped: {tail} - its Python packages are missing. Press \
                         Install / repair above (it runs pip in that environment)."
                    )
                } else {
                    format!("Maestro stopped: {tail}")
                }
            };
        }
    }

    /// Export the Edit cut and import it in EffectCraft as a composition.
    ///
    /// Both apps already speak Final Cut Pro XML, the interchange Premiere and
    /// After Effects use, so this is a real link rather than a copy of UI
    /// widgets. `force` re-imports even when the cut has not changed.
    #[cfg(all(feature = "filmcraft", feature = "effectcraft"))]
    fn sync_effects_from_edit(&mut self, force: bool) {
        let Some(filmcraft) = self.filmcraft.as_mut() else {
            self.effects_status =
                "FilmCraft is not available, so there is no cut to link.".to_string();
            self.effects_detail.clear();
            return;
        };
        let Some(effects) = self.effects.as_mut() else {
            return;
        };
        match effects.sync_from_edit(filmcraft.session_mut(), force) {
            Ok(report) => {
                self.effects_status = report.summary;
                self.effects_detail = report.detail;
            }
            Err(error) => {
                self.effects_status = error.clone();
                self.effects_detail = error;
            }
        }
    }

    /// Send the frame under the Edit playhead to Photo as a photo layer. The
    /// reverse of "Place on timeline": the cut comes into the design document
    /// instead of the design going onto the cut.
    #[cfg(feature = "filmcraft")]
    fn send_edit_frame_to_photo(&mut self) {
        let timecode = self
            .filmcraft
            .as_ref()
            .and_then(|host| host.timeline_view())
            .map(|view| timeline_time(view.playhead))
            .unwrap_or_default();
        let path = frame_path("edit-frame", self.frame_serial);
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            self.design.doc.status = format!("Could not create {}: {error}", parent.display());
            return;
        }
        let exported = match self.filmcraft.as_mut() {
            Some(host) => host.export_frame_png(&path),
            None => Err(self
                .filmcraft_error
                .clone()
                .unwrap_or_else(|| "FilmCraft is not available.".to_string())),
        };
        match exported {
            Ok((width, height)) => {
                self.frame_serial = self.frame_serial.wrapping_add(1);
                self.design.import_photo_path(&path);
                self.design_rev = self.design_rev.wrapping_add(1);
                self.design.doc.status =
                    format!("Imported the Edit frame at {timecode} ({width} × {height})");
                self.maestro_status = format!("Sent the frame at {timecode} to Photo");
                self.select_workspace(Workspace::Photo);
            }
            Err(error) => {
                self.frame_status = error.clone();
                self.design.doc.status = error;
            }
        }
    }

    /// Send the frame under EffectCraft's playhead to Photo. This is what the
    /// Composition panel shows, rendered at full size, so effects and keyframes
    /// arrive with it.
    #[cfg(all(feature = "filmcraft", feature = "effectcraft"))]
    fn send_effects_frame_to_photo(&mut self) {
        let rendered = match self.effects.as_ref() {
            Some(host) => host.current_frame_png(),
            None => Err("EffectCraft is not available.".to_string()),
        };
        match rendered {
            Ok((png, width, height)) => {
                let path = frame_path("effectcraft-frame", self.frame_serial);
                if let Some(parent) = path.parent()
                    && let Err(error) = std::fs::create_dir_all(parent)
                {
                    self.effects_status = format!("Could not create {}: {error}", parent.display());
                    return;
                }
                if let Err(error) = std::fs::write(&path, png) {
                    self.effects_status = format!("Could not save {}: {error}", path.display());
                    return;
                }
                self.frame_serial = self.frame_serial.wrapping_add(1);
                self.design.import_photo_path(&path);
                self.design_rev = self.design_rev.wrapping_add(1);
                self.design.doc.status =
                    format!("Imported the EffectCraft frame ({width} × {height})");
                self.select_workspace(Workspace::Photo);
            }
            Err(error) => self.effects_status = error,
        }
    }

    /// The current editable vector artwork.
    ///
    /// VectorCraft and FigCraft read and write this one SVG, so a shape or text
    /// layer deleted in either is really gone in the other. `force` re-seeds it
    /// from the shared design (the strip's **Design to ...** buttons); otherwise
    /// a file that already matches the design is reused, keeping the edits.
    #[cfg(any(feature = "vectorcraft", feature = "figcraft"))]
    fn vector_artwork(&mut self, force: bool) -> Result<std::path::PathBuf, String> {
        let path = shared_dir().join("artwork.svg");
        if !force && self.vector_art_rev == self.design_rev && path.exists() {
            return Ok(path);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        self.design.export_svg_to(&path)?;
        self.vector_art_rev = self.design_rev;
        Ok(path)
    }

    /// Open the current artwork in the hosted VectorCraft. The strip's
    /// **Design to VectorCraft** re-seeds it from the design.
    #[cfg(feature = "vectorcraft")]
    fn sync_design_to_vector(&mut self) {
        let path = match self.vector_artwork(true) {
            Ok(path) => path,
            Err(error) => {
                self.vector_status = error;
                return;
            }
        };
        let opened = match self.vectorcraft.as_mut() {
            Some(host) => host.open(&path),
            None => Err(self
                .vectorcraft_error
                .clone()
                .unwrap_or_else(|| "VectorCraft is not available.".to_string())),
        };
        match opened {
            Ok(()) => {
                self.vector_status = "Design opened in VectorCraft".to_string();
                self.vector_seen = self.design_rev;
            }
            Err(error) => self.vector_status = error,
        }
    }

    /// Entering the Vector persona: open the shared editable artwork when it
    /// changed since this app last saw it (the first entry always does).
    #[cfg(feature = "vectorcraft")]
    fn enter_vector(&mut self) {
        if self.vector_seen == self.design_rev {
            return;
        }
        let path = match self.vector_artwork(false) {
            Ok(path) => path,
            Err(error) => {
                self.vector_status = error;
                return;
            }
        };
        let opened = match self.vectorcraft.as_mut() {
            Some(host) => host.open(&path),
            None => Err(self
                .vectorcraft_error
                .clone()
                .unwrap_or_else(|| "VectorCraft is not available.".to_string())),
        };
        match opened {
            Ok(()) => {
                self.vector_status = "Design opened in VectorCraft".to_string();
                self.vector_seen = self.design_rev;
            }
            Err(error) => self.vector_status = error,
        }
    }

    /// Leaving the Vector persona: if artwork was drawn or changed, write the
    /// editable SVG for FigCraft and put its PNG into the design.
    #[cfg(feature = "vectorcraft")]
    fn leave_vector(&mut self) {
        let edited = self
            .vectorcraft
            .as_ref()
            .is_some_and(vectorcraft_host::VectorcraftHost::has_unsaved_artwork);
        if edited {
            self.return_vector_artwork(false);
        }
    }

    /// Write VectorCraft's artwork back: SVG for FigCraft, PNG for the design.
    #[cfg(feature = "vectorcraft")]
    fn return_vector_artwork(&mut self, show_layout: bool) {
        let svg = shared_dir().join("artwork.svg");
        let svg_written = match self.vectorcraft.as_mut() {
            Some(host) => host.export_svg(&svg),
            None => Err("VectorCraft is not available.".to_string()),
        };
        let folder = vector_dir().join(format!("export-{:03}", self.frame_serial));
        let exported = match self.vectorcraft.as_mut() {
            Some(host) => host.export_png(&folder),
            None => Err("VectorCraft is not available.".to_string()),
        };
        match exported {
            Ok(png) => {
                self.frame_serial = self.frame_serial.wrapping_add(1);
                match self.design.replace_with_artwork(&png) {
                    Ok(_) => {
                        self.design_rev = self.design_rev.wrapping_add(1);
                        self.vector_seen = self.design_rev;
                        self.design.doc.status =
                            "Updated the design with VectorCraft's artwork".into();
                        match svg_written {
                            Ok(()) => {
                                self.vector_art_rev = self.design_rev;
                                self.vector_status =
                                    "Artwork added - still editable in FigCraft".to_string();
                            }
                            Err(error) => {
                                self.vector_art_rev = 0;
                                self.vector_status =
                                    format!("Artwork added; SVG link failed: {error}");
                            }
                        }
                        if show_layout {
                            self.persona = Persona::Layout;
                        }
                    }
                    Err(error) => self.vector_status = error,
                }
            }
            Err(error) => self.vector_status = error,
        }
    }

    /// Open VectorCraft's layers in PhotoCraft, one image layer each.
    #[cfg(all(feature = "vectorcraft", feature = "photocraft"))]
    fn send_vector_to_pixel(&mut self) {
        let folder = vector_dir().join(format!("to-pixel-{:03}", self.frame_serial));
        self.frame_serial = self.frame_serial.wrapping_add(1);
        let layers = match self.vectorcraft.as_mut() {
            Some(host) => host.export_layer_pngs(&folder),
            None => Err("VectorCraft is not available.".to_string()),
        };
        let layers = match layers {
            Ok(layers) => layers,
            Err(error) => {
                self.vector_status = error;
                return;
            }
        };
        let (width, height) = self.canvas_px();
        let opened = match self.photocraft.as_mut() {
            Some(host) => host.replace_with_layers(width, height, &layers),
            None => Err("PhotoCraft is not available.".to_string()),
        };
        match opened {
            Ok(count) => {
                self.pixel_seen = self.design_rev;
                self.pixel_status = format!("{count} VectorCraft layer(s) opened in PhotoCraft");
                self.persona = Persona::Pixel;
            }
            Err(error) => self.vector_status = error,
        }
    }

    #[cfg(not(all(feature = "vectorcraft", feature = "photocraft")))]
    fn send_vector_to_pixel(&mut self) {
        self.vector_status = "PhotoCraft is not in this build.".to_string();
    }

    /// Write VectorCraft's editable SVG and open it in FigCraft.
    #[cfg(all(feature = "vectorcraft", feature = "figcraft"))]
    fn send_vector_layers_to_fig(&mut self) {
        let svg = shared_dir().join("artwork.svg");
        let written = match self.vectorcraft.as_mut() {
            Some(host) => host.export_svg(&svg),
            None => Err("VectorCraft is not available.".to_string()),
        };
        match written {
            Ok(()) => {
                self.vector_art_rev = self.design_rev;
                let opened = match self.figcraft.as_mut() {
                    Some(host) => host.open(&svg),
                    None => Err("FigCraft is not available.".to_string()),
                };
                match opened {
                    Ok(()) => {
                        self.fig_seen = self.design_rev;
                        self.fig_status = "VectorCraft layers opened in FigCraft".to_string();
                        self.persona = Persona::Layout;
                    }
                    Err(error) => self.vector_status = error,
                }
            }
            Err(error) => self.vector_status = error,
        }
    }

    #[cfg(not(all(feature = "vectorcraft", feature = "figcraft")))]
    fn send_vector_layers_to_fig(&mut self) {
        self.vector_status = "FigCraft is not in this build.".to_string();
    }

    /// The strip's **Artwork to Photo**: same as the automatic return, but shows
    /// the Layout persona so the result is visible.
    #[cfg(feature = "vectorcraft")]
    fn send_vector_to_photo(&mut self) {
        self.return_vector_artwork(true);
    }

    /// Place the VectorCraft artboard on the FilmCraft timeline at the playhead.
    #[cfg(all(feature = "vectorcraft", feature = "filmcraft"))]
    fn place_vector_on_timeline(&mut self) {
        let folder = vector_dir().join(format!("timeline-{:03}", self.frame_serial));
        let exported = match self.vectorcraft.as_mut() {
            Some(host) => host.export_png(&folder),
            None => Err("VectorCraft is not available.".to_string()),
        };
        let png = match exported {
            Ok(png) => png,
            Err(error) => {
                self.vector_status = error;
                return;
            }
        };
        let layer_folder = vector_dir().join(format!("layers-{:03}", self.frame_serial));
        let layers = match self.vectorcraft.as_mut() {
            Some(host) => host.export_layer_pngs(&layer_folder),
            None => Err("VectorCraft is not available.".to_string()),
        };
        self.frame_serial = self.frame_serial.wrapping_add(1);
        let placed = match self.filmcraft.as_mut() {
            Some(host) => host.place_design(&png),
            None => Err("FilmCraft is not available.".to_string()),
        };
        match placed {
            Ok(_) => match layers {
                Ok(layers) => match self.send_layers_to_effects("VectorCraft design", layers) {
                    Ok((count, comp)) => {
                        self.vector_status = format!("Clip on Edit; {count} layers in Effects");
                        self.select_workspace(Workspace::Effects);
                        self.show_effect_comp(comp, count);
                    }
                    Err(error) => {
                        self.vector_status =
                            format!("Artwork placed on the timeline; Effects: {error}");
                        self.select_workspace(Workspace::Edit);
                    }
                },
                Err(error) => {
                    self.vector_status = format!("Artwork placed on the timeline; layers: {error}");
                    self.select_workspace(Workspace::Edit);
                }
            },
            Err(error) => self.vector_status = error,
        }
    }

    /// Create an EffectCraft composition with one layer per rendered element.
    fn send_layers_to_effects(
        &mut self,
        name: &str,
        layers: Vec<crate::layer::LayerPng>,
    ) -> Result<(usize, u64), String> {
        #[cfg(feature = "effectcraft")]
        {
            let width = self.design.doc.canvas_w.round().max(1.0) as u32;
            let height = self.design.doc.canvas_h.round().max(1.0) as u32;
            match self.effects.as_mut() {
                Some(host) => host.animate_layers(name, width, height, &layers),
                None => Err("EffectCraft is not available.".to_string()),
            }
        }
        #[cfg(not(feature = "effectcraft"))]
        {
            let _ = (name, layers);
            Err("EffectCraft support is not compiled in.".to_string())
        }
    }

    /// Open the design's composition after the Effects tab has imported the cut.
    fn show_effect_comp(&mut self, comp: u64, count: usize) {
        #[cfg(feature = "effectcraft")]
        {
            if let Some(host) = self.effects.as_mut() {
                let _ = host.open_comp(comp);
            }
            self.effects_status = if count == 1 {
                "1 design layer ready to animate".to_string()
            } else {
                format!("{count} design layers ready to animate")
            };
        }
        #[cfg(not(feature = "effectcraft"))]
        {
            let _ = (comp, count);
        }
    }

    /// Entering the Pixel persona: hand the design over as a flattened image
    /// when it changed since PhotoCraft last saw it.
    #[cfg(feature = "photocraft")]
    fn enter_pixel(&mut self) {
        if self.pixel_seen != self.design_rev {
            self.pixel_status = "Design opened in PhotoCraft".to_string();
            self.sync_design_to_pixel();
        }
    }

    #[cfg(feature = "photocraft")]
    fn sync_design_to_pixel(&mut self) {
        let path = pixel_dir().join("design.png");
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            self.pixel_status = format!("Could not create {}: {error}", parent.display());
            return;
        }
        if let Err(error) = self.design.export_png_to(&path) {
            self.pixel_status = error;
            return;
        }
        let opened = match self.photocraft.as_mut() {
            Some(host) => host.open(&path),
            None => Err(self
                .photocraft_error
                .clone()
                .unwrap_or_else(|| "PhotoCraft is not available.".to_string())),
        };
        if let Err(error) = opened {
            self.pixel_status = error;
        } else {
            self.pixel_seen = self.design_rev;
        }
    }

    /// Leaving the Pixel persona: put its image back only when it was edited.
    #[cfg(feature = "photocraft")]
    fn leave_pixel(&mut self) {
        let edited = self
            .photocraft
            .as_ref()
            .is_some_and(photocraft_host::PhotocraftHost::has_unsaved_artwork);
        if edited {
            self.return_pixel_artwork(false);
        }
    }

    #[cfg(feature = "photocraft")]
    fn return_pixel_artwork(&mut self, show_layout: bool) {
        let path = pixel_dir().join(format!("pixel-{:03}.png", self.frame_serial));
        let exported = match self.photocraft.as_mut() {
            Some(host) => host.export_png(&path),
            None => Err("PhotoCraft is not available.".to_string()),
        };
        match exported {
            Ok(()) => {
                self.frame_serial = self.frame_serial.wrapping_add(1);
                match self.design.replace_with_artwork(&path) {
                    Ok(_) => {
                        self.design_rev = self.design_rev.wrapping_add(1);
                        self.pixel_seen = self.design_rev;
                        self.design.doc.status =
                            "Updated the design with PhotoCraft's image".into();
                        self.pixel_status = "Image added to the design".to_string();
                        if show_layout {
                            self.persona = Persona::Layout;
                        }
                    }
                    Err(error) => self.pixel_status = error,
                }
            }
            Err(error) => self.pixel_status = error,
        }
    }

    /// Open the shared editable artwork in FigCraft. The strip's **Design to
    /// FigCraft** re-seeds it from the design.
    #[cfg(feature = "figcraft")]
    fn sync_design_to_fig(&mut self) {
        let path = match self.vector_artwork(true) {
            Ok(path) => path,
            Err(error) => {
                self.fig_status = error;
                return;
            }
        };
        let opened = match self.figcraft.as_mut() {
            Some(host) => host.open(&path),
            None => Err(self
                .figcraft_error
                .clone()
                .unwrap_or_else(|| "FigCraft is not available.".to_string())),
        };
        match opened {
            Ok(()) => {
                self.fig_status = "Design opened in FigCraft".to_string();
                self.fig_seen = self.design_rev;
            }
            Err(error) => self.fig_status = error,
        }
    }

    /// Entering the Layout persona: open the shared editable artwork when it
    /// changed since FigCraft last saw it.
    #[cfg(feature = "figcraft")]
    fn enter_fig(&mut self) {
        if self.fig_seen == self.design_rev {
            return;
        }
        let path = match self.vector_artwork(false) {
            Ok(path) => path,
            Err(error) => {
                self.fig_status = error;
                return;
            }
        };
        let opened = match self.figcraft.as_mut() {
            Some(host) => host.open(&path),
            None => Err(self
                .figcraft_error
                .clone()
                .unwrap_or_else(|| "FigCraft is not available.".to_string())),
        };
        match opened {
            Ok(()) => {
                self.fig_status = "Design opened in FigCraft".to_string();
                self.fig_seen = self.design_rev;
            }
            Err(error) => self.fig_status = error,
        }
    }

    /// Leaving Layout: write the editable SVG for VectorCraft and the PNG for
    /// the design, but only when FigCraft was edited.
    #[cfg(feature = "figcraft")]
    fn leave_fig(&mut self) {
        let edited = self
            .figcraft
            .as_ref()
            .is_some_and(figcraft_host::FigcraftHost::has_unsaved_artwork);
        if edited {
            self.return_fig_artwork(false);
        }
    }

    #[cfg(feature = "figcraft")]
    fn return_fig_artwork(&mut self, show_layout: bool) {
        let svg = shared_dir().join("artwork.svg");
        let svg_written = match self.figcraft.as_mut() {
            Some(host) => host.export_svg(&svg),
            None => Err("FigCraft is not available.".to_string()),
        };
        let path = fig_dir().join(format!("page-{:03}.png", self.frame_serial));
        let exported = match self.figcraft.as_mut() {
            Some(host) => host.export_png(&path),
            None => Err("FigCraft is not available.".to_string()),
        };
        match exported {
            Ok(()) => {
                self.frame_serial = self.frame_serial.wrapping_add(1);
                match self.design.replace_with_artwork(&path) {
                    Ok(_) => {
                        self.design_rev = self.design_rev.wrapping_add(1);
                        self.fig_seen = self.design_rev;
                        self.design.doc.status = "Updated the design with FigCraft's page".into();
                        match svg_written {
                            Ok(()) => {
                                self.vector_art_rev = self.design_rev;
                                self.fig_status =
                                    "Page added - still editable in VectorCraft".to_string();
                            }
                            Err(error) => {
                                self.vector_art_rev = 0;
                                self.fig_status = format!("Page added; SVG link failed: {error}");
                            }
                        }
                        if show_layout {
                            self.persona = Persona::Layout;
                        }
                    }
                    Err(error) => self.fig_status = error,
                }
            }
            Err(error) => self.fig_status = error,
        }
    }

    /// The strip above the hosted FigCraft: the same four link actions.
    #[cfg(feature = "figcraft")]
    fn figcraft_strip(&mut self, ui: &mut egui::Ui) {
        let status = self.fig_status.clone();
        let mut sync = false;
        let mut bring = false;
        let mut place = false;
        let mut pull = false;
        let mut to_pixel = false;
        let mut to_vector = false;
        #[cfg(feature = "filmcraft")]
        let timeline_ready = self.filmcraft.is_some();
        #[cfg(not(feature = "filmcraft"))]
        let timeline_ready = false;

        egui::Panel::top("fig_link")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 20, 24))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("FigCraft")
                            .strong()
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("· the real interface designer · linked to the design")
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Layers to Vector").size(11.0))
                            .on_hover_text(
                                "Open FigCraft's page in VectorCraft with its editable layers.",
                            )
                            .clicked()
                        {
                            to_vector = true;
                        }
                        if ui
                            .button(RichText::new("Layers to Pixel").size(11.0))
                            .on_hover_text(
                                "Open FigCraft's page elements in PhotoCraft, one image layer each.",
                            )
                            .clicked()
                        {
                            to_pixel = true;
                        }
                        if ui
                            .button(RichText::new("Page to Photo").size(11.0))
                            .on_hover_text("Render FigCraft's page and put it in this design.")
                            .clicked()
                        {
                            bring = true;
                        }
                        if ui
                            .button(RichText::new("Design to FigCraft").size(11.0))
                            .on_hover_text("Re-send the design as SVG and open it here.")
                            .clicked()
                        {
                            sync = true;
                        }
                        if timeline_ready
                            && ui
                                .button(RichText::new("Place on timeline").size(11.0))
                                .on_hover_text(
                                    "Render the page to a PNG and insert it at the playhead, and build a layered composition in Effects.",
                                )
                                .clicked()
                        {
                            place = true;
                        }
                        if timeline_ready
                            && ui
                                .button(RichText::new("Frame from timeline").size(11.0))
                                .on_hover_text(
                                    "Pull the Edit frame under the playhead into the design.",
                                )
                                .clicked()
                        {
                            pull = true;
                        }
                        ui.add(
                            egui::Label::new(RichText::new(status).size(10.0).color(ACCENT))
                                .truncate(),
                        );
                    });
                });
            });

        if sync {
            self.sync_design_to_fig();
        }
        if bring {
            self.return_fig_artwork(true);
        }
        if place {
            self.place_fig_on_timeline();
        }
        if pull {
            #[cfg(feature = "filmcraft")]
            self.send_edit_frame_to_photo();
            #[cfg(not(feature = "filmcraft"))]
            let _ = pull;
        }
        if to_pixel {
            self.send_fig_to_pixel();
        }
        if to_vector {
            self.send_fig_layers_to_vector();
        }
    }

    /// Open FigCraft's page elements in PhotoCraft, one image layer each.
    #[cfg(all(feature = "figcraft", feature = "photocraft"))]
    fn send_fig_to_pixel(&mut self) {
        let folder = fig_dir().join(format!("to-pixel-{:03}", self.frame_serial));
        self.frame_serial = self.frame_serial.wrapping_add(1);
        let layers = match self.figcraft.as_mut() {
            Some(host) => host.export_layer_pngs(&folder),
            None => Err("FigCraft is not available.".to_string()),
        };
        let layers = match layers {
            Ok(layers) => layers,
            Err(error) => {
                self.fig_status = error;
                return;
            }
        };
        let (width, height) = self.canvas_px();
        let opened = match self.photocraft.as_mut() {
            Some(host) => host.replace_with_layers(width, height, &layers),
            None => Err("PhotoCraft is not available.".to_string()),
        };
        match opened {
            Ok(count) => {
                self.pixel_seen = self.design_rev;
                self.pixel_status = format!("{count} FigCraft layer(s) opened in PhotoCraft");
                self.persona = Persona::Pixel;
            }
            Err(error) => self.fig_status = error,
        }
    }

    #[cfg(not(all(feature = "figcraft", feature = "photocraft")))]
    fn send_fig_to_pixel(&mut self) {
        self.fig_status = "PhotoCraft is not in this build.".to_string();
    }

    /// Write FigCraft's editable SVG and open it in VectorCraft.
    #[cfg(all(feature = "figcraft", feature = "vectorcraft"))]
    fn send_fig_layers_to_vector(&mut self) {
        let svg = shared_dir().join("artwork.svg");
        let written = match self.figcraft.as_mut() {
            Some(host) => host.export_svg(&svg),
            None => Err("FigCraft is not available.".to_string()),
        };
        match written {
            Ok(()) => {
                self.vector_art_rev = self.design_rev;
                let opened = match self.vectorcraft.as_mut() {
                    Some(host) => host.open(&svg),
                    None => Err("VectorCraft is not available.".to_string()),
                };
                match opened {
                    Ok(()) => {
                        self.vector_seen = self.design_rev;
                        self.vector_status = "FigCraft layers opened in VectorCraft".to_string();
                        self.persona = Persona::Vector;
                    }
                    Err(error) => self.fig_status = error,
                }
            }
            Err(error) => self.fig_status = error,
        }
    }

    #[cfg(not(all(feature = "figcraft", feature = "vectorcraft")))]
    fn send_fig_layers_to_vector(&mut self) {
        self.fig_status = "VectorCraft is not in this build.".to_string();
    }

    /// Render FigCraft's page and put it on the FilmCraft timeline.
    #[cfg(all(feature = "figcraft", feature = "filmcraft"))]
    fn place_fig_on_timeline(&mut self) {
        let path = fig_dir().join(format!("timeline-{:03}.png", self.frame_serial));
        let exported = match self.figcraft.as_mut() {
            Some(host) => host.export_png(&path),
            None => Err("FigCraft is not available.".to_string()),
        };
        let layer_folder = fig_dir().join(format!("layers-{:03}", self.frame_serial));
        let layers = match self.figcraft.as_mut() {
            Some(host) => host.export_layer_pngs(&layer_folder),
            None => Err("FigCraft is not available.".to_string()),
        };
        let placed = match exported {
            Ok(()) => {
                self.frame_serial = self.frame_serial.wrapping_add(1);
                match self.filmcraft.as_mut() {
                    Some(host) => host.place_design(&path),
                    None => Err("FilmCraft is not available.".to_string()),
                }
            }
            Err(error) => Err(error),
        };
        match placed {
            Ok(_) => match layers {
                Ok(layers) => match self.send_layers_to_effects("FigCraft page", layers) {
                    Ok((count, comp)) => {
                        self.fig_status = format!("Clip on Edit; {count} layers in Effects");
                        self.select_workspace(Workspace::Effects);
                        self.show_effect_comp(comp, count);
                    }
                    Err(error) => {
                        self.fig_status = format!("Page placed on the timeline; Effects: {error}");
                        self.select_workspace(Workspace::Edit);
                    }
                },
                Err(error) => {
                    self.fig_status = format!("Page placed on the timeline; layers: {error}");
                    self.select_workspace(Workspace::Edit);
                }
            },
            Err(error) => self.fig_status = error,
        }
    }

    /// The strip above the hosted PhotoCraft: the same four link actions.
    #[cfg(feature = "photocraft")]
    fn photocraft_strip(&mut self, ui: &mut egui::Ui) {
        let status = self.pixel_status.clone();
        let mut sync = false;
        let mut bring = false;
        let mut place = false;
        let mut pull = false;
        let mut to_vector = false;
        let mut to_layout = false;
        #[cfg(feature = "filmcraft")]
        let timeline_ready = self.filmcraft.is_some();
        #[cfg(not(feature = "filmcraft"))]
        let timeline_ready = false;

        egui::Panel::top("pixel_link")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 20, 24))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("PhotoCraft")
                            .strong()
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("· the real pixel editor · linked to the design")
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Layers to Vector").size(11.0))
                            .on_hover_text(
                                "Open PhotoCraft's layers in VectorCraft, one image layer each.",
                            )
                            .clicked()
                        {
                            to_vector = true;
                        }
                        if ui
                            .button(RichText::new("Layers to Layout").size(11.0))
                            .on_hover_text(
                                "Open PhotoCraft's layers in FigCraft, one image layer each.",
                            )
                            .clicked()
                        {
                            to_layout = true;
                        }
                        if ui
                            .button(RichText::new("Image to Photo").size(11.0))
                            .on_hover_text(
                                "Render PhotoCraft's image to a PNG and put it in this design.",
                            )
                            .clicked()
                        {
                            bring = true;
                        }
                        if ui
                            .button(RichText::new("Design to PhotoCraft").size(11.0))
                            .on_hover_text("Re-send the design as an image and open it here.")
                            .clicked()
                        {
                            sync = true;
                        }
                        if timeline_ready
                            && ui
                                .button(RichText::new("Place on timeline").size(11.0))
                                .on_hover_text(
                                    "Save the image as a PNG and insert it at the playhead.",
                                )
                                .clicked()
                        {
                            place = true;
                        }
                        if timeline_ready
                            && ui
                                .button(RichText::new("Frame from timeline").size(11.0))
                                .on_hover_text(
                                    "Pull the Edit frame under the playhead into the design.",
                                )
                                .clicked()
                        {
                            pull = true;
                        }
                        ui.add(
                            egui::Label::new(RichText::new(status).size(10.0).color(ACCENT))
                                .truncate(),
                        );
                    });
                });
            });

        if sync {
            self.sync_design_to_pixel();
        }
        if bring {
            self.return_pixel_artwork(true);
        }
        if place {
            self.place_pixel_on_timeline();
        }
        if pull {
            #[cfg(feature = "filmcraft")]
            self.send_edit_frame_to_photo();
            #[cfg(not(feature = "filmcraft"))]
            let _ = pull;
        }
        if to_vector {
            self.send_pixels_to_vector();
        }
        if to_layout {
            self.send_pixels_to_layout();
        }
    }

    /// The shared canvas size in whole pixels.
    fn canvas_px(&self) -> (u32, u32) {
        (
            self.design.doc.canvas_w.round().max(1.0) as u32,
            self.design.doc.canvas_h.round().max(1.0) as u32,
        )
    }

    /// Open PhotoCraft's layers in VectorCraft, one image layer each.
    #[cfg(all(feature = "photocraft", feature = "vectorcraft"))]
    fn send_pixels_to_vector(&mut self) {
        let folder = pixel_dir().join(format!("to-vector-{:03}", self.frame_serial));
        self.frame_serial = self.frame_serial.wrapping_add(1);
        let layers = match self.photocraft.as_mut() {
            Some(host) => host.export_layer_pngs(&folder),
            None => Err("PhotoCraft is not available.".to_string()),
        };
        let layers = match layers {
            Ok(layers) => layers,
            Err(error) => {
                self.pixel_status = error;
                return;
            }
        };
        let (width, height) = self.canvas_px();
        let opened = match self.vectorcraft.as_mut() {
            Some(host) => host.replace_with_layers(width, height, &layers),
            None => Err("VectorCraft is not available.".to_string()),
        };
        match opened {
            Ok(count) => {
                self.vector_seen = self.design_rev;
                self.vector_status = format!("{count} PhotoCraft layer(s) opened in VectorCraft");
                self.persona = Persona::Vector;
            }
            Err(error) => self.pixel_status = error,
        }
    }

    /// Open PhotoCraft's layers in FigCraft, one image layer each.
    #[cfg(all(feature = "photocraft", feature = "figcraft"))]
    fn send_pixels_to_layout(&mut self) {
        let folder = pixel_dir().join(format!("to-fig-{:03}", self.frame_serial));
        self.frame_serial = self.frame_serial.wrapping_add(1);
        let layers = match self.photocraft.as_mut() {
            Some(host) => host.export_layer_pngs(&folder),
            None => Err("PhotoCraft is not available.".to_string()),
        };
        let layers = match layers {
            Ok(layers) => layers,
            Err(error) => {
                self.pixel_status = error;
                return;
            }
        };
        let (width, height) = self.canvas_px();
        let opened = match self.figcraft.as_mut() {
            Some(host) => host.replace_with_layers(width, height, &layers),
            None => Err("FigCraft is not available.".to_string()),
        };
        match opened {
            Ok(count) => {
                self.fig_seen = self.design_rev;
                self.fig_status = format!("{count} PhotoCraft layer(s) opened in FigCraft");
                self.persona = Persona::Layout;
            }
            Err(error) => self.pixel_status = error,
        }
    }

    #[cfg(not(all(feature = "photocraft", feature = "vectorcraft")))]
    fn send_pixels_to_vector(&mut self) {
        self.pixel_status = "VectorCraft is not in this build.".to_string();
    }

    #[cfg(not(all(feature = "photocraft", feature = "figcraft")))]
    fn send_pixels_to_layout(&mut self) {
        self.pixel_status = "FigCraft is not in this build.".to_string();
    }

    /// Save PhotoCraft's image and put it on the FilmCraft timeline.
    #[cfg(all(feature = "photocraft", feature = "filmcraft"))]
    fn place_pixel_on_timeline(&mut self) {
        let path = pixel_dir().join(format!("timeline-{:03}.png", self.frame_serial));
        let exported = match self.photocraft.as_mut() {
            Some(host) => host.export_png(&path),
            None => Err("PhotoCraft is not available.".to_string()),
        };
        let layer_folder = pixel_dir().join(format!("layers-{:03}", self.frame_serial));
        let layers = match self.photocraft.as_mut() {
            Some(host) => host.export_layer_pngs(&layer_folder),
            None => Err("PhotoCraft is not available.".to_string()),
        };
        let placed = match exported {
            Ok(()) => {
                self.frame_serial = self.frame_serial.wrapping_add(1);
                match self.filmcraft.as_mut() {
                    Some(host) => host.place_design(&path),
                    None => Err("FilmCraft is not available.".to_string()),
                }
            }
            Err(error) => Err(error),
        };
        match placed {
            Ok(_) => match layers {
                Ok(layers) => match self.send_layers_to_effects("PhotoCraft image", layers) {
                    Ok((count, comp)) => {
                        self.pixel_status = format!("Clip on Edit; {count} layers in Effects");
                        self.select_workspace(Workspace::Effects);
                        self.show_effect_comp(comp, count);
                    }
                    Err(error) => {
                        self.pixel_status =
                            format!("Image placed on the timeline; Effects: {error}");
                        self.select_workspace(Workspace::Edit);
                    }
                },
                Err(error) => {
                    self.pixel_status = format!("Image placed on the timeline; layers: {error}");
                    self.select_workspace(Workspace::Edit);
                }
            },
            Err(error) => self.pixel_status = error,
        }
    }

    /// The strip above the hosted VectorCraft: the four link actions.
    #[cfg(feature = "vectorcraft")]
    fn vectorcraft_strip(&mut self, ui: &mut egui::Ui) {
        // Entering the persona already handed the design over ([`Self::enter_vector`]),
        // and leaving it writes artwork back, so this strip never reloads by
        // itself and cannot throw away what is open.

        let status = self.vector_status.clone();
        let mut sync = false;
        let mut bring = false;
        let mut place = false;
        let mut pull = false;
        let mut to_pixel = false;
        let mut to_layout = false;
        #[cfg(feature = "filmcraft")]
        let timeline_ready = self.filmcraft.is_some();
        #[cfg(not(feature = "filmcraft"))]
        let timeline_ready = false;

        egui::Panel::top("vector_link")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 20, 24))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("VectorCraft")
                            .strong()
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("· the real illustrator · linked to the design by SVG")
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Layers to Layout").size(11.0))
                            .on_hover_text(
                                "Open VectorCraft's artwork in FigCraft with its editable layers.",
                            )
                            .clicked()
                        {
                            to_layout = true;
                        }
                        if ui
                            .button(RichText::new("Layers to Pixel").size(11.0))
                            .on_hover_text(
                                "Open VectorCraft's layers in PhotoCraft, one image layer each.",
                            )
                            .clicked()
                        {
                            to_pixel = true;
                        }
                        if ui
                            .button(RichText::new("Artwork to Photo").size(11.0))
                            .on_hover_text(
                                "Render VectorCraft's artwork to a PNG and add it as a photo \
                                 layer in this design.",
                            )
                            .clicked()
                        {
                            bring = true;
                        }
                        if ui
                            .button(RichText::new("Design to VectorCraft").size(11.0))
                            .on_hover_text(
                                "Re-export the design as SVG and open it here. The artwork \
                                 currently open is replaced.",
                            )
                            .clicked()
                        {
                            sync = true;
                        }
                        if timeline_ready
                            && ui
                                .button(RichText::new("Place on timeline").size(11.0))
                                .on_hover_text(
                                    "Render VectorCraft's artboard to a PNG, insert it at the playhead, \
                                     and build a layered composition in Effects.",
                                )
                                .clicked()
                        {
                            place = true;
                        }
                        if timeline_ready
                            && ui
                                .button(RichText::new("Frame from timeline").size(11.0))
                                .on_hover_text(
                                    "Pull the Edit frame under the playhead into this design \
                                     (it then reloads here automatically).",
                                )
                                .clicked()
                        {
                            pull = true;
                        }
                        ui.add(
                            egui::Label::new(RichText::new(status).size(10.0).color(ACCENT))
                                .truncate(),
                        );
                    });
                });
            });

        if sync {
            self.sync_design_to_vector();
        }
        if bring {
            self.send_vector_to_photo();
        }
        if place {
            #[cfg(feature = "filmcraft")]
            self.place_vector_on_timeline();
        }
        if pull {
            #[cfg(feature = "filmcraft")]
            self.send_edit_frame_to_photo();
            #[cfg(not(feature = "filmcraft"))]
            let _ = pull;
        }
        if to_pixel {
            self.send_vector_to_pixel();
        }
        if to_layout {
            self.send_vector_layers_to_fig();
        }
    }

    /// EffectCraft is a whole application, and one of its first passes maximizes
    /// a window it believes doesn't fit the screen. In the shell that fights the
    /// user's window, so undo that one automatic maximize for a few seconds
    /// after the tab opens. A maximize the user asks for comes with input and is
    /// left alone.
    #[cfg(feature = "effectcraft")]
    fn keep_our_window_size(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let (until, was_maximized) = *self
            .effects_window_guard
            .get_or_insert_with(|| (now + 15.0, maximized));
        if now > until {
            self.effects_window_guard = None;
            return;
        }
        // EffectCraft's compositor renders the first frame a few passes after
        // the tab opens. Without a repaint request the viewer could sit on its
        // empty black placeholder until the next mouse move.
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
        if was_maximized || !maximized {
            return;
        }
        let user_input = ctx.input(|i| !i.events.is_empty());
        if !user_input {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        }
    }

    /// The strip above EffectCraft: what is linked and when to re-sync.
    #[cfg(feature = "effectcraft")]
    fn effects_link_strip(&mut self, ui: &mut egui::Ui) {
        let composition = self
            .effects
            .as_ref()
            .and_then(effectcraft_host::EffectcraftHost::open_composition);
        let status = self.effects_status.clone();
        let detail = self.effects_detail.clone();
        let mut sync = false;
        let mut to_photo = false;
        let mut render = false;
        let mut model3d = false;

        egui::Panel::top("effects_link")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 20, 24))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("EffectCraft")
                            .strong()
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("· linked to the Edit cut by Final Cut Pro XML")
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                    if let Some(composition) = &composition {
                        ui.label(RichText::new("·").size(11.0).color(TEXT_DIM));
                        ui.label(RichText::new(composition).size(11.0).color(ACCENT));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Render to Edit").size(11.0))
                            .on_hover_text(
                                "Render this composition with its animation, import it into \n\
                                 FilmCraft and place the clip at the playhead. Rendering may \n\
                                 take a minute.",
                            )
                            .clicked()
                        {
                            render = true;
                        }
                        if ui
                            .button(RichText::new("3D model").size(11.0))
                            .on_hover_text(
                                "Import a 3D model (.gltf, .glb, .obj, .stl, .fbx) as a layer \n\
                                 in this composition, through EffectCraft's own Advanced 3D.",
                            )
                            .clicked()
                        {
                            model3d = true;
                        }
                        if ui
                            .button(RichText::new("Sync cut").size(11.0))
                            .on_hover_text(
                                "Import the current Edit sequence as a new composition.\n\
                                 Work on earlier compositions is kept.",
                            )
                            .clicked()
                        {
                            sync = true;
                        }
                        if ui
                            .button(RichText::new("Frame to Photo").size(11.0))
                            .on_hover_text(
                                "Render the frame under EffectCraft's playhead and add it \
                                 as a photo layer in the Photo tab.",
                            )
                            .clicked()
                        {
                            to_photo = true;
                        }
                        // The full import report goes in the tooltip: the strip is
                        // one line and must not run over the labels on the left.
                        ui.add(
                            egui::Label::new(RichText::new(status).size(10.0).color(TEXT_DIM))
                                .truncate(),
                        )
                        .on_hover_text(detail);
                    });
                });
            });

        if sync {
            self.sync_effects_from_edit(true);
            ui.ctx().request_repaint();
        }
        if to_photo {
            self.send_effects_frame_to_photo();
        }
        if render {
            self.render_effects_to_edit();
        }
        if model3d {
            self.import_3d_model();
        }
    }

    /// The picture the 3D tab shows: `3d.png` next to the app, in CreativeCraft's
    /// data folder, or in the projects root. Loaded once and cached until it changes.
    fn three_d_picture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
        {
            paths.push(dir.join("3d.png"));
            if let Some(parent) = dir.parent().and_then(|p| p.parent()) {
                paths.push(parent.join("3d.png"));
            }
        }
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            paths.push(
                std::path::PathBuf::from(base)
                    .join("CreativeCraft")
                    .join("3d.png"),
            );
        }
        let Some(path) = paths.into_iter().find(|path| path.is_file()) else {
            return Self::bundled_face(ctx);
        };
        let stamp = std::fs::metadata(&path).ok()?.modified().ok()?;
        let key = egui::Id::new("creativecraft-3d-picture");
        if let Some((cached, cached_stamp, texture)) = ctx.data(|data| {
            data.get_temp::<(
                std::path::PathBuf,
                std::time::SystemTime,
                egui::TextureHandle,
            )>(key)
        }) && cached == path
            && cached_stamp == stamp
        {
            return Some(texture);
        }
        let bytes = std::fs::read(&path).ok()?;
        let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        let texture = ctx.load_texture(
            "creativecraft-3d-picture",
            color,
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|data| data.insert_temp(key, (path, stamp, texture.clone())));
        Some(texture)
    }

    /// The 3D tab's face: the crying emoji drawn in code, so the tab has a picture
    /// with no asset, no emoji font and no download.
    fn paint_crying_face(painter: &egui::Painter, rect: egui::Rect) {
        let c = rect.center();
        let s = rect.width().min(rect.height());
        let at = |x: f32, y: f32| egui::pos2(c.x + (x - 0.5) * s, c.y + (y - 0.5) * s);
        let yellow = egui::Color32::from_rgb(255, 198, 30);
        let shade = egui::Color32::from_rgb(236, 172, 18);
        let ink = egui::Color32::from_rgb(48, 34, 12);
        let tear = egui::Color32::from_rgb(64, 190, 250);
        let blush = egui::Color32::from_rgba_unmultiplied(255, 150, 60, 90);
        painter.circle_filled(c, s * 0.47, yellow);
        painter.circle_stroke(c, s * 0.47, egui::Stroke::new(s * 0.012, shade));
        for side in [-1.0f32, 1.0] {
            let eye = at(0.5 + side * 0.165, 0.40);
            painter.circle_filled(eye, s * 0.135, egui::Color32::WHITE);
            let h = s * 0.115;
            painter.circle_filled(
                egui::pos2(eye.x - h * 0.28, eye.y - h * 0.20),
                h * 0.34,
                ink,
            );
            painter.circle_filled(
                egui::pos2(eye.x + h * 0.28, eye.y - h * 0.20),
                h * 0.34,
                ink,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(eye.x - h * 0.60, eye.y - h * 0.04),
                    egui::pos2(eye.x + h * 0.60, eye.y - h * 0.04),
                    egui::pos2(eye.x, eye.y + h * 0.66),
                ],
                ink,
                egui::Stroke::NONE,
            ));
            painter.add(egui::Shape::ellipse_filled(
                at(0.5 + side * 0.165, 0.545),
                egui::vec2(s * 0.075, s * 0.055),
                tear,
            ));
            painter.circle_filled(at(0.5 + side * 0.315, 0.53), s * 0.070, blush);
            let hand = egui::Rect::from_center_size(
                at(0.5 + side * 0.335, 0.745),
                egui::vec2(s * 0.225, s * 0.27),
            );
            painter.rect_filled(hand, s * 0.06, yellow);
            for i in 0..3 {
                let y = hand.top() + hand.height() * (0.32 + 0.19 * i as f32);
                painter.line_segment(
                    [
                        egui::pos2(hand.left() + s * 0.028, y),
                        egui::pos2(hand.right() - s * 0.028, y),
                    ],
                    egui::Stroke::new(s * 0.006, shade),
                );
            }
        }
        painter.add(egui::Shape::ellipse_filled(
            at(0.5, 0.625),
            egui::vec2(s * 0.095, s * 0.060),
            ink,
        ));
    }

    /// The picture shipped with the app, shown when no override file is present.
    fn bundled_face(ctx: &egui::Context) -> Option<egui::TextureHandle> {
        /// The crying-face picture, bundled so the tab needs no file and no font.
        const FACE_PNG: &[u8] = include_bytes!("../assets/3d-face.png");
        let key = egui::Id::new("creativecraft-3d-bundled");
        if let Some(texture) = ctx.data(|data| data.get_temp::<egui::TextureHandle>(key)) {
            return Some(texture);
        }
        let image = image::load_from_memory(FACE_PNG).ok()?.to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        let texture = ctx.load_texture(
            "creativecraft-3d-bundled",
            color,
            egui::TextureOptions::LINEAR,
        );
        ctx.data_mut(|data| data.insert_temp(key, texture.clone()));
        Some(texture)
    }

    /// The 3D tab: a placeholder until a real 3D app can be hosted here. It shows
    /// the picture saved at `3d.png` next to the app (or in CreativeCraft's data
    /// folder), so you choose the image rather than an emoji font.
    fn three_d_tab(&mut self, ui: &mut egui::Ui) {
        let texture = Self::three_d_picture(ui.ctx());
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add_space(48.0);
                ui.vertical_centered(|ui| {
                    match &texture {
                        Some(texture) => {
                            ui.add(
                                egui::Image::new((texture.id(), egui::vec2(300.0, 300.0)))
                                    .fit_to_exact_size(egui::vec2(300.0, 300.0)),
                            );
                        }
                        None => {
                            // Drawn here so the tab is never empty: the same face
                            // as the picture, scaled with the space it is given.
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(300.0, 300.0),
                                egui::Sense::hover(),
                            );
                            Self::paint_crying_face(ui.painter(), rect);
                        }
                    }
                    ui.add_space(14.0);
                    ui.label(RichText::new("3D").strong().size(26.0).color(TEXT_BRIGHT));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Coming soon").size(14.0).color(TEXT_DIM));
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("I don't have more AI credits to build this — maybe soon.")
                            .size(12.0)
                            .color(ACCENT),
                    );
                });
            });
    }

    /// Pick a 3D model and add it to the open EffectCraft composition.
    #[cfg(feature = "effectcraft")]
    fn import_3d_model(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("3D model", &["gltf", "glb", "obj", "stl", "fbx"])
            .set_title("Import a 3D model into EffectCraft")
            .pick_file()
        else {
            return;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        match self.effects.as_mut() {
            Some(host) => match host.add_3d_model(&path) {
                Ok(()) => self.effects_status = format!("{name} added to the composition"),
                Err(error) => self.effects_status = error,
            },
            None => self.effects_status = "EffectCraft is not available.".to_string(),
        }
    }

    /// Render the open EffectCraft composition and place it on the Edit timeline.
    #[cfg(all(feature = "effectcraft", feature = "filmcraft"))]
    fn render_effects_to_edit(&mut self) {
        let path = render_dir().join(format!("effects-{:03}.mp4", self.frame_serial));
        self.frame_serial = self.frame_serial.wrapping_add(1);
        self.effects_status = "Rendering the composition...".to_string();
        let rendered = match self.effects.as_mut() {
            Some(host) => host.render_active_comp(&path),
            None => Err("EffectCraft is not available.".to_string()),
        };
        if let Err(error) = rendered {
            self.effects_status = error;
            return;
        }
        let placed = match self.filmcraft.as_mut() {
            Some(host) => host.place_render(&path),
            None => Err("FilmCraft is not available.".to_string()),
        };
        match placed {
            Ok(_) => {
                self.effects_status = "Rendered animation placed on Edit".to_string();
                self.select_workspace(Workspace::Edit);
            }
            Err(error) => self.effects_status = error,
        }
    }

    /// Rendering to Edit needs both apps; this keeps the button harmless in a
    /// build that has only one of them.
    #[cfg(not(all(feature = "effectcraft", feature = "filmcraft")))]
    fn render_effects_to_edit(&mut self) {
        self.effects_status = "Rendering to Edit needs both EffectCraft and FilmCraft.".to_string();
    }

    /// The Edit cut's audio as a timeline under the hosted SoundCraft: the same
    /// clips, at the same times, so you can see what is going into the DAW and
    /// drag a clip up into it.
    #[cfg(feature = "soundcraft")]
    fn audio_timeline_strip(&mut self, ui: &mut egui::Ui) {
        let status = self.soundcraft_status.clone();
        let mut cut = false;
        let mut bounce = false;
        let mut clip = false;
        let mut mixer = false;
        let mut open: Option<(String, std::path::PathBuf, f64, f64)> = None;

        let view = {
            #[cfg(feature = "filmcraft")]
            {
                self.filmcraft
                    .as_ref()
                    .map(|host| host.audio_timeline_view())
            }
            #[cfg(not(feature = "filmcraft"))]
            {
                None
            }
        };

        // One row per audio track, so a music bed spanning the whole cut does
        // not hide the clips under it. The strip is only as tall as it needs to
        // be, and the last row is reused past four tracks.
        let lanes = view
            .as_ref()
            .map(|view| {
                view.clips
                    .iter()
                    .map(|clip| clip.lane + 1)
                    .max()
                    .unwrap_or(1)
                    .clamp(1, 4)
            })
            .unwrap_or(1);
        let timeline_height = 8.0 + 17.0 * lanes as f32;

        egui::Panel::bottom("audio_edit_timeline")
            .exact_size(24.0 + timeline_height + 12.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(22, 22, 27))
                    .inner_margin(egui::Margin::symmetric(12, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Edit audio")
                            .strong()
                            .size(11.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("drag a clip up into SoundCraft, or click it")
                            .size(10.0)
                            .color(TEXT_DIM),
                    );
                    if !status.is_empty() {
                        ui.label(RichText::new(status).size(10.0).color(ACCENT));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button(RichText::new("⋯").size(12.0), |ui| {
                            let mixer_label = if self.audio_daw {
                                "FilmCraft mixer"
                            } else {
                                "Back to SoundCraft"
                            };
                            if ui.button(mixer_label).clicked() {
                                mixer = true;
                                ui.close();
                            }
                            if ui
                                .button("Selected clip from Edit")
                                .on_hover_text(
                                    "Same as clicking a clip below: the clip selected in the \
                                     Edit timeline opens in SoundCraft.",
                                )
                                .clicked()
                            {
                                clip = true;
                                ui.close();
                            }
                        });
                        if ui
                            .button(RichText::new("Mix to Edit").size(11.0))
                            .on_hover_text(
                                "Bounce this SoundCraft session to a WAV and place it at the \
                                 Edit playhead on a new audio track.",
                            )
                            .clicked()
                        {
                            bounce = true;
                        }
                        if ui
                            .button(RichText::new("Cut audio").size(11.0))
                            .on_hover_text(
                                "Render the whole Edit sequence's audio and open it in \
                                 SoundCraft as one track, ready to mix.",
                            )
                            .clicked()
                        {
                            cut = true;
                        }
                        ui.label(RichText::new("SoundCraft").size(10.0).color(TEXT_DIM));
                    });
                });

                let height = timeline_height;
                let (response, painter) = ui.allocate_painter(
                    egui::vec2(ui.available_width(), height),
                    egui::Sense::hover(),
                );
                let rect = response.rect;
                painter.rect_filled(rect, 4.0, Color32::from_rgb(13, 13, 17));

                let Some(view) = &view else {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No Edit sequence open",
                        egui::FontId::proportional(10.0),
                        TEXT_DIM,
                    );
                    return;
                };

                let inner = rect.shrink2(egui::vec2(6.0, 4.0));
                let span = view.duration.max(0.001);
                let x_of = |seconds: f64| {
                    inner.left() + (seconds / span).clamp(0.0, 1.0) as f32 * inner.width()
                };
                // Rows come from the lane count computed above, so the strip's
                // height and the drawing always agree.
                let lane_height = inner.height() / lanes as f32;
                let lane_rect = |lane: usize| {
                    let top = inner.top() + lane.min(lanes - 1) as f32 * lane_height;
                    egui::Rect::from_min_max(
                        egui::pos2(inner.left(), top),
                        egui::pos2(inner.right(), top + lane_height - 1.0),
                    )
                };

                for clip in &view.clips {
                    if clip.end <= 0.0 || clip.start >= view.duration {
                        continue;
                    }
                    let x0 = x_of(clip.start.max(0.0));
                    let x1 = x_of(clip.end.min(view.duration)).max(x0 + 2.0);
                    let row = lane_rect(clip.lane);
                    let clip_rect = egui::Rect::from_min_max(
                        egui::pos2(x0, row.top()),
                        egui::pos2(x1, row.bottom()),
                    );
                    // A generator has no file: it is drawn dimmed and cannot be dragged.
                    let has_file = clip.path.is_some();
                    painter.rect_filled(
                        clip_rect,
                        2.0,
                        if has_file {
                            Color32::from_rgb(42, 160, 128)
                        } else {
                            Color32::from_rgb(58, 70, 78)
                        },
                    );
                    painter.with_clip_rect(clip_rect.shrink(1.0)).text(
                        clip_rect.left_top() + egui::vec2(4.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        &clip.name,
                        egui::FontId::proportional(9.0),
                        if has_file {
                            Color32::from_rgb(10, 30, 26)
                        } else {
                            Color32::from_rgb(196, 206, 212)
                        },
                    );

                    let response = ui.interact(
                        clip_rect,
                        ui.scope_id().with(("edit-audio-clip", clip.id)),
                        if has_file {
                            egui::Sense::click_and_drag()
                        } else {
                            egui::Sense::click()
                        },
                    );
                    if response.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    if response.clicked() {
                        // Keep Edit's selection in step with the click, so the
                        // clip you chose here is the one Edit shows selected.
                        #[cfg(feature = "filmcraft")]
                        if let Some(host) = self.filmcraft.as_mut() {
                            host.select_clip(clip.id);
                        }
                        if let Some(path) = clip.path.clone() {
                            open = Some((clip.name.clone(), path, clip.start, clip.source_in));
                        } else {
                            self.soundcraft_status =
                                format!("{} is a generator with no audio file.", clip.name);
                        }
                    }
                    if response.drag_started()
                        && let Some(path) = clip.path.clone()
                    {
                        response.dnd_set_drag_payload(AudioDrag {
                            name: clip.name.clone(),
                            path,
                            start: clip.start,
                            source_in: clip.source_in,
                        });
                    }
                    let hint = if has_file {
                        format!(
                            "{} · {:.2}s–{:.2}s · drag up into SoundCraft, or click",
                            clip.name, clip.start, clip.end
                        )
                    } else {
                        format!(
                            "{} · {:.2}s–{:.2}s · generator, no file to send",
                            clip.name, clip.start, clip.end
                        )
                    };
                    response.on_hover_text(hint);
                }

                let playhead = x_of(view.playhead);
                painter.line_segment(
                    [
                        egui::pos2(playhead, rect.top() + 2.0),
                        egui::pos2(playhead, rect.bottom() - 2.0),
                    ],
                    egui::Stroke::new(1.5, TEXT_BRIGHT),
                );
            });

        if let Some((name, path, start, source_in)) = open {
            self.send_clip_to_soundcraft(&name, &path, start, source_in);
        }
        if clip {
            self.import_timeline_audio();
        }
        if cut {
            self.import_cut_audio();
        }
        if bounce {
            self.bounce_audio_to_edit();
        }
        if mixer {
            self.audio_daw = !self.audio_daw;
            // SoundCraft's transport stops when its view is hidden, so the
            // FilmCraft mixer is never playing underneath it.
            if !self.audio_daw
                && let Some(host) = self.soundcraft.as_mut()
            {
                host.stop();
            }
        }
    }

    /// Open one Edit audio clip in SoundCraft, at the time it sits in the cut.
    /// A trimmed clip is offset back by however much the cut skipped, so the
    /// frame it starts on lands at the same time on both timelines.
    #[cfg(feature = "soundcraft")]
    fn send_clip_to_soundcraft(
        &mut self,
        name: &str,
        path: &std::path::Path,
        start: f64,
        source_in: f64,
    ) {
        self.ensure_soundcraft();
        let Some(host) = self.soundcraft.as_mut() else {
            self.soundcraft_status = "SoundCraft did not start.".to_string();
            return;
        };
        let at = (start - source_in).max(0.0);
        let imported = host.import_audio(path, at).map(|()| host.track_count());
        match imported {
            Ok(tracks) => {
                self.soundcraft_status =
                    format!("{name} from Edit at {start:.2}s - SoundCraft has {tracks} track(s)");
            }
            Err(error) => self.soundcraft_status = error,
        }
    }

    /// Send the audio clip selected in Edit to SoundCraft as its own track.
    #[cfg(feature = "soundcraft")]
    fn import_timeline_audio(&mut self) {
        #[cfg(feature = "filmcraft")]
        {
            let clip = match self.filmcraft.as_ref() {
                Some(host) => host.selected_audio_clip(),
                None => Err("FilmCraft is not available.".to_string()),
            };
            match clip {
                Ok(clip) => {
                    self.send_clip_to_soundcraft(
                        &clip.name,
                        &clip.path,
                        clip.start,
                        clip.source_in,
                    );
                }
                Err(error) => self.soundcraft_status = error,
            }
        }
        #[cfg(not(feature = "filmcraft"))]
        {
            self.soundcraft_status = "FilmCraft is not in this build.".to_string();
        }
    }

    /// Render the whole cut's audio and open it in SoundCraft as one track.
    #[cfg(feature = "soundcraft")]
    fn import_cut_audio(&mut self) {
        #[cfg(feature = "filmcraft")]
        {
            let path = audio_dir().join(format!("cut-audio-{:03}.wav", self.frame_serial));
            self.frame_serial = self.frame_serial.wrapping_add(1);
            self.soundcraft_status = "Rendering the cut's audio...".to_string();
            let exported = match self.filmcraft.as_mut() {
                Some(host) => host.export_audio_mix(&path),
                None => Err("FilmCraft is not available.".to_string()),
            };
            if let Err(error) = exported {
                self.soundcraft_status = error;
                return;
            }
            self.ensure_soundcraft();
            let Some(host) = self.soundcraft.as_mut() else {
                self.soundcraft_status = "SoundCraft did not start.".to_string();
                return;
            };
            match host.import_audio(&path, 0.0) {
                Ok(()) => {
                    self.soundcraft_status =
                        "The cut's audio is in SoundCraft as one track".to_string();
                }
                Err(error) => self.soundcraft_status = error,
            }
        }
        #[cfg(not(feature = "filmcraft"))]
        {
            self.soundcraft_status = "FilmCraft is not in this build.".to_string();
        }
    }

    /// Bounce the SoundCraft session and place it at the Edit playhead.
    #[cfg(feature = "soundcraft")]
    fn bounce_audio_to_edit(&mut self) {
        #[cfg(feature = "filmcraft")]
        {
            let playhead = self
                .filmcraft
                .as_ref()
                .and_then(|host| host.timeline_view())
                .map(|view| view.playhead)
                .unwrap_or(0.0);
            let path = audio_dir().join(format!("soundcraft-mix-{:03}.wav", self.frame_serial));
            self.frame_serial = self.frame_serial.wrapping_add(1);
            self.ensure_soundcraft();
            let Some(host) = self.soundcraft.as_mut() else {
                self.soundcraft_status = "SoundCraft did not start.".to_string();
                return;
            };
            self.soundcraft_status = "Bouncing the SoundCraft mix...".to_string();
            if let Err(error) = host.bounce(&path) {
                self.soundcraft_status = error;
                return;
            }
            let placed = match self.filmcraft.as_mut() {
                Some(film) => film.place_audio_at(&path, playhead).map(|_| ()),
                None => Err("FilmCraft is not available.".to_string()),
            };
            match placed {
                Ok(()) => {
                    self.soundcraft_status = format!("Mix placed in Edit at {playhead:.2}s");
                    self.select_workspace(Workspace::Edit);
                }
                Err(error) => self.soundcraft_status = error,
            }
        }
        #[cfg(not(feature = "filmcraft"))]
        {
            self.soundcraft_status = "FilmCraft is not in this build.".to_string();
        }
    }

    /// The strip above FilmCraft in Edit: hand the current frame to Photo, the
    /// reverse of Photo's "Place on timeline".
    #[cfg(feature = "filmcraft")]
    fn edit_frame_strip(&mut self, ui: &mut egui::Ui) {
        let playhead = self
            .filmcraft
            .as_ref()
            .and_then(|host| host.timeline_view())
            .map(|view| timeline_time(view.playhead))
            .unwrap_or_default();
        let mut send = false;
        let mut audio_out = false;
        let mut audio_in = false;
        let mut video_out = false;
        let status = self.frame_status.clone();

        egui::Panel::top("edit_frame_link")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 20, 24))
                    .inner_margin(egui::Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("FilmCraft")
                            .strong()
                            .size(12.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("· Edit · the frame under the playhead can go to Photo")
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                    if !status.is_empty() {
                        ui.label(RichText::new(status).size(10.0).color(ACCENT));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("Cut to DAW").size(11.0))
                            .on_hover_text(
                                "Render the cut at half size with its audio to an MP4 for \
                                 a DAW's video track (SoundCraft, Reaper...). Picture and \
                                 sound stay in sync. Rendering can take a minute.",
                            )
                            .clicked()
                        {
                            video_out = true;
                        }
                        if ui
                            .button(RichText::new("Mix from DAW").size(11.0))
                            .on_hover_text(
                                "Import a WAV/AIFF/FLAC bounce from your DAW (SoundCraft, Lemon Studio, Reaper...), put it at the \
                                 sequence start on a new audio track, and mute the tracks it \
                                 replaces.",
                            )
                            .clicked()
                        {
                            audio_in = true;
                        }
                        if ui
                            .button(RichText::new("Audio to DAW").size(11.0))
                            .on_hover_text(
                                "Render this sequence's audio to a WAV mix for your DAW (SoundCraft, Lemon Studio, Reaper...).",
                            )
                            .clicked()
                        {
                            audio_out = true;
                        }
                        if ui
                            .button(RichText::new("Send frame to Photo").size(11.0))
                            .on_hover_text(
                                "Export the Program frame at the playhead at full size and \
                                 add it as a photo layer in the Photo tab.",
                            )
                            .clicked()
                        {
                            send = true;
                        }
                        ui.label(
                            RichText::new(format!("at {playhead}"))
                                .size(10.0)
                                .color(TEXT_DIM),
                        );
                    });
                });
            });

        if send {
            self.send_edit_frame_to_photo();
        }
        if audio_out {
            self.send_audio_to_soundcraft();
        }
        if audio_in {
            self.bring_soundcraft_mix();
        }
        if video_out {
            self.send_cut_to_soundcraft();
        }
    }

    /// Render the cut as a half-size MP4 with audio for SoundCraft's picture track.
    #[cfg(feature = "filmcraft")]
    fn send_cut_to_soundcraft(&mut self) {
        let path = audio_dir().join(format!("cut-video-{:03}.mp4", self.frame_serial));
        self.frame_serial = self.frame_serial.wrapping_add(1);
        self.frame_status = "Rendering the cut for your DAW...".to_string();
        let exported = match self.filmcraft.as_mut() {
            Some(host) => host.export_cut_video(&path),
            None => Err("FilmCraft is not available.".to_string()),
        };
        match exported {
            Ok(()) => {
                self.frame_status = format!(
                    "Video written to {} - import it in your DAW (SoundCraft: File > Import > Video)",
                    path.display()
                );
            }
            Err(error) => self.frame_status = error,
        }
    }

    /// Write the Edit sequence's audio as a WAV mix for SoundCraft.
    #[cfg(feature = "filmcraft")]
    fn send_audio_to_soundcraft(&mut self) {
        let path = audio_dir().join(format!("cut-audio-{:03}.wav", self.frame_serial));
        self.frame_serial = self.frame_serial.wrapping_add(1);
        let exported = match self.filmcraft.as_mut() {
            Some(host) => host.export_audio_mix(&path),
            None => Err("FilmCraft is not available.".to_string()),
        };
        match exported {
            Ok(()) => {
                self.maestro_status = format!(
                    "Audio mix written to {} - open it in your DAW (SoundCraft, Lemon Studio, ...)",
                    path.display()
                );
            }
            Err(error) => self.frame_status = error,
        }
    }

    /// Bring a SoundCraft bounce back: new audio track at the start, old tracks muted.
    #[cfg(feature = "filmcraft")]
    fn bring_soundcraft_mix(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open the DAW mix")
            .add_filter("Audio", &["wav", "aif", "aiff", "flac"])
            .pick_file()
        else {
            return;
        };
        let placed = match self.filmcraft.as_mut() {
            Some(host) => host.place_audio_mix(&path),
            None => Err("FilmCraft is not available.".to_string()),
        };
        match placed {
            Ok(_) => {
                self.frame_status = "DAW mix placed at the start".to_string();
                self.select_workspace(Workspace::Edit);
            }
            Err(error) => self.frame_status = error,
        }
    }

    /// Re-render the linked design into its stable PNG. FilmCraft is only asked
    /// to refresh the media when the pixels actually changed.
    #[cfg(feature = "filmcraft")]
    fn sync_design_to_timeline(&mut self) {
        let Some(path) = self
            .design_link
            .as_ref()
            .filter(|link| link.serial == self.design.document_serial)
            .map(|link| link.path.clone())
        else {
            return;
        };
        let bytes = match self.design.render_png_bytes() {
            Ok(bytes) => bytes,
            Err(error) => {
                self.design.doc.status = error;
                return;
            }
        };
        if std::fs::read(&path).ok().as_deref() == Some(bytes.as_slice()) {
            return;
        }
        if let Err(error) = std::fs::write(&path, &bytes) {
            self.design.doc.status = format!("Could not update {}: {error}", path.display());
            return;
        }
        if let Some(host) = self.filmcraft.as_mut()
            && let Err(error) = host.refresh_media()
        {
            self.design.doc.status = error;
        }
    }

    fn timeline_available(&self) -> bool {
        #[cfg(feature = "filmcraft")]
        {
            return self.filmcraft.is_some();
        }
        #[cfg(not(feature = "filmcraft"))]
        {
            false
        }
    }

    /// Export the design and place it at FilmCraft's playhead. The first
    /// placement creates a stable PNG; later placements reuse that media, so
    /// every clip stays live-linked to the same design.
    #[cfg(feature = "filmcraft")]
    fn place_design_on_timeline(&mut self) {
        if self.filmcraft.is_none() {
            self.design.doc.status = self
                .filmcraft_error
                .clone()
                .unwrap_or_else(|| "FilmCraft is not available.".to_string());
            return;
        }

        let path = match self.design_link.as_ref() {
            Some(link) if link.serial == self.design.document_serial => link.path.clone(),
            _ => {
                let path = filmcraft_host::next_overlay_path(self.overlay_serial);
                self.overlay_serial = self.overlay_serial.wrapping_add(1);
                path
            }
        };
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            self.design.doc.status = format!("Could not create {}: {error}", parent.display());
            return;
        }
        if let Err(error) = self.design.export_png_to(&path) {
            self.design.doc.status = error;
            return;
        }

        let existing_item = self
            .design_link
            .as_ref()
            .filter(|link| link.serial == self.design.document_serial)
            .map(|link| link.item);
        let placed = {
            let host = self
                .filmcraft
                .as_mut()
                .expect("FilmCraft was checked above");
            match existing_item {
                // The snapshot was just rewritten: refresh it, then add another
                // clip that points at the same live media item.
                Some(item) => host
                    .refresh_media()
                    .and_then(|()| host.place_existing(item))
                    .map(|()| item),
                None => host.place_design(&path),
            }
        };

        match placed {
            Ok(item) => {
                if existing_item.is_none() {
                    self.design_link = Some(DesignLink {
                        path: path.clone(),
                        item,
                        serial: self.design.document_serial,
                    });
                }
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("design");
                self.design.doc.status = format!("Placed {name} on the FilmCraft timeline");
                self.select_workspace(Workspace::Edit);
            }
            Err(error) => self.design.doc.status = error,
        }
    }

    /// A small, read-only copy of the one shared timeline. It makes the
    /// "same data" promise visible while Photo is in front, without giving
    /// Photo a second edit model to keep in sync.
    #[cfg(feature = "filmcraft")]
    fn photo_shared_timeline(&mut self, ui: &mut egui::Ui) {
        let Some(view) = self
            .filmcraft
            .as_ref()
            .and_then(|host| host.timeline_view())
        else {
            return;
        };

        egui::Panel::bottom("photo_shared_timeline")
            .exact_size(76.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(22, 22, 27))
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Shared timeline")
                            .strong()
                            .size(11.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("read-only · Edit, Effects and Audio use this same sequence")
                            .size(10.0)
                            .color(TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} / {}",
                                timeline_time(view.playhead),
                                timeline_time(view.duration)
                            ))
                            .monospace()
                            .size(10.0)
                            .color(TEXT_DIM),
                        );
                    });
                });

                let (response, painter) = ui
                    .allocate_painter(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
                let rect = response.rect;
                painter.rect_filled(rect, 4.0, Color32::from_rgb(13, 13, 17));

                let inner = rect.shrink2(egui::vec2(5.0, 3.0));
                let span = view.duration.max(0.001);
                let x_of = |seconds: f64| {
                    inner.left() + (seconds / span).clamp(0.0, 1.0) as f32 * inner.width()
                };

                // Video clips share the upper row and audio clips the lower
                // row. Design clips are drawn last so a Photo overlay stays
                // visible when it sits over footage.
                for design_pass in [false, true] {
                    for clip in view.clips.iter().filter(|clip| clip.design == design_pass) {
                        if clip.end <= 0.0 || clip.start >= view.duration {
                            continue;
                        }
                        let x0 = x_of(clip.start.max(0.0));
                        let x1 = x_of(clip.end.min(view.duration)).max(x0 + 1.5);
                        let y = if clip.audio {
                            inner.bottom() - 8.0
                        } else {
                            inner.top() + 1.0
                        };
                        let color = if clip.audio {
                            Color32::from_rgb(42, 160, 128)
                        } else if clip.design {
                            ACCENT
                        } else {
                            Color32::from_rgb(74, 104, 214)
                        };
                        painter.rect_filled(
                            egui::Rect::from_min_max(egui::pos2(x0, y), egui::pos2(x1, y + 7.0)),
                            1.5,
                            color,
                        );
                    }
                }

                let playhead = x_of(view.playhead);
                painter.line_segment(
                    [
                        egui::pos2(playhead, rect.top() + 2.0),
                        egui::pos2(playhead, rect.bottom() - 2.0),
                    ],
                    egui::Stroke::new(1.5, TEXT_BRIGHT),
                );
                let names = view
                    .clips
                    .iter()
                    .map(|clip| clip.name.as_str())
                    .take(6)
                    .collect::<Vec<_>>()
                    .join(" · ");
                let _ = response.on_hover_text(format!(
                    "One sequence · same clips and playhead as the other workspaces\n{names}"
                ));
            });
    }

    fn footer_hint(&self) -> &str {
        if self.workspace == Workspace::Photo {
            #[cfg(feature = "filmcraft")]
            if self
                .design_link
                .as_ref()
                .is_some_and(|link| link.serial == self.design.document_serial)
            {
                return "Live link · edits update the timeline clip when you switch tabs";
            }
            return "Design workspace · Ctrl+S saves · Ctrl+Z undoes";
        }
        match self.workspace {
            Workspace::Edit => "One project · one timeline · Photo designs become clips",
            Workspace::Effects => {
                "EffectCraft compositor · the Edit cut is imported as a composition · Sync cut re-imports it"
            }
            Workspace::Audio => {
                "SoundCraft DAW - Clip from Edit sends the selected clip; Mix to Edit bounces back"
            }
            Workspace::Photo => unreachable!("Photo is handled above"),
            Workspace::Render => {
                "FilmCraft's Export screen and EffectCraft's Render Queue - one tab, both apps' own panels"
            }
            Workspace::Maestro => {
                "Maestro: the local AI studio, opened in your browser - Pinokio keeps it and its models"
            }
            Workspace::ThreeD => "3D: coming soon - a real 3D app has no tab here yet",
        }
    }
}

/// One small remembered setting (the Maestro address and folder).
fn settings_file(name: &str) -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join(format!("{name}.txt"))
}

fn read_setting(name: &str) -> String {
    std::fs::read_to_string(settings_file(name))
        // A file saved by an editor (or PowerShell) can start with a UTF-8 byte
        // order mark; it is not part of the path.
        .map(|text| text.trim_start_matches('\u{feff}').trim().to_string())
        .unwrap_or_default()
}

fn save_setting(name: &str, value: &str) {
    let path = settings_file(name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, value.trim());
}

/// Where Pinokio keeps Maestro, and where Maestro downloads its models. Only a
/// guess at the usual layout: it is shown to the user, never created.
fn default_maestro_folder() -> std::path::PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join("pinokio").join("api").join("Maestro.git")
}

/// Open a folder in Explorer (Windows) - used to show where Maestro's models go.
fn open_in_explorer(folder: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("explorer").arg(folder).spawn()?;
    Ok(())
}

/// Open a URL in the user's default browser. This window's egui build does not
/// hand `open_url` to the platform, so the browser is started directly.
fn open_in_browser(url: &str) {
    #[cfg(windows)]
    {
        // `start` with an empty title argument hands the URL to the default
        // browser, including one with a query string.
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Short minutes:seconds for the Photo timeline strip.
fn timeline_time(seconds: f64) -> String {
    let total = seconds.max(0.0);
    let minutes = (total / 60.0).floor() as u64;
    format!("{minutes:02}:{:04.1}", total % 60.0)
}

/// Where a frame handed from the timeline (or from EffectCraft) to Photo lives.
fn frame_path(kind: &str, serial: u32) -> std::path::PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("frames");
    base.join(format!("{kind}-{serial:03}.png"))
}

/// The interchange folder between the shared design and VectorCraft.
#[cfg(feature = "vectorcraft")]
fn vector_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("vector")
}

/// The interchange folder between the shared design and PhotoCraft.
#[cfg(feature = "photocraft")]
fn pixel_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("pixel")
}

/// The interchange folder between the shared design and FigCraft.
#[cfg(feature = "figcraft")]
fn fig_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("fig")
}

/// The one editable SVG VectorCraft and FigCraft exchange.
///
/// Keeping it outside both apps' folders makes it clear this file belongs to
/// the link, not to either app's own document cache.
#[cfg(any(feature = "vectorcraft", feature = "figcraft"))]
fn shared_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("shared")
}

/// Rendered EffectCraft animations on their way to the Edit timeline.
#[cfg(all(feature = "effectcraft", feature = "filmcraft"))]
fn render_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("render")
}

/// Audio mixes exchanged with SoundCraft.
#[cfg(feature = "filmcraft")]
fn audio_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CreativeCraft")
        .join("audio")
}

/// One workflow tab: icon above label, dark bar, accent when active.
fn workflow_tab(ui: &mut egui::Ui, workspace: Workspace, active: bool) -> bool {
    let color = if active { TEXT_BRIGHT } else { TEXT_DIM };
    let text = RichText::new(format!("{}\n{}", workspace.icon(), workspace.label()))
        .size(13.0)
        .color(color);
    let button = egui::Button::new(text)
        .fill(if active {
            WORKSPACE_BG
        } else {
            Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::NONE)
        .corner_radius(6)
        .min_size(egui::vec2(96.0, 50.0));
    ui.add(button).clicked()
}

/// A placeholder for a workspace whose host could not start.
fn unavailable_panel(ui: &mut egui::Ui, workspace: Workspace, error: &str) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(90.0);
            ui.label(RichText::new(workspace.icon()).size(44.0));
            ui.add_space(12.0);
            ui.label(
                RichText::new(workspace.app())
                    .size(28.0)
                    .strong()
                    .color(TEXT_BRIGHT),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(workspace.blurb()).size(14.0).color(TEXT_DIM));
            ui.add_space(28.0);
            ui.label(RichText::new(error).italics().color(ACCENT));
        });
    });
}

/// One persona pill: icon and label side by side, accent fill when active.
fn persona_pill(ui: &mut egui::Ui, persona: Persona, active: bool) -> bool {
    let color = if active { Color32::WHITE } else { TEXT_DIM };
    let text = RichText::new(format!("{}  {}", persona.icon(), persona.label()))
        .size(14.0)
        .color(color);
    let button = egui::Button::new(text)
        .fill(if active { ACCENT } else { Color32::TRANSPARENT })
        .stroke(egui::Stroke::NONE)
        .corner_radius(18)
        .min_size(egui::vec2(116.0, 32.0));
    ui.add(button).clicked()
}

impl eframe::App for StudioShell {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // ── Level 1: the workflow bar (bottom, DaVinci-style page tabs) ──────
        egui::Panel::bottom("workflow_bar")
            .exact_size(68.0)
            .frame(egui::Frame::new().fill(BAR_BG))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("CreativeCraft")
                            .strong()
                            .size(16.0)
                            .color(TEXT_BRIGHT),
                    );
                    ui.add_space(20.0);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for workspace in Workspace::ALL {
                        if workflow_tab(ui, workspace, self.workspace == workspace) {
                            self.select_workspace(workspace);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(self.workspace.app())
                                .size(12.0)
                                .color(TEXT_DIM),
                        );
                    });
                });
            });

        // ── Level 2: the design personas, only inside Photo ────────────────
        if self.workspace == Workspace::Photo {
            egui::Panel::top("persona_bar")
                .exact_size(48.0)
                .frame(egui::Frame::new().fill(WORKSPACE_BG))
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        egui::Frame::new()
                            .fill(Color32::from_rgb(20, 20, 24))
                            .corner_radius(22)
                            .inner_margin(egui::Margin::symmetric(6, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    for persona in Persona::ALL {
                                        if persona_pill(ui, persona, self.persona == persona) {
                                            self.persona = persona;
                                        }
                                    }
                                });
                            });
                        ui.add_space(10.0);
                        ui.label(RichText::new("Shared document").size(12.0).color(TEXT_DIM));
                        ui.add_space(10.0);
                        ui.checkbox(
                            &mut self.auto_link,
                            RichText::new("Auto link").size(12.0).color(TEXT_DIM),
                        )
                        .on_hover_text(
                            "On: entering or leaving a persona hands the work over \
                             automatically. Off: only the strip buttons move work between apps.",
                        );
                    });
                });
        }

        // ── Status bar ─────────────────────────────────────────────────────
        egui::Panel::bottom("status_bar")
            .exact_size(26.0)
            .frame(egui::Frame::new().fill(BAR_BG))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(10.0);
                    let (app, view) = match self.workspace {
                        Workspace::Photo => {
                            (self.design.doc.title.as_str(), Some(self.persona.label()))
                        }
                        other => (other.app(), None),
                    };
                    ui.label(RichText::new(app).size(11.0).color(TEXT_DIM));
                    if let Some(view) = view {
                        ui.label(RichText::new("·").size(11.0).color(TEXT_DIM));
                        ui.label(RichText::new(view).size(11.0).color(ACCENT));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new(self.footer_hint()).size(11.0).color(TEXT_DIM));
                    });
                });
            });

        // Photo still shows the shared sequence, read-only, above the status
        // bar. The editable Timeline remains in Edit, Effects and Audio.
        #[cfg(feature = "filmcraft")]
        if self.workspace == Workspace::Photo {
            self.photo_shared_timeline(ui);
        }

        // ── The active view ────────────────────────────────────────────────
        if self.workspace == Workspace::Photo {
            // The Layout persona is the real FigCraft, linked to the design by
            // SVG in and PNG back.
            #[cfg(feature = "figcraft")]
            if self.persona == Persona::Layout {
                if self.figcraft.is_some() {
                    self.figcraft_strip(ui);
                    if let Some(host) = self.figcraft.as_mut() {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(ui, |ui| host.ui(ui));
                    }
                } else {
                    let error = self
                        .figcraft_error
                        .as_deref()
                        .unwrap_or("FigCraft is not available in this build.");
                    unavailable_panel(ui, Workspace::Photo, error);
                }
                return;
            }
            // The Pixel persona is the real PhotoCraft, linked to the design
            // by a flattened image.
            #[cfg(feature = "photocraft")]
            if self.persona == Persona::Pixel {
                if self.photocraft.is_some() {
                    self.photocraft_strip(ui);
                    if let Some(host) = self.photocraft.as_mut() {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(ui, |ui| host.ui(ui, frame));
                    }
                } else {
                    let error = self
                        .photocraft_error
                        .as_deref()
                        .unwrap_or("PhotoCraft is not available in this build.");
                    unavailable_panel(ui, Workspace::Photo, error);
                }
                return;
            }
            // The Vector persona is the real VectorCraft, linked to the design
            // by SVG. Layout stays the shared CreativeCraft design.
            #[cfg(feature = "vectorcraft")]
            if self.persona == Persona::Vector {
                if self.vectorcraft.is_some() {
                    self.vectorcraft_strip(ui);
                    if let Some(host) = self.vectorcraft.as_mut() {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(ui, |ui| host.ui(ui));
                    }
                } else {
                    let error = self
                        .vectorcraft_error
                        .as_deref()
                        .unwrap_or("VectorCraft is not available in this build.");
                    unavailable_panel(ui, Workspace::Photo, error);
                }
                return;
            }
            self.design
                .show(ui, self.persona, self.timeline_available());
            if self.design.take_place_on_timeline_request() {
                #[cfg(feature = "filmcraft")]
                self.place_design_on_timeline();
            }
            // The reverse link: pull the Edit frame into the design document.
            #[cfg(feature = "filmcraft")]
            if self.design.take_frame_from_timeline_request() {
                self.send_edit_frame_to_photo();
            }
        } else {
            // Render: the two apps' own output panels. FilmCraft's Export
            // screen renders the cut; EffectCraft's Render Queue renders the
            // animation.
            if self.workspace == Workspace::Render {
                self.render_tab(ui, frame);
                return;
            }
            // Maestro: its own web page, laid over the tab.
            if self.workspace == Workspace::Maestro {
                self.maestro_tab(ui);
                return;
            }
            // 3D: a placeholder until a real 3D app can be hosted here.
            if self.workspace == Workspace::ThreeD {
                self.three_d_tab(ui);
                return;
            }
            // Effects is EffectCraft's own interface, linked to the same cut.
            #[cfg(feature = "effectcraft")]
            if self.workspace == Workspace::Effects && self.effects.is_some() {
                self.effects_link_strip(ui);
                if let Some(host) = self.effects.as_mut() {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ui, |ui| host.ui(ui, frame));
                }
                return;
            }
            // EffectCraft could not start: say so instead of showing a
            // FilmCraft workspace under an Effects label.
            #[cfg(feature = "effectcraft")]
            if self.workspace == Workspace::Effects {
                let error = self
                    .effects_error
                    .as_deref()
                    .unwrap_or("EffectCraft is not available in this build.");
                unavailable_panel(ui, Workspace::Effects, error);
                return;
            }
            // Audio: the real SoundCraft, filling the tab, with the Edit cut's
            // audio as a timeline underneath. FilmCraft's own Audio workspace
            // is one menu item away for the clip mixer and Essential Sound.
            #[cfg(feature = "soundcraft")]
            if self.workspace == Workspace::Audio {
                if self.audio_daw {
                    if self.soundcraft.is_some() {
                        self.audio_timeline_strip(ui);
                        // The whole DAW is a drop target: a clip dragged out of
                        // the timeline below lands on a new SoundCraft track.
                        let mut dropped = None;
                        if let Some(host) = self.soundcraft.as_mut() {
                            let (_, payload) = ui
                                .dnd_drop_zone::<AudioDrag, ()>(egui::Frame::NONE, |ui| {
                                    host.ui(ui)
                                });
                            dropped = payload.map(|drag| (*drag).clone());
                        }
                        if let Some(drag) = dropped {
                            self.send_clip_to_soundcraft(
                                &drag.name,
                                &drag.path,
                                drag.start,
                                drag.source_in,
                            );
                        }
                        return;
                    }
                    let error = self
                        .soundcraft_error
                        .as_deref()
                        .unwrap_or("SoundCraft is not available in this build.");
                    unavailable_panel(ui, Workspace::Audio, error);
                    return;
                }
                // The FilmCraft mixer: the same Edit audio timeline stays
                // underneath, with its way back to SoundCraft.
                self.audio_timeline_strip(ui);
            }
            // Edit: a strip that hands the current frame to Photo.
            #[cfg(feature = "filmcraft")]
            if self.workspace == Workspace::Edit && self.filmcraft.is_some() {
                self.edit_frame_strip(ui);
            }
            #[cfg(feature = "filmcraft")]
            if let Some(host) = self.filmcraft.as_mut() {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| host.ui(ui, frame));
                return;
            }
            let workspace = self.workspace;
            #[cfg(feature = "filmcraft")]
            let error = self
                .filmcraft_error
                .as_deref()
                .unwrap_or("FilmCraft is not available in this build.");
            #[cfg(not(feature = "filmcraft"))]
            let error = "FilmCraft support is not compiled in.";
            unavailable_panel(ui, workspace, error);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Closing the window is the shell's decision. FilmCraft and EffectCraft
        // answer the OS close by cancelling it and asking "save changes?" in
        // their own window — which is not drawn on the other tabs, so the click
        // did nothing at all. Save every tab here, then close for real: the
        // hosted apps never see this close request and cannot hold it.
        if self.closing {
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.closing = true;
            #[cfg(feature = "filmcraft")]
            self.save_everything();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        // The last close saved everything into one folder; reopening it happens
        // here, in the first frame's own stack, not inside `new` (see
        // `restore_pending`).
        #[cfg(feature = "filmcraft")]
        if std::mem::take(&mut self.restore_pending) {
            self.restore_last_project();
        }
        // Maestro's backend (when this tab started it) may have printed its
        // address since the last frame.
        self.poll_maestro(ctx);
        self.poll_maestro_install();
        self.poll_maestro_uninstall(ctx);
        #[cfg(not(feature = "filmcraft"))]
        {
            let _ = (ctx, frame);
        }
        // FilmCraft stays live in the background: playback, frame jobs and
        // save prompts keep working while Photo is in front.
        #[cfg(feature = "filmcraft")]
        {
            // While EffectCraft is in front FilmCraft is hidden, and decoding
            // 1080p60 footage twice at once is what made the compositor crawl.
            // Pause FilmCraft's background work there, but never while one of
            // its dialogs is waiting for an answer.
            let dialog_open = self
                .filmcraft
                .as_ref()
                .is_some_and(filmcraft_host::FilmcraftHost::has_dialog);
            if self.workspace != Workspace::Effects || dialog_open {
                if let Some(host) = self.filmcraft.as_mut() {
                    host.logic(ctx, frame);
                }
            }
            let dialog_open = dialog_open
                || self
                    .filmcraft
                    .as_ref()
                    .is_some_and(filmcraft_host::FilmcraftHost::has_dialog);
            if dialog_open && self.workspace == Workspace::Photo {
                self.select_workspace(Workspace::Edit);
            }
        }
        // EffectCraft's compositor, preview prefetch and render queue run while
        // its tab is in front — and on the Render tab, where its Render Queue
        // keeps working. They do not run in the background on purpose: Edit
        // keeps its own playback clock and frame server busy.
        #[cfg(feature = "effectcraft")]
        if matches!(
            self.workspace,
            Workspace::Effects | Workspace::Render | Workspace::ThreeD
        ) {
            if let Some(host) = self.effects.as_mut() {
                host.logic(ctx, frame);
            }
            self.keep_our_window_size(ctx);
        } else {
            self.effects_window_guard = None;
        }
        // SoundCraft's own transport, meters and autosave, every frame: a stop
        // requested by a tab switch is applied here, and a bounce keeps
        // reporting even if the tab is left mid-job.
        #[cfg(feature = "soundcraft")]
        if let Some(host) = self.soundcraft.as_mut() {
            if self.workspace == Workspace::Audio && self.audio_daw {
                host.ensure_fonts(ctx);
            }
            host.logic(ctx);
        }
        // VectorCraft runs while its persona is in front; it is a whole
        // application (fonts, canvases, dialogs) and has no background work.
        #[cfg(feature = "vectorcraft")]
        {
            // Automatic, two-way hand-off on a persona switch: entering Vector
            // always opens the current design; leaving it returns artwork that
            // was changed here into the design.
            let active = self.workspace == Workspace::Photo && self.persona == Persona::Vector;
            if active {
                // Also re-checked every frame: another app's result may have
                // landed after this persona was entered (the hand-offs run in
                // tab order, so a leaving app can return its art after the
                // entering one has already opened).
                self.vector_active = true;
                if self.auto_link {
                    self.enter_vector();
                }
            } else if self.vector_active {
                self.vector_active = false;
                if self.auto_link {
                    self.leave_vector();
                }
            }
            if active && let Some(host) = self.vectorcraft.as_mut() {
                host.logic(ctx);
            }
        }
        // PhotoCraft in the Pixel persona: the same automatic hand-off.
        #[cfg(feature = "photocraft")]
        {
            let active = self.workspace == Workspace::Photo && self.persona == Persona::Pixel;
            if active {
                self.pixel_active = true;
                if self.auto_link {
                    self.enter_pixel();
                }
            } else if self.pixel_active {
                self.pixel_active = false;
                if self.auto_link {
                    self.leave_pixel();
                }
            }
            if active && let Some(host) = self.photocraft.as_mut() {
                host.logic(ctx, frame);
            }
        }
        // FigCraft in the Layout persona: the same automatic hand-off.
        #[cfg(feature = "figcraft")]
        {
            let active = self.workspace == Workspace::Photo && self.persona == Persona::Layout;
            if active {
                self.fig_active = true;
                if self.auto_link {
                    self.enter_fig();
                }
            } else if self.fig_active {
                self.fig_active = false;
                if self.auto_link {
                    self.leave_fig();
                }
            }
        }
        // The embedded apps share egui's font context, so whichever installed its
        // fonts last wins. Restore the active app's families before the frame is
        // drawn: FilmCraft/EffectCraft use "semibold"/"medium", VectorCraft "ui".
        #[cfg(feature = "filmcraft")]
        {
            #[cfg(feature = "soundcraft")]
            let filmcraft_front = self.workspace == Workspace::Edit
                || (self.workspace == Workspace::Audio && !self.audio_daw)
                || (self.workspace == Workspace::Render && self.render_side == RenderSide::Cut);
            #[cfg(not(feature = "soundcraft"))]
            let filmcraft_front = matches!(self.workspace, Workspace::Edit | Workspace::Audio)
                || (self.workspace == Workspace::Render && self.render_side == RenderSide::Cut);
            if filmcraft_front && let Some(host) = self.filmcraft.as_ref() {
                host.ensure_fonts(ctx);
            }
        }
        #[cfg(feature = "effectcraft")]
        if (self.workspace == Workspace::Effects
            || (self.workspace == Workspace::Render && self.render_side == RenderSide::Animation))
            && let Some(host) = self.effects.as_mut()
        {
            host.ensure_fonts(ctx);
        }
        // The Photo personas each install their own font families: the one about
        // to draw puts its set back last, or its labels become empty boxes.
        #[cfg(feature = "figcraft")]
        if self.workspace == Workspace::Photo
            && self.persona == Persona::Layout
            && let Some(host) = self.figcraft.as_ref()
        {
            host.ensure_fonts(ctx);
        }
    }

    fn on_exit(&mut self) {
        // Maestro's backend is ours to stop: leaving it running would keep a
        // GPU-held process behind a closed window.
        if let Some(mut launch) = self.maestro_launch.take() {
            launch.stop();
        }
        if let Some(mut install) = self.maestro_install.take() {
            install.stop();
        }
        #[cfg(feature = "effectcraft")]
        effectcraft_ofx::shutdown();
        #[cfg(feature = "soundcraft")]
        if let Some(host) = self.soundcraft.as_mut() {
            host.on_exit();
        }
        #[cfg(feature = "effectcraft")]
        if let Some(host) = self.effects.as_mut() {
            host.on_exit();
        }
        #[cfg(feature = "filmcraft")]
        if let Some(host) = self.filmcraft.as_mut() {
            host.on_exit();
        }
    }
}

/// Start the OpenFX side of the Effects tab.
///
/// OpenFX plug-ins are native libraries from third parties, so they live in a
/// helper process: this executable re-runs itself with `--ofx-worker` for that
/// job. Scanning happens off the UI thread, because some bundles take seconds
/// to describe (and a hung one is killed after a timeout and skipped).
#[cfg(feature = "effectcraft")]
fn start_openfx() {
    if std::env::args().any(|arg| arg == effectcraft_ofx::worker::WORKER_ARG) {
        effectcraft_ofx::worker::serve();
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    effectcraft_ofx::configure_worker(exe);
    let skipped = effectcraft_ofx::skipped_bundles();
    if !skipped.is_empty() {
        eprintln!(
            "[OpenFX] skipping {} bundle(s) that hung or crashed before (see %LOCALAPPDATA%\\CreativeCraft\\ofx-skip.txt)",
            skipped.len()
        );
    }
    std::thread::Builder::new()
        .name("openfx-scan".into())
        .spawn(|| {
            let count = effectcraft_ofx::scan_and_register_all();
            eprintln!("[OpenFX] {count} effect(s) available in Effects & Presets");
        })
        .ok();
}

fn main() -> eframe::Result<()> {
    #[cfg(feature = "effectcraft")]
    start_openfx();
    // FilmCraft's panels need room: at 1000x620 points its toolbar and the
    // Program monitor's timecodes overlap each other. 1360x880 fits the usual
    // 1080p screen (1003 points tall beside the taskbar at 150%) with room for
    // both the Edit and the EffectCraft layouts.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CreativeCraft")
            .with_inner_size([1360.0, 880.0])
            .with_position([10.0, 10.0])
            .with_min_inner_size([1000.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "CreativeCraft",
        options,
        Box::new(|cc| Ok(Box::new(StudioShell::new(cc)))),
    )
}
