//! The shared design workspace: one document, three persona views.

mod canvas;
mod export;
mod model;
mod svg;
mod ui;

pub use canvas::PaintSettings;
pub use model::{Document, History, Rasters};

use model::{Kind, PhotoSource, Raster, Rect};
use std::collections::HashMap;
use std::path::Path;

use eframe::egui;

/// The Affinity-style personas. They are views of the same design document,
/// not separate applications.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Persona {
    Vector,
    Pixel,
    Layout,
}

impl Persona {
    pub const ALL: [Persona; 3] = [Persona::Vector, Persona::Pixel, Persona::Layout];

    pub fn icon(self) -> &'static str {
        match self {
            Persona::Vector => "✒",
            Persona::Pixel => "🖼",
            Persona::Layout => "⊞",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Persona::Vector => "Vector",
            Persona::Pixel => "Pixel",
            Persona::Layout => "Layout",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Persona::Vector => "Paths, shapes, fills and corners.",
            Persona::Pixel => "Brush, erase and mask on raster layers.",
            Persona::Layout => "Frames, alignment and text layout.",
        }
    }
}

pub struct DesignState {
    pub doc: Document,
    pub history: History,
    pub rasters: Rasters,
    pub selected: Option<model::NodeId>,
    pub paint: PaintSettings,
    /// Bumped whenever a different document is opened. The timeline link uses
    /// it so a new design can never overwrite an older design's clips.
    pub document_serial: u64,
    place_on_timeline_requested: bool,
    frame_from_timeline_requested: bool,
}

impl DesignState {
    pub fn new() -> Self {
        let doc = Document::blank();
        let mut rasters = Rasters::default();
        rasters.sync(&doc);
        Self {
            doc,
            history: History::default(),
            rasters,
            selected: None,
            paint: PaintSettings::default(),
            document_serial: 0,
            place_on_timeline_requested: false,
            frame_from_timeline_requested: false,
        }
    }

    /// The old "Pro plan" starter design, kept as a test fixture. The window no
    /// longer opens on it, but the design→app hand-off tests need a document
    /// with known layers and real text.
    #[cfg(test)]
    pub(crate) fn sample() -> Self {
        let mut state = Self::new();
        state.doc = Document::sample();
        state.rasters = Rasters::default();
        state.rasters.sync(&state.doc);
        state
    }

    pub fn show(&mut self, ui: &mut egui::Ui, persona: Persona, timeline_available: bool) {
        ui::show(self, ui, persona, timeline_available);
    }

    /// Photo asks the shell to put this design on FilmCraft's timeline.
    pub fn request_place_on_timeline(&mut self) {
        self.place_on_timeline_requested = true;
    }

    pub fn take_place_on_timeline_request(&mut self) -> bool {
        std::mem::take(&mut self.place_on_timeline_requested)
    }

    /// Photo asks the shell for the frame under the Edit playhead, as a photo
    /// layer (the reverse of "Place on timeline").
    pub fn request_frame_from_timeline(&mut self) {
        self.frame_from_timeline_requested = true;
    }

    pub fn take_frame_from_timeline_request(&mut self) -> bool {
        std::mem::take(&mut self.frame_from_timeline_requested)
    }

    pub fn save(&mut self) {
        if let Err(error) = self.doc.save(&self.rasters) {
            self.doc.status = error;
        }
    }

    pub fn open(&mut self) {
        match Document::load(&mut self.rasters) {
            Ok(doc) => {
                self.doc = doc;
                self.history.reset();
                self.selected = None;
                self.document_serial = self.document_serial.wrapping_add(1);
            }
            Err(error) => self.doc.status = error,
        }
        self.rasters.sync(&self.doc);
    }

    /// Flatten the document to a PNG at its native size, not the window size.
    pub fn export_png(&mut self) {
        let Some(mut path) = rfd::FileDialog::new()
            .set_title("Export PNG")
            .set_file_name("creativecraft.png")
            .add_filter("PNG image", &["png"])
            .save_file()
        else {
            return;
        };

        // The dialog filter only offers PNG, but a typed name may have another
        // extension. The encoded data is always PNG, so make the name match.
        let is_png = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
        if !is_png {
            path.set_extension("png");
        }

        let width = self.doc.canvas_w.round().max(1.0) as u32;
        let height = self.doc.canvas_h.round().max(1.0) as u32;

        match self.export_png_to(&path) {
            Ok(()) => {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("PNG");
                self.doc.status = format!("Exported {width} × {height} PNG to {name}");
            }
            Err(error) => self.doc.status = error,
        }
    }

    /// Render the PNG in memory. Used by the live link so we can skip writing
    /// and refreshing the timeline when the design has not actually changed.
    pub fn render_png_bytes(&self) -> Result<Vec<u8>, String> {
        let image = export::render_png(&self.doc, &self.rasters)?;
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .map_err(|error| format!("Could not encode the PNG: {error}"))?;
        Ok(bytes)
    }

    /// Render and write the PNG without showing a dialog. Kept separate so the
    /// whole export path can be tested with a temporary file.
    pub fn export_png_to(&mut self, path: &Path) -> Result<(), String> {
        let bytes = self.render_png_bytes()?;
        std::fs::write(path, bytes)
            .map_err(|error| format!("Could not save {}: {error}", path.display()))
    }

    pub fn undo(&mut self) {
        let before = photo_sources(&self.doc);
        if self.history.undo(&mut self.doc) {
            reload_changed_photos(&before, &self.doc, &mut self.rasters);
            self.rasters.sync(&self.doc);
        }
    }

    /// Add a photo layer from an image file on disk. The timeline and Effects
    /// tabs use it to hand a frame to Photo; the same undo step covers it.
    pub fn import_photo_path(&mut self, path: &Path) {
        ui::import_photo(self, path, None);
    }

    /// Add the image as a photo layer, or replace the existing photo layer
    /// `target` (the one VectorCraft's artwork last came back into), so a
    /// round trip updates one layer instead of stacking duplicates.
    /// Make one returned image the whole design.
    ///
    /// Every hosted app hands its result back as a single picture. Replacing the
    /// document with that one layer (instead of stacking a new layer on top of
    /// the old design) is what makes a deletion in one app a deletion in all:
    /// the next app receives exactly this picture, with nothing left underneath
    /// to reappear. The rich, editable document lives on inside the app that
    /// produced the picture until another app takes over.
    pub fn replace_with_artwork(&mut self, path: &Path) -> Result<u64, String> {
        let raster = Raster::load_image(path)?.downscale(4096);
        let aspect = raster.w.max(1) as f32 / raster.h.max(1) as f32;
        let (w, h) = (self.doc.canvas_w.max(1.0), self.doc.canvas_h.max(1.0));
        let (mut iw, mut ih) = (w, w / aspect);
        if ih > h {
            ih = h;
            iw = ih * aspect;
        }
        let rect = Rect::new((w - iw) * 0.5, (h - ih) * 0.5, iw, ih);
        let px = (raster.w as u32, raster.h as u32);

        self.history.checkpoint(&self.doc);
        let mut doc = Document::new(self.doc.title.clone(), w, h);
        let id = doc.add(
            "Artwork",
            Kind::Photo {
                source: PhotoSource::File(path.display().to_string()),
                px,
            },
            rect,
        );
        doc.status = "Updated the shared artwork".into();
        self.doc = doc;
        self.rasters.clear();
        self.rasters.map.insert(id, raster);
        self.selected = Some(id);
        Ok(id)
    }

    /// Write the design as SVG, the document VectorCraft opens.
    pub fn export_svg_to(&self, path: &Path) -> Result<(), String> {
        let svg = svg::export_svg(&self.doc, &self.rasters)?;
        std::fs::write(path, svg)
            .map_err(|error| format!("Could not save {}: {error}", path.display()))
    }

    pub fn redo(&mut self) {
        let before = photo_sources(&self.doc);
        if self.history.redo(&mut self.doc) {
            reload_changed_photos(&before, &self.doc, &mut self.rasters);
            self.rasters.sync(&self.doc);
        }
    }
}

/// Which image each photo layer points at. Used to notice when undo/redo swaps
/// an imported photo back to the previous one.
fn photo_sources(doc: &Document) -> HashMap<model::NodeId, String> {
    doc.nodes
        .iter()
        .filter_map(|node| match &node.kind {
            model::Kind::Photo { source, .. } => Some((
                node.id,
                match source {
                    model::PhotoSource::Sample => "<sample>".to_string(),
                    model::PhotoSource::File(path) => path.clone(),
                },
            )),
            _ => None,
        })
        .collect()
}

fn reload_changed_photos(
    before: &HashMap<model::NodeId, String>,
    after: &Document,
    rasters: &mut Rasters,
) {
    for (id, source) in photo_sources(after) {
        if before.get(&id) != Some(&source) {
            rasters.map.remove(&id);
        }
    }
}

impl Default for DesignState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView as _;

    #[test]
    fn export_writes_a_real_png_at_canvas_size() {
        let mut state = DesignState::new();
        let path = std::env::temp_dir().join(format!(
            "creativecraft-export-test-{}.png",
            std::process::id()
        ));

        state.export_png_to(&path).expect("export writes a file");

        let image = image::open(&path).expect("exported file is a valid image");
        assert_eq!(image.dimensions(), (1200, 800));
        assert!(
            std::fs::metadata(&path).expect("file exists").len() > 1_000,
            "export should contain real image data"
        );

        let _ = std::fs::remove_file(path);
    }

    /// A result coming back from one hosted app must replace the whole design,
    /// not stack on top of it: otherwise a deletion in the next app only hides
    /// one copy and the old picture underneath brings it back.
    #[test]
    fn a_returned_artwork_replaces_the_design_instead_of_stacking() {
        let mut state = DesignState::new();
        // A new session is blank; add the layers this test needs to replace.
        state.doc.add(
            "Background",
            Kind::Frame {
                fill: model::Rgba::CANVAS,
            },
            Rect::new(0.0, 0.0, 1200.0, 800.0),
        );
        state.doc.add(
            "Card",
            Kind::Rect {
                fill: model::Rgba::CARD,
                radius: 28.0,
                stroke: 0.0,
                stroke_color: model::Rgba::WHITE,
            },
            Rect::new(640.0, 120.0, 480.0, 560.0),
        );
        assert!(
            state.doc.nodes.len() > 1,
            "there are layers to replace"
        );
        let source = std::env::temp_dir().join(format!(
            "creativecraft-artwork-test-{}.png",
            std::process::id()
        ));
        state
            .export_png_to(&source)
            .expect("the design renders to a PNG");

        let id = state
            .replace_with_artwork(&source)
            .expect("the artwork is read back");

        assert_eq!(state.doc.nodes.len(), 1, "one shared artwork layer");
        assert_eq!(state.doc.nodes[0].id, id);
        assert!(
            matches!(state.doc.nodes[0].kind, Kind::Photo { .. }),
            "the artwork is stored as the one photo layer"
        );
        assert_eq!(state.doc.canvas_w, 1200.0);
        assert_eq!(state.doc.canvas_h, 800.0);
        assert!(state.rasters.map.contains_key(&id));
        let _ = std::fs::remove_file(source);
    }
}
