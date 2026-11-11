//! The one-project host.
//!
//! FilmCraft owns the shared project, media pool, sequences, timeline and
//! playhead. This module wraps one `FilmcraftApp` so the shell can show its
//! Editing, Effects and Audio workspaces in the Edit, Effects and Audio
//! tabs, and can insert the Photo design as a real clip on the same timeline.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::App as _;
use eframe::egui;
use filmcraft_engine::Session;
use filmcraft_ui_egui::FilmcraftApp;
use filmcraft_ui_egui::dock::PanelKind;
use serde_json::json;

/// How long a placed design stays on the timeline, in seconds.
pub const OVERLAY_SECONDS: f64 = 5.0;

/// Name the demo project after the window that owns it. FilmCraft's own demo
/// project is called "FilmCraft Demo", which is also the folder the shell saves
/// under; inside CreativeCraft that name is the last leftover of the app the
/// tab hosts. Only that exact name is changed, so a project the user opened or
/// renamed is never touched.
fn adopt_creativecraft_name(session: &mut Session) {
    if session.project.name != "FilmCraft Demo" {
        return;
    }
    let name = "CreativeCraft Demo".to_string();
    let project = std::sync::Arc::make_mut(&mut session.project);
    project.root.name = name.clone();
    project.name = name;
}

/// A read-only copy of the shared sequence for the small timeline in Photo.
/// Times are seconds so the shell does not need FilmCraft's tick type.
#[derive(Clone, Debug, Default)]
pub struct TimelineView {
    pub duration: f64,
    pub playhead: f64,
    pub clips: Vec<TimelineClipView>,
}

#[derive(Clone, Debug)]
pub struct TimelineClipView {
    pub name: String,
    pub start: f64,
    pub end: f64,
    pub audio: bool,
    pub design: bool,
}

/// One audio clip selected in the Edit timeline: the file to hand a DAW, and
/// where it sits in the cut.
pub struct SelectedAudioClip {
    pub name: String,
    pub path: PathBuf,
    /// Timeline start, seconds.
    pub start: f64,
    /// Where in the source file the clip begins, seconds.
    pub source_in: f64,
}

/// One audio clip of the Edit sequence, as the Audio tab's timeline draws it.
#[derive(Clone, Debug)]
pub struct AudioClipView {
    pub id: u64,
    pub name: String,
    pub start: f64,
    pub end: f64,
    /// Where in the source file the clip begins, seconds.
    pub source_in: f64,
    /// The clip's file, when it is real media. Generators (the demo footage)
    /// have none, so they cannot be dragged into a DAW.
    pub path: Option<PathBuf>,
    /// Which audio track of the cut it is on: the strip draws one row per
    /// track, so overlapping clips stay readable.
    pub lane: usize,
}

/// The audio half of the Edit sequence: what the Audio tab's timeline shows.
#[derive(Clone, Debug)]
pub struct AudioTimelineView {
    pub duration: f64,
    pub playhead: f64,
    pub clips: Vec<AudioClipView>,
}

pub struct FilmcraftHost {
    app: FilmcraftApp,
    /// The dedicated video track that holds placed designs. Kept so repeated
    /// placements stack instead of adding a new track every time.
    overlay_track: Option<u64>,
}

impl FilmcraftHost {
    /// Start one FilmCraft session on an **empty** project.
    ///
    /// CreativeCraft opens on a blank slate: no demo cut, no sample media, no
    /// clips on the timeline. FilmCraft's own `Session::default()` is an
    /// "Untitled" project with no sequence, which is exactly what its File >
    /// New Project gives. A project name the user later chooses becomes the
    /// folder under %LOCALAPPDATA%\CreativeCraft\projects.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let session = Session::default();

        let mut app = FilmcraftApp::new(session);
        if let Some(render_state) = cc.wgpu_render_state.clone()
            && std::env::var_os("FILMCRAFT_CPU_COMPOSITE").is_none()
        {
            app.set_wgpu(render_state);
        }
        install_file_dialogs(&mut app);
        Ok(Self {
            app,
            overlay_track: None,
        })
    }

    /// FilmCraft's own workspace names: Editing, Effects and Audio.
    pub fn set_workspace(&mut self, name: &str) {
        self.app.set_workspace(name);
    }

    /// The shared session, for interchange with the other apps' hosts.
    pub fn session_mut(&mut self) -> &mut Session {
        &mut self.app.session
    }

    /// The project's name, which is also the folder the shell keeps it in.
    pub fn project_name(&self) -> String {
        self.app.session.project.name.clone()
    }

    /// Write the whole project to `path` (Save As). The project adopts the file
    /// so it is no longer dirty, exactly as if the user had saved it.
    pub fn save_project(&mut self, path: &Path) -> Result<(), String> {
        self.app
            .session
            .execute("file.saveAs", json!({ "path": path.to_string_lossy() }))
            .map(|_| ())
            .map_err(|error| format!("FilmCraft could not save {}: {error}", path.display()))
    }

    /// Open a project file over the demo, for reopening the last session.
    pub fn open_project(&mut self, path: &Path) -> Result<(), String> {
        self.app
            .session
            .execute("file.open", json!({ "path": path.to_string_lossy() }))
            .map_err(|error| format!("FilmCraft could not open {}: {error}", path.display()))?;
        adopt_creativecraft_name(&mut self.app.session);
        Ok(())
    }

    /// Show FilmCraft's Export screen: the header's Export mode over the
    /// Editing workspace. This is the real export panel (presets, output
    /// settings, its queue), the one its own File > Export menu opens.
    pub fn show_export(&mut self) {
        self.app.set_workspace("Editing");
        self.app.ui.mode = filmcraft_ui_egui::state::Mode::Export;
    }

    /// Put the Edit timeline back in front (leaving the Render tab).
    pub fn show_editing(&mut self) {
        if self.app.ui.mode != filmcraft_ui_egui::state::Mode::Edit {
            self.app.ui.mode = filmcraft_ui_egui::state::Mode::Edit;
        }
    }

    /// Make the point of a workspace visible immediately. Effects brings
    /// Effect Controls forward and selects the video clip at the playhead;
    /// Audio brings the Track Mixer forward and selects the audio clip there.
    /// Without a selection those panels only show an empty state.
    pub fn focus_workspace(&mut self, name: &str) {
        let (panel, message) = match name {
            "Effects" => (
                PanelKind::EffectControls,
                select_clip_for(&mut self.app.session, true),
            ),
            "Audio" => (
                PanelKind::AudioTrackMixer,
                select_clip_for(&mut self.app.session, false),
            ),
            _ => return,
        };
        self.app.show_panel(panel);
        if !message.is_empty() {
            self.app.ui.status = message;
        }
    }

    /// The shared sequence, flattened for the Photo timeline strip.
    pub fn timeline_view(&self) -> Option<TimelineView> {
        let sequence = self.app.session.active_sequence()?;
        let playhead = self.app.session.playhead().seconds();
        let duration = sequence.duration().seconds().max(0.0);
        let mut clips = Vec::new();
        for (audio, tracks) in [
            (false, &sequence.video_tracks),
            (true, &sequence.audio_tracks),
        ] {
            for track in tracks {
                for item in &track.items {
                    clips.push(TimelineClipView {
                        name: item.name.clone(),
                        start: item.start.seconds(),
                        end: item.end().seconds(),
                        audio,
                        design: !audio && item.name.starts_with("design-"),
                    });
                }
            }
        }
        Some(TimelineView {
            duration,
            playhead,
            clips,
        })
    }

    /// The audio clips of the Edit sequence, for the Audio tab's timeline: the
    /// same cut the DAW is fed from, with the playhead, so you can see what is
    /// going to be dragged across.
    pub fn audio_timeline_view(&self) -> AudioTimelineView {
        audio_timeline(&self.app.session)
    }

    /// Select one clip in the Edit timeline, so a click in the Audio tab's
    /// timeline is the same selection the Edit tab shows.
    pub fn select_clip(&mut self, id: u64) {
        let _ = self
            .app
            .session
            .execute("timeline.select", json!({ "clips": [id] }));
    }

    pub fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }
    /// Save the Program frame under the playhead as a PNG. FilmCraft's own
    /// "Export Frame" path renders the sequence at full resolution, so Photo
    /// receives exactly the cut, colour and effects included.
    pub fn export_frame_png(&mut self, path: &Path) -> Result<(u32, u32), String> {
        export_program_frame(&mut self.app.session, path)
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }

    pub fn on_exit(&mut self) {
        self.app.on_exit();
    }

    /// Put FilmCraft's interface fonts back if another embedded app replaced
    /// them on the shared egui context (VectorCraft installs its own families).
    pub fn ensure_fonts(&self, ctx: &egui::Context) {
        let installed = ctx.fonts(|fonts| {
            fonts
                .definitions()
                .families
                .contains_key(&egui::FontFamily::Name("semibold".into()))
        });
        if !installed {
            filmcraft_ui_egui::theme::install(ctx, &self.app.tokens);
        }
    }

    /// A FilmCraft dialog (save prompt, recovery) is open. The shell brings the
    /// Edit workspace forward so the user can see and answer it.
    pub fn has_dialog(&self) -> bool {
        self.app.dialog.is_some()
    }

    /// Import the design PNG and place it on its own video track at FilmCraft's
    /// current playhead. Returns the imported media item id.
    pub fn place_design(&mut self, png: &Path) -> Result<u64, String> {
        let mut track = self.overlay_track;
        let item = import_and_place(&mut self.app.session, png, &mut track, OVERLAY_SECONDS)?;
        self.overlay_track = track;
        // Record the new file's size and timestamp now. Later edits overwrite
        // the same path, so the next refresh can see that it changed.
        let _ = self.app.session.execute("media.refreshChanged", json!({}));
        self.app.set_workspace("Editing");
        self.show_placed_status();
        Ok(item)
    }

    /// Import a rendered animation and place the whole clip at the playhead.
    ///
    /// Unlike a design still, the clip keeps its own length: the animation
    /// EffectCraft just rendered.
    pub fn place_render(&mut self, video: &Path) -> Result<u64, String> {
        let mut track = self.overlay_track;
        let item = import_still(&mut self.app.session, video)?;
        let track_id = overlay_track_for(&mut self.app.session, &mut track)?;
        self.app
            .session
            .execute(
                "timeline.place",
                json!({
                    "item": item,
                    "track": track_id,
                    "video": true,
                    "audio": false,
                }),
            )
            .map_err(|error| format!("FilmCraft could not place the render: {error}"))?;
        self.overlay_track = track;
        let _ = self.app.session.execute("media.refreshChanged", json!({}));
        self.app.set_workspace("Editing");
        self.app.ui.status = "Rendered animation placed on the Edit timeline".to_string();
        Ok(item)
    }

    /// Render the sequence's audio to a WAV for SoundCraft.
    ///
    /// SoundCraft has no AAF/OMF interchange yet, so a WAV mix is the real link
    /// (the same role FCP7 XML plays between Edit and Effects). The file opens
    /// in SoundCraft's standalone app; its bounce comes back with
    /// [`place_audio_mix`](Self::place_audio_mix).
    pub fn export_audio_mix(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        self.app
            .session
            .execute(
                "file.exportMedia",
                json!({
                    "path": path.to_string_lossy(),
                    "format": "wav",
                    "audio": true,
                    "range": "entire",
                    "wait": true,
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("FilmCraft could not export the audio mix: {error}"))
    }

    /// Render the cut to a half-size H.264 MP4 with its audio, for SoundCraft's
    /// Video track. SoundCraft then has picture and sound in sync while mixing.
    pub fn export_cut_video(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        self.app
            .session
            .execute(
                "file.exportMedia",
                json!({
                    "path": path.to_string_lossy(),
                    "format": "h264",
                    "audio": true,
                    "range": "entire",
                    "scale": 0.5,
                    "wait": true,
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("FilmCraft could not render the cut: {error}"))
    }

    /// Put a SoundCraft bounce at the start of the sequence on a new audio
    /// track, muting the tracks it replaces so the mix is not heard twice.
    pub fn place_audio_mix(&mut self, wav: &Path) -> Result<u64, String> {
        let item = place_audio_at_start(&mut self.app.session, wav)?;
        self.app.set_workspace("Editing");
        self.app.ui.status =
            "SoundCraft mix placed at the start; the original audio tracks are muted".to_string();
        Ok(item)
    }

    /// Put a DAW bounce at `seconds` on a new audio track and leave the rest of
    /// the sequence alone. This is the incremental counterpart of
    /// [`place_audio_mix`](Self::place_audio_mix): an extra layer, not a
    /// replacement for the cut's own audio.
    pub fn place_audio_at(&mut self, wav: &Path, seconds: f64) -> Result<u64, String> {
        let item = import_still(&mut self.app.session, wav)?;
        self.app
            .session
            .execute(
                "timeline.place",
                json!({
                    "item": item,
                    "audio": true,
                    "video": false,
                    "audioTrack": "new",
                    "seconds": seconds.max(0.0),
                }),
            )
            .map_err(|error| format!("FilmCraft could not place the mix: {error}"))?;
        self.app.set_workspace("Editing");
        self.app.ui.status = format!(
            "SoundCraft mix placed at {:.2}s on a new audio track",
            seconds.max(0.0)
        );
        Ok(item)
    }

    /// The audio clip selected in the Edit timeline, ready to hand to a DAW.
    ///
    /// Only a real audio clip on an audio track counts. A still, a generator or
    /// a video clip is refused by name, so "send the selected clip" can never
    /// quietly send something else.
    pub fn selected_audio_clip(&self) -> Result<SelectedAudioClip, String> {
        let session = &self.app.session;
        let Some(sequence) = session.active_sequence() else {
            return Err("FilmCraft has no sequence open.".to_string());
        };
        if session.state.selection.is_empty() {
            return Err("Select an audio clip on the Edit timeline first.".to_string());
        }
        for track in &sequence.audio_tracks {
            for item in &track.items {
                if !session.state.selection.contains(&item.id) {
                    continue;
                }
                let Some(project_item) = session.project.item(item.item) else {
                    continue;
                };
                let Some(media) = project_item.as_media() else {
                    continue;
                };
                let filmcraft_engine::project::MediaRef::File { path } = &media.media else {
                    return Err(format!(
                        "{} is a generator with no audio file, so there is nothing to send. \
                         Import real footage or audio in Edit and select that clip.",
                        project_item.name
                    ));
                };
                return Ok(SelectedAudioClip {
                    name: project_item.name.clone(),
                    path: PathBuf::from(path),
                    start: item.start.seconds(),
                    source_in: item.source_in.seconds(),
                });
            }
        }
        Err("The selected clip is not audio. Select a clip on an audio track.".to_string())
    }

    /// Place another clip that uses an already-imported design. All clips of
    /// the same design share one media item, so one refresh updates them all.
    pub fn place_existing(&mut self, item: u64) -> Result<(), String> {
        let mut track = self.overlay_track;
        place_on_track(&mut self.app.session, item, &mut track, OVERLAY_SECONDS)?;
        self.overlay_track = track;
        self.app.set_workspace("Editing");
        self.show_placed_status();
        Ok(())
    }

    /// Re-read the design PNG after it was rewritten in place. FilmCraft keeps
    /// the media pool and every clip that uses the file in sync.
    pub fn refresh_media(&mut self) -> Result<(), String> {
        self.app
            .session
            .execute("media.refreshChanged", json!({}))
            .map(|_| ())
            .map_err(|error| format!("FilmCraft could not refresh the design media: {error}"))
    }

    fn show_placed_status(&mut self) {
        let message = format!(
            "Design placed on the FilmCraft timeline · {OVERLAY_SECONDS:.0}s at the playhead"
        );
        self.app.ui.status = message;
    }
}

/// Pick the clip that makes a workspace useful. Keep a relevant selection the
/// user already made; otherwise prefer the clip under the playhead, then the
/// first clip on that kind of track. Returns a short status line for the shell.
fn select_clip_for(session: &mut Session, video: bool) -> String {
    let found = {
        let Some(sequence) = session.active_sequence() else {
            return "No sequence is open.".to_string();
        };
        let playhead = session.playhead();
        let tracks = if video {
            &sequence.video_tracks
        } else {
            &sequence.audio_tracks
        };
        let already_selected = session.state.selection.iter().any(|clip| {
            tracks
                .iter()
                .any(|track| track.items.iter().any(|item| item.id == *clip))
        });
        if already_selected {
            return String::new();
        }
        let under_playhead = tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|item| item.start <= playhead && playhead < item.end());
        match under_playhead {
            Some(item) => Some((item.id, item.name.clone(), true)),
            None => tracks
                .iter()
                .flat_map(|track| track.items.iter())
                .min_by_key(|item| item.start.0)
                .map(|item| (item.id, item.name.clone(), false)),
        }
    };

    let Some((clip, name, under_playhead)) = found else {
        return if video {
            "No video clips on the timeline · drag media from Project onto V1".to_string()
        } else {
            "No audio clips on the timeline · drag audio from Project onto an audio track"
                .to_string()
        };
    };

    session.state.selection = vec![clip];
    session.state.transition_selection.clear();
    session.state.gap_selection = None;

    let hint = if video {
        "click the stopwatch in Effect Controls, then the arrow for value/velocity curves"
    } else {
        "Track Mixer, EQ and Essential Sound edit this clip"
    };
    if under_playhead {
        format!("Selected \"{name}\" · {hint}")
    } else {
        format!("No clip at the playhead · selected \"{name}\" · {hint}")
    }
}

/// Save the Program frame under the session's playhead as a still. Split from
/// the app wrapper so the engine path can be tested without an egui frame.
fn export_program_frame(session: &mut Session, path: &Path) -> Result<(u32, u32), String> {
    let result = session
        .execute(
            "file.exportFrame",
            json!({
                "path": path.to_string_lossy(),
                "format": "png",
                "target": "program",
            }),
        )
        .map_err(|error| format!("FilmCraft could not export the frame: {error}"))?;
    let width = result["width"].as_u64().unwrap_or_default() as u32;
    let height = result["height"].as_u64().unwrap_or_default() as u32;
    if width == 0 || height == 0 {
        return Err("FilmCraft exported an empty frame.".to_string());
    }
    Ok((width, height))
}

/// Import a still and place it on the overlay track. Split from the app wrapper
/// so the whole engine path can be tested without an egui frame.
fn import_and_place(
    session: &mut Session,
    png: &Path,
    overlay_track: &mut Option<u64>,
    seconds: f64,
) -> Result<u64, String> {
    let item = import_still(session, png)?;
    place_on_track(session, item, overlay_track, seconds)?;
    Ok(item)
}

/// Insert a SoundCraft bounce at the sequence start on a new audio track, and
/// mute the tracks it replaces.
fn place_audio_at_start(session: &mut Session, wav: &Path) -> Result<u64, String> {
    let item = import_still(session, wav)?;
    let tracks: Vec<u64> = session
        .active_sequence()
        .map(|sequence| {
            sequence
                .audio_tracks
                .iter()
                .map(|track| track.id.0)
                .collect()
        })
        .unwrap_or_default();
    for track in tracks {
        let _ = session.execute(
            "timeline.setTrack",
            json!({ "track": track, "muted": true }),
        );
    }
    session
        .execute(
            "timeline.place",
            json!({
                "item": item,
                "audio": true,
                "video": false,
                "audioTrack": "new",
                "seconds": 0.0,
            }),
        )
        .map_err(|error| format!("FilmCraft could not place the mix: {error}"))?;
    Ok(item)
}

/// Insert an existing media item at the playhead on the overlay track.
fn place_on_track(
    session: &mut Session,
    item: u64,
    overlay_track: &mut Option<u64>,
    seconds: f64,
) -> Result<(), String> {
    let track = overlay_track_for(session, overlay_track)?;
    session
        .execute(
            "timeline.place",
            json!({
                "item": item,
                "track": track,
                "video": true,
                "audio": false,
                "duration": { "seconds": seconds },
            }),
        )
        .map(|_| ())
        .map_err(|error| format!("FilmCraft could not place the design: {error}"))
}

/// The audio half of the sequence: every audio clip, its position, and its
/// file when it has one. Free of the host so it is testable with a session.
pub fn audio_timeline(session: &Session) -> AudioTimelineView {
    let Some(sequence) = session.active_sequence() else {
        return AudioTimelineView {
            duration: 0.0,
            playhead: 0.0,
            clips: Vec::new(),
        };
    };
    let playhead = session.playhead().seconds();
    let duration = sequence.duration().seconds().max(0.0);
    let mut clips = Vec::new();
    for (lane, track) in sequence.audio_tracks.iter().enumerate() {
        for item in &track.items {
            let path = session
                .project
                .item(item.item)
                .and_then(|item| item.as_media())
                .and_then(|media| match &media.media {
                    filmcraft_engine::project::MediaRef::File { path } => Some(PathBuf::from(path)),
                    filmcraft_engine::project::MediaRef::Generator(_) => None,
                });
            clips.push(AudioClipView {
                id: item.id.0,
                name: item.name.clone(),
                start: item.start.seconds(),
                end: item.end().seconds(),
                source_in: item.source_in.seconds(),
                path,
                lane,
            });
        }
    }
    AudioTimelineView {
        duration,
        playhead,
        clips,
    }
}

fn import_still(session: &mut Session, png: &Path) -> Result<u64, String> {
    let path = png.to_string_lossy().into_owned();
    let result = session
        .execute("file.import", json!({ "paths": [path] }))
        .map_err(|error| format!("FilmCraft could not import the design: {error}"))?;
    result
        .get("items")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|value| value.as_u64())
        .ok_or_else(|| "FilmCraft imported the file but returned no media item.".to_string())
}

/// Reuse the existing overlay track, or add one at the top of the sequence.
fn overlay_track_for(session: &mut Session, slot: &mut Option<u64>) -> Result<u64, String> {
    if let Some(id) = *slot
        && session
            .active_sequence()
            .is_some_and(|sequence| sequence.video_tracks.iter().any(|track| track.id.0 == id))
    {
        return Ok(id);
    }

    let result = session
        .execute("sequence.addTracks", json!({ "video": 1 }))
        .map_err(|error| format!("FilmCraft could not add an overlay track: {error}"))?;
    let id = result
        .get("video")
        .and_then(|value| value.as_array())
        .and_then(|tracks| tracks.first())
        .and_then(|value| value.as_u64())
        .ok_or_else(|| "FilmCraft added a track but returned no track id.".to_string())?;
    *slot = Some(id);
    Ok(id)
}

/// A stable folder for design snapshots. They are real media files referenced
/// by the FilmCraft project, so they must outlive the session.
pub fn overlay_dir() -> PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(base).join("CreativeCraft").join("overlays");
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if cfg!(target_os = "macos") {
            return home
                .join("Library")
                .join("Application Support")
                .join("CreativeCraft")
                .join("overlays");
        }
        return home
            .join(".local")
            .join("share")
            .join("CreativeCraft")
            .join("overlays");
    }
    std::env::temp_dir().join("CreativeCraft").join("overlays")
}

/// A unique PNG path for this placement, so an older timeline clip never
/// changes when a new design is placed.
pub fn next_overlay_path(serial: u32) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    overlay_dir().join(format!("design-{stamp}-{serial}.png"))
}

/// Native open/save dialogs for FilmCraft's File menu and media panels.
fn install_file_dialogs(app: &mut FilmcraftApp) {
    app.hooks.pick_files = Some(Box::new(|extensions| {
        rfd::FileDialog::new()
            .add_filter("Media", extensions)
            .pick_files()
            .unwrap_or_default()
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }));
    app.hooks.pick_open_project = Some(Box::new(|| {
        rfd::FileDialog::new()
            .add_filter("FilmCraft Project", &["fcproj"])
            .pick_file()
            .map(|path| path.to_string_lossy().into_owned())
    }));
    app.hooks.pick_save = Some(Box::new(|name| {
        rfd::FileDialog::new()
            .add_filter("FilmCraft Project", &["fcproj"])
            .set_file_name(name)
            .save_file()
            .map(|path| path.to_string_lossy().into_owned())
    }));
    app.hooks.pick_save_as = Some(Box::new(|filter, extensions, name| {
        rfd::FileDialog::new()
            .add_filter(filter, extensions)
            .set_file_name(name)
            .save_file()
            .map(|path| path.to_string_lossy().into_owned())
    }));
    app.hooks.pick_open_file = Some(Box::new(|filter, extensions| {
        rfd::FileDialog::new()
            .add_filter(filter, extensions)
            .pick_file()
            .map(|path| path.to_string_lossy().into_owned())
    }));
    app.hooks.pick_file_for_relink = Some(Box::new(|extensions, _hint| {
        rfd::FileDialog::new()
            .add_filter("Media", extensions)
            .pick_file()
            .map(|path| path.to_string_lossy().into_owned())
    }));
    app.hooks.pick_folder = Some(Box::new(|| {
        rfd::FileDialog::new()
            .pick_folder()
            .map(|path| path.to_string_lossy().into_owned())
    }));
    app.hooks.pick_folder_at = Some(Box::new(|directory| {
        rfd::FileDialog::new()
            .set_directory(directory)
            .pick_folder()
            .map(|path| path.to_string_lossy().into_owned())
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    /// A short stereo 48 kHz WAV, written by hand so the test needs no encoder.
    fn write_silent_wav(path: &Path, seconds: f32) {
        let rate: u32 = 48_000;
        let channels: u16 = 2;
        let bits: u16 = 16;
        let frames = (rate as f32 * seconds) as u32;
        let data_len = frames * u32::from(channels) * u32::from(bits / 8);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * u32::from(bits / 8)).to_le_bytes());
        bytes.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        bytes.extend_from_slice(&bits.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(bytes.len() + data_len as usize, 0);
        std::fs::write(path, bytes).expect("the WAV is written");
    }

    #[test]
    fn a_saved_project_reopens() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");
        let items = session.project.items.len();
        let path = std::env::temp_dir().join(format!(
            "creativecraft-reopen-{}.fcproj",
            std::process::id()
        ));
        session
            .execute("file.saveAs", json!({ "path": path.to_string_lossy() }))
            .expect("the project saves");

        // The window saves on close and opens this same file on the next launch:
        // a project that cannot be read back would lose the whole cut.
        let mut reopened = Session::default();
        reopened
            .execute("file.open", json!({ "path": path.to_string_lossy() }))
            .expect("the saved project opens");
        assert_eq!(reopened.project.items.len(), items, "every item comes back");
        assert!(
            reopened.active_sequence().is_some(),
            "the sequence comes back with the project"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_program_frame_exports_as_a_png() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");
        let path = std::env::temp_dir().join(format!(
            "creativecraft-frame-export-{}.png",
            std::process::id()
        ));
        let (width, height) =
            export_program_frame(&mut session, &path).expect("a frame is written");
        assert!(width > 0 && height > 0, "the frame reports its size");
        let bytes = std::fs::read(&path).expect("the PNG exists");
        let _ = std::fs::remove_file(&path);
        assert!(
            bytes.starts_with(b"\x89PNG"),
            "Photo can read the frame as a PNG"
        );
    }

    #[test]
    fn a_design_png_becomes_a_clip_on_the_overlay_track() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");

        let path = std::env::temp_dir().join(format!(
            "creativecraft-overlay-test-{}.png",
            std::process::id()
        ));
        RgbaImage::from_pixel(4, 4, Rgba([200, 40, 90, 255]))
            .save(&path)
            .expect("test image saves");

        let mut track = None;
        let playhead = session.playhead();
        let item = import_and_place(&mut session, &path, &mut track, 2.0).expect("design places");

        let sequence = session.active_sequence().expect("sequence is active");
        let placed = sequence
            .video_tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|clip| clip.item.0 == item)
            .expect("the imported still is on a track");
        assert_eq!(placed.start, playhead, "the clip lands at the playhead");
        assert!(placed.duration.0 > 0, "the clip has a duration");
        assert!(track.is_some(), "an overlay track was recorded");
        assert!(
            sequence.video_tracks.len() > 1,
            "the overlay gets its own track instead of covering the demo"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_audio_timeline_lists_the_cut_with_its_files() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");

        // A real audio file, written by hand so the test needs no encoder.
        let wav = std::env::temp_dir().join(format!(
            "creativecraft-audio-view-{}.wav",
            std::process::id()
        ));
        write_silent_wav(&wav, 2.0);

        let item = import_still(&mut session, &wav).expect("the WAV imports as media");
        session
            .execute(
                "timeline.place",
                json!({
                    "item": item,
                    "audio": true,
                    "video": false,
                    "audioTrack": "new",
                    "seconds": 1.5,
                }),
            )
            .expect("the clip lands on an audio track");

        // The view needs a session, not a host, so the data side is testable.
        let view = audio_timeline(&session);
        let mine = view
            .clips
            .iter()
            .find(|clip| clip.path.as_deref() == Some(wav.as_path()))
            .expect("the imported WAV is in the audio timeline");
        assert!(
            (mine.start - 1.5).abs() < 0.01,
            "the clip keeps its timeline position: {}",
            mine.start
        );
        // The demo's audio is synthetic: it is listed (so the strip matches the
        // cut) but carries no file, so it cannot be dragged into a DAW.
        assert!(
            view.clips
                .iter()
                .any(|clip| clip.path.is_none() && clip.name.contains("Ocean_Sunset")),
            "generator audio stays in the list without a file: {:?}",
            view.clips.iter().map(|clip| &clip.name).collect::<Vec<_>>()
        );

        // The demo's music bed runs under the whole cut: it must be on a
        // different row, or it would hide every clip it overlaps.
        let bed = view
            .clips
            .iter()
            .find(|clip| clip.name == "Ambient_Score.wav")
            .expect("the music bed is in the list");
        let footage = view
            .clips
            .iter()
            .find(|clip| clip.name.contains("Ocean_Sunset"))
            .expect("the first clip is in the list");
        assert_ne!(
            bed.lane, footage.lane,
            "overlapping clips are drawn on separate rows"
        );

        let _ = std::fs::remove_file(&wav);
    }

    #[test]
    fn entering_effects_selects_the_clip_under_the_playhead() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");
        session.set_playhead(filmcraft_engine::time::Tick::from_seconds_f64(4.0));

        let message = select_clip_for(&mut session, true);

        assert!(
            !session.state.selection.is_empty(),
            "Effects should have a video clip to show: {message}"
        );
        assert!(
            message.contains("Selected"),
            "the shell should explain the selection: {message}"
        );
    }

    #[test]
    fn rewriting_the_design_file_refreshes_its_media() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");

        let path = std::env::temp_dir().join(format!(
            "creativecraft-refresh-test-{}.png",
            std::process::id()
        ));
        RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]))
            .save(&path)
            .expect("test image saves");

        let mut track = None;
        let item = import_and_place(&mut session, &path, &mut track, 2.0).expect("design places");
        // FilmCraft stamps media after import; this is the "seen file" state.
        session
            .execute("media.refreshChanged", json!({}))
            .expect("stamp succeeds");

        // The live link rewrites the same path with new pixels.
        RgbaImage::from_pixel(8, 4, Rgba([200, 10, 10, 255]))
            .save(&path)
            .expect("rewritten image saves");
        let refreshed = session
            .execute("media.refreshChanged", json!({}))
            .expect("refresh succeeds");
        let ids = refreshed["refreshed"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            ids.contains(&json!(item)),
            "the rewritten design should refresh its clip media: {refreshed}"
        );

        let _ = std::fs::remove_file(path);
    }

    /// The Edit cut's audio leaves as a real WAV for SoundCraft.
    #[test]
    fn the_cut_exports_an_audio_mix() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");
        let path = std::env::temp_dir().join(format!(
            "creativecraft-audio-mix-{}.wav",
            std::process::id()
        ));
        session
            .execute(
                "file.exportMedia",
                json!({
                    "path": path.to_string_lossy(),
                    "format": "wav",
                    "audio": true,
                    "range": "entire",
                    "wait": true,
                }),
            )
            .expect("the mix exports");
        let bytes = std::fs::read(&path).expect("the WAV exists");
        let _ = std::fs::remove_file(&path);
        assert!(bytes.starts_with(b"RIFF"), "a real WAV file");
        assert!(bytes.len() > 10_000, "the demo cut has audio in it");
    }

    /// The cut leaves as a video with audio for SoundCraft's Video track.
    #[test]
    fn the_cut_exports_a_video_for_soundcraft() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");
        let path = std::env::temp_dir().join(format!(
            "creativecraft-cut-video-{}.mp4",
            std::process::id()
        ));
        session
            .execute(
                "file.exportMedia",
                json!({
                    "path": path.to_string_lossy(),
                    "format": "h264",
                    "audio": true,
                    "range": "custom",
                    "startSeconds": 0.0,
                    "endSeconds": 1.0,
                    "scale": 0.5,
                    "wait": true,
                }),
            )
            .expect("the clip exports");
        let bytes = std::fs::read(&path).expect("the MP4 exists");
        let _ = std::fs::remove_file(&path);
        assert!(bytes.len() > 1_000, "the file has real video");
        assert!(
            bytes.windows(4).take(64).any(|window| window == b"ftyp"),
            "a real MP4 container"
        );
    }

    /// A SoundCraft bounce comes back at the sequence start and the old tracks
    /// are muted, so the mix is not heard twice.
    #[test]
    fn a_soundcraft_mix_is_placed_at_the_start_and_mutes_the_old_audio() {
        let mut session = Session::default();
        session
            .execute("file.openDemoProject", json!({}))
            .expect("demo project opens");
        let path = std::env::temp_dir().join(format!(
            "creativecraft-soundcraft-mix-{}.wav",
            std::process::id()
        ));
        session
            .execute(
                "file.exportMedia",
                json!({
                    "path": path.to_string_lossy(),
                    "format": "wav",
                    "audio": true,
                    "range": "entire",
                    "wait": true,
                }),
            )
            .expect("the mix exports");

        let before = session
            .active_sequence()
            .map(|sequence| sequence.audio_tracks.len())
            .unwrap_or_default();
        let item = place_audio_at_start(&mut session, &path).expect("the mix is placed");
        let sequence = session.active_sequence().expect("a sequence");
        let clip = sequence
            .audio_tracks
            .iter()
            .flat_map(|track| track.items.iter())
            .find(|clip| clip.item.0 == item)
            .expect("the mix is on an audio track");
        let _ = std::fs::remove_file(&path);

        assert_eq!(clip.start.0, 0, "the mix starts at the sequence start");
        assert!(
            sequence.audio_tracks.len() > before,
            "the mix gets its own audio track"
        );
        assert!(
            sequence
                .audio_tracks
                .iter()
                .filter(|track| track.muted)
                .count()
                >= before,
            "the old audio tracks are muted"
        );
    }
}
