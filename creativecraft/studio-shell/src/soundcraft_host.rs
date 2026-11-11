//! The real SoundCraft DAW behind the Audio tab.
//!
//! SoundCraft's whole application is one widget (`SoundApp::ui`) over an
//! `Engine`, so the shell hosts it exactly like the other Craft apps. The link
//! to the Edit cut is audio, not a shared session: SoundCraft has no AAF/OMF
//! interchange yet, so clips and mixes travel as files and each app keeps a
//! normal project of its own.

use std::path::Path;

use eframe::egui;
use serde_json::json;
use soundcraft_audio_io::FileFormat;
use soundcraft_engine::Engine;
use soundcraft_playback::{AudioConfig, Player};
use soundcraft_ui_egui::{Services, SoundApp};

pub struct SoundcraftHost {
    app: SoundApp,
}

impl SoundcraftHost {
    /// Start SoundCraft with the system's default audio device.
    pub fn new() -> Result<Self, String> {
        Self::with_audio(true)
    }

    /// Start SoundCraft without opening an audio device. Tests use this: they
    /// must not take the machine's output while something else is playing.
    #[cfg(test)]
    pub fn without_audio() -> Result<Self, String> {
        Self::with_audio(false)
    }

    fn with_audio(audio: bool) -> Result<Self, String> {
        let engine = Engine::default();
        let player =
            audio.then(|| Player::with_config(engine.session_arc(), &AudioConfig::default()));
        Ok(Self {
            app: SoundApp::new(engine, player, services()),
        })
    }

    /// SoundCraft installs its fonts once, at startup, but every other hosted
    /// app replaces the context's whole font set when it becomes front (that is
    /// how each one gets its own faces). So before drawing, put SoundCraft's
    /// set back if it is gone; otherwise its text renders as boxes after a
    /// round trip through Edit or Photo.
    pub fn ensure_fonts(&self, ctx: &egui::Context) {
        let installed = ctx.fonts(|fonts| {
            fonts
                .definitions()
                .families
                .contains_key(&egui::FontFamily::Name("bold".into()))
        });
        if !installed {
            soundcraft_ui_egui::fonts::install(ctx, true);
        }
    }

    /// Draw the whole SoundCraft window contents into `ui`.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.app.guarded(|app| app.ui(ui));
    }

    /// SoundCraft's per-frame work: transport, meters, document sync. It runs
    /// on every frame, not only while the tab is in front: a bounce keeps
    /// reporting, autosave keeps going and a stop requested by the tab switch
    /// is applied here.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.app.guarded(|app| app.logic(ctx));
    }

    /// Stop SoundCraft's transport, so leaving the Audio tab never leaves two
    /// applications playing at once. Applied on the next [`logic`](Self::logic)
    /// tick, which is where SoundCraft acts on transport requests.
    pub fn stop(&mut self) {
        let _ = self.app.engine.execute("transport.stop", &json!({}));
    }

    /// Import one audio file as a new track at `at` seconds on SoundCraft's
    /// timeline. This is how a clip selected in Edit reaches the Audio tab.
    pub fn import_audio(&mut self, file: &Path, at: f64) -> Result<(), String> {
        self.app
            .engine
            .execute(
                "file.import_audio",
                &json!({
                    "path": file.to_string_lossy(),
                    "at": { "seconds": at.max(0.0) },
                    "new_tracks": true,
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("SoundCraft could not import {}: {error}", file.display()))?;
        // Put the playhead on the clip so it is on screen, not scrolled away.
        let _ = self.app.engine.execute(
            "transport.locate",
            &json!({ "at": { "seconds": at.max(0.0) } }),
        );
        Ok(())
    }

    /// Render the whole session to `path` (WAV). The range is explicit so a
    /// time selection in SoundCraft cannot silently shorten the bounce.
    pub fn bounce(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        let seconds = self.duration().max(0.05);
        self.app
            .engine
            .execute(
                "file.bounce_mix",
                &json!({
                    "path": path.to_string_lossy(),
                    "format": "wav",
                    "start": { "seconds": 0.0 },
                    "end": { "seconds": seconds },
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("SoundCraft could not bounce the mix: {error}"))
    }

    /// How long the session's content runs, in seconds.
    pub fn duration(&self) -> f64 {
        let session = self.app.engine.session();
        session.sample_rate.seconds(session.content_end().max(0))
    }

    /// How many tracks the session has; 0 means nothing has been imported yet.
    pub fn track_count(&self) -> usize {
        self.app.engine.session().tracks.len()
    }

    /// Write the whole session to `path` (Save Session As), replacing whatever
    /// the shell saved there on the last close.
    pub fn save_session(&mut self, path: &Path) -> Result<(), String> {
        self.app
            .engine
            .execute(
                "session.save_as",
                &json!({
                    "path": path.to_string_lossy(),
                    "replace": true,
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("SoundCraft could not save {}: {error}", path.display()))
    }

    /// Open a session file over the current one, for reopening the last mix.
    pub fn open_session(&mut self, path: &Path) -> Result<(), String> {
        self.app
            .engine
            .execute("session.open", &json!({ "path": path.to_string_lossy() }))
            .map(|_| ())
            .map_err(|error| format!("SoundCraft could not open {}: {error}", path.display()))
    }

    pub fn on_exit(&mut self) {
        // Release the player's plugin instances before the process ends, the
        // same order SoundCraft's own app uses.
        self.app.player = None;
        soundcraft_engine::shutdown_plugin_hosts();
    }
}

/// File dialogs for SoundCraft's own File menu, wired to the same rfd dialog
/// the rest of the shell uses. Without these its Open/Save items would do
/// nothing inside CreativeCraft.
fn services() -> Services {
    Services {
        pick_open: Some(Box::new(|purpose: &str, _exts: &[&str]| {
            let dialog = match purpose {
                "session.open" => {
                    rfd::FileDialog::new().add_filter("SoundCraft session", &["scraft"])
                }
                "file.import_midi" => {
                    rfd::FileDialog::new().add_filter("MIDI", &["mid", "midi", "smf"])
                }
                _ => rfd::FileDialog::new().add_filter("Audio", &FileFormat::readable_extensions()),
            };
            dialog.pick_file()
        })),
        pick_save: Some(Box::new(|purpose: &str, path: &Path| {
            let mut dialog = if purpose == "file.bounce_mix" {
                rfd::FileDialog::new().add_filter("Audio", &FileFormat::writable_extensions())
            } else {
                rfd::FileDialog::new().add_filter("SoundCraft session", &["scraft"])
            };
            if let Some(dir) = path.parent().filter(|dir| dir.is_dir()) {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) {
                dialog = dialog.set_file_name(name);
            }
            dialog.save_file()
        })),
        pick_folder: Some(Box::new(|start: &Path| {
            rfd::FileDialog::new().set_directory(start).pick_folder()
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "creativecraft-soundcraft-{}-{tag}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        dir
    }

    /// A quarter second of stereo 48 kHz silence, written by hand so the test
    /// needs no encoder to make an input file.
    fn silent_wav(path: &Path) {
        let rate: u32 = 48_000;
        let channels: u16 = 2;
        let bits: u16 = 16;
        let frames = rate / 4;
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

    /// The whole Audio link: a timeline clip imports as a track and the
    /// session bounces back to a real WAV.
    #[test]
    fn a_clip_imports_and_the_mix_bounces() {
        let dir = temp_dir("clip");
        let wav = dir.join("clip.wav");
        silent_wav(&wav);

        let mut host = SoundcraftHost::without_audio().expect("SoundCraft starts");
        host.import_audio(&wav, 0.0).expect("the clip imports");
        assert!(host.track_count() >= 1, "the clip made a track");

        let mix = dir.join("mix.wav");
        host.bounce(&mix).expect("the mix bounces");
        let bytes = std::fs::read(&mix).expect("the bounce is a file");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(&bytes[..4], b"RIFF", "the bounce is a WAV");
        assert!(bytes.len() > 44, "the bounce has samples");
    }

    /// A clip can be placed later on the timeline, which is how an Edit clip
    /// keeps its position inside SoundCraft.
    #[test]
    fn a_clip_can_be_placed_at_its_timeline_position() {
        let dir = temp_dir("placed");
        let wav = dir.join("clip.wav");
        silent_wav(&wav);

        let mut host = SoundcraftHost::without_audio().expect("SoundCraft starts");
        host.import_audio(&wav, 2.5).expect("the clip imports");
        let session = host.app.engine.session();
        let starts: Vec<f64> = session
            .tracks
            .iter()
            .flat_map(|track| track.playlists.iter())
            .flat_map(|playlist| playlist.clips.iter())
            .map(|clip| session.sample_rate.seconds(clip.start))
            .collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            starts.iter().any(|start| (*start - 2.5).abs() < 0.01),
            "the clip sits at 2.5 s: {starts:?}"
        );
    }
}
