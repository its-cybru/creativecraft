//! The real VectorCraft, hosted in the Photo tab's Vector persona.
//!
//! VectorCraft draws into an egui `Ui` the same way EffectCraft does, so the
//! whole illustrator — tools, layers, styles, the canvas — runs inside the
//! CreativeCraft window instead of a look-alike panel.
//!
//! The shared design and VectorCraft each keep their own document. They are
//! linked with **SVG**, the format both sides read and write: the design is
//! exported to an SVG and opened here, artwork is exported back as a PNG at the
//! document's size and becomes a photo layer in the design. That is the same
//! interchange model as Edit ↔ Effects, not a shared in-memory document.

use eframe::egui::{self, FontFamily};
use serde_json::json;
use std::path::{Path, PathBuf};
use vectorcraft_ui_egui::VectorcraftApp;

/// One hosted illustrator.
pub struct VectorcraftHost {
    app: VectorcraftApp,
    /// The undo depth at our last hand-off or export. A deeper history means the
    /// user drew or changed something since, so the design must not replace it
    /// unasked. Export marks the document dirty (it remembers export settings),
    /// which is why a plain dirty flag was not enough.
    clean_undo: Option<usize>,
}

impl VectorcraftHost {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            app: VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default()),
            clean_undo: None,
        })
    }

    /// VectorCraft's interface, drawn inside the shell's Photo area.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.app.ui(ui);
    }

    /// VectorCraft's logic pass: fonts, canvases, dialogs and background jobs.
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.ensure_fonts(ctx);
        self.app.logic(ctx);
    }

    /// Keep VectorCraft's interface families installed.
    ///
    /// FilmCraft installs its own fonts on the shared egui context, and that
    /// drops the `ui` / `ui-semibold` / `mono` families VectorCraft's tabs and
    /// dialogs ask for — their text then draws as empty boxes. The check is a
    /// map lookup; the install only runs when they are actually missing.
    fn ensure_fonts(&mut self, ctx: &egui::Context) {
        let installed = ctx.fonts(|fonts| {
            let families = &fonts.definitions().families;
            families.contains_key(&FontFamily::Name("ui-semibold".into()))
                && families.contains_key(&FontFamily::Name("ui".into()))
        });
        if !installed {
            vectorcraft_ui_egui::theme::install_fonts(ctx);
        }
    }

    /// Open a document — the SVG written from the shared design, or any file
    /// VectorCraft reads.
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        // The design replaces whatever is open. Without this, VectorCraft asks
        // to save the previous (unsaved) document first, which blocks the tab
        // behind a modal dialog.
        let _ = self.app.session.execute("file.closeAll", &json!({}));
        self.app
            .session
            .execute("document.open", &json!({ "path": path.to_string_lossy() }))
            .map_err(|error| format!("VectorCraft could not open {}: {error}", path.display()))?;
        self.mark_clean();
        Ok(())
    }

    /// Whether the artwork has changes the user made since the last hand-off or
    /// export (undo depth), so the design must not replace it without asking.
    pub fn has_unsaved_artwork(&self) -> bool {
        self.clean_undo
            .is_some_and(|clean| self.undo_depth() != clean)
    }

    fn undo_depth(&self) -> usize {
        self.app
            .session
            .doc()
            .map(|state| state.history.undo.len())
            .unwrap_or(0)
    }

    /// Remember the current state as "ours", right after a hand-off or export.
    fn mark_clean(&mut self) {
        self.clean_undo = Some(self.undo_depth());
    }

    /// Write the artwork as SVG, keeping shapes and live text editable.
    ///
    /// This is the file FigCraft opens, so a shape deleted here is really gone
    /// there (and the other way round) instead of being flattened to pixels.
    pub fn export_svg(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        self.app
            .session
            .execute(
                "document.export",
                &json!({
                    "format": "svg",
                    "path": path.to_string_lossy(),
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("VectorCraft could not export SVG: {error}"))?;
        self.mark_clean();
        Ok(())
    }

    /// Render every object on the artboard to its own PNG, in stacking order.
    ///
    /// EffectCraft turns these into separate composition layers, so each part of
    /// the design can be animated on its own.
    pub fn export_layer_pngs(
        &mut self,
        folder: &Path,
    ) -> Result<Vec<crate::layer::LayerPng>, String> {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("Could not create {}: {error}", folder.display()))?;
        let objects: Vec<(vectorcraft_engine::doc::NodeId, String)> = {
            let state = self
                .app
                .session
                .doc()
                .map_err(|error| format!("VectorCraft has no document: {error}"))?;
            state
                .doc
                .layers
                .iter()
                .flat_map(|layer| layer.children().into_iter().flatten())
                .map(|node| {
                    (
                        node.id,
                        node.name
                            .clone()
                            .filter(|name| !name.is_empty())
                            .unwrap_or_else(|| format!("Object {}", node.id)),
                    )
                })
                .collect()
        };
        let mut layers = Vec::with_capacity(objects.len());
        for (index, (id, name)) in objects.into_iter().enumerate() {
            // End the name in a letter: EffectCraft's importer would otherwise
            // group the numbered files into an image sequence.
            let path = folder.join(format!("{}.png", crate::layer::file_stem(index, &name)));
            self.app
                .session
                .execute("select.set", &json!({ "ids": [id] }))
                .map_err(|error| format!("VectorCraft could not select {name}: {error}"))?;
            self.app
                .session
                .execute(
                    "document.export",
                    &json!({
                        "format": "png",
                        "path": path.to_string_lossy(),
                        "selectedOnly": true,
                    }),
                )
                .map_err(|error| format!("VectorCraft could not render {name}: {error}"))?;
            layers.push(crate::layer::LayerPng {
                name,
                path,
                position: None,
            });
        }
        Ok(layers)
    }

    /// Start a new document with one layer per supplied image (PhotoCraft's
    /// layers, for example), each placed at its page position.
    pub fn replace_with_layers(
        &mut self,
        width: u32,
        height: u32,
        layers: &[crate::layer::LayerPng],
    ) -> Result<usize, String> {
        let _ = self.app.session.execute("file.closeAll", &json!({}));
        self.app
            .session
            .execute(
                "file.new",
                &json!({ "width": width.max(1), "height": height.max(1) }),
            )
            .map_err(|error| format!("VectorCraft could not start a document: {error}"))?;
        for layer in layers {
            let (iw, ih) = image::image_dimensions(&layer.path)
                .map_err(|error| format!("Could not read {}: {error}", layer.path.display()))?;
            let (cx, cy) = layer
                .position
                .unwrap_or((width as f64 / 2.0, height as f64 / 2.0));
            let id = self
                .app
                .session
                .execute("layer.new", &json!({ "name": layer.name, "top": true }))
                .map_err(|error| format!("VectorCraft could not add a layer: {error}"))?["id"]
                .as_u64()
                .ok_or_else(|| "VectorCraft did not return the layer.".to_string())?;
            let _ = self
                .app
                .session
                .execute("layer.setCurrent", &json!({ "id": id }));
            self.app
                .session
                .execute(
                    "file.place",
                    &json!({
                        "path": layer.path.to_string_lossy(),
                        "link": false,
                        "rect": [
                            cx - iw as f64 / 2.0,
                            cy - ih as f64 / 2.0,
                            iw as f64,
                            ih as f64
                        ],
                    }),
                )
                .map_err(|error| format!("VectorCraft could not place {}: {error}", layer.name))?;
        }
        self.mark_clean();
        Ok(layers.len())
    }

    /// Render the active artboard to a PNG in `folder` and return its path.
    pub fn export_png(&mut self, folder: &Path) -> Result<PathBuf, String> {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("Could not create {}: {error}", folder.display()))?;
        let result = self
            .app
            .session
            .execute(
                "document.exportForScreens",
                &json!({
                    "folder": folder.to_string_lossy(),
                    // Keep the artboard's transparency: the layer is composited
                    // over the design, not flattened onto a black page.
                    "formats": [{ "format": "png", "scale": 1, "background": "transparent" }],
                }),
            )
            .map_err(|error| format!("VectorCraft could not export the artwork: {error}"))?;
        let file = result["files"]
            .as_array()
            .and_then(|files| {
                files
                    .iter()
                    .filter_map(|file| file.as_str())
                    .find(|file| file.to_ascii_lowercase().ends_with(".png"))
            })
            .map(PathBuf::from)
            .ok_or_else(|| {
                "VectorCraft exported no PNG — open or draw a document first.".to_string()
            })?;
        // The export itself marks the document dirty (it remembers its
        // settings); that is our change, not the user's.
        self.mark_clean();
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening the design again must replace the open artwork, not add another
    /// document tab or stop at a "save changes?" prompt.
    #[test]
    fn opening_the_design_again_replaces_the_document() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-vc-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let svg = dir.join("design.svg");
        let design = crate::design::DesignState::new();
        design.export_svg_to(&svg).expect("SVG");

        let mut host = VectorcraftHost::new().expect("VectorCraft starts");
        host.open(&svg).expect("first open");
        assert!(
            !host.has_unsaved_artwork(),
            "a fresh hand-off has no user edits"
        );
        let _ = host.export_png(&dir.join("out")).expect("export");
        assert!(
            !host.has_unsaved_artwork(),
            "exporting is our change, not the user's (it used to look dirty and block the link)"
        );
        let first = host.app.session.documents().len();
        host.open(&svg).expect("second open");
        let second = host.app.session.documents().len();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(first, 1, "one document after the first open");
        assert_eq!(second, 1, "the second open replaced it");
    }

    /// PhotoCraft's layers can open in VectorCraft as one image layer each.
    #[test]
    fn photocraft_layers_open_in_vectorcraft() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-px-to-vc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let png = dir.join("design.png");
        crate::design::DesignState::new()
            .export_png_to(&png)
            .expect("the design renders to a PNG");
        let mut photo = crate::photocraft_host::PhotocraftHost::new().expect("PhotoCraft starts");
        photo.open(&png).expect("PhotoCraft opens the design");
        let layers = photo
            .export_layer_pngs(&dir.join("layers"))
            .expect("PhotoCraft exports its layers");

        let mut vector = VectorcraftHost::new().expect("VectorCraft starts");
        let count = vector
            .replace_with_layers(1200, 800, &layers)
            .expect("the layers open in VectorCraft");
        let doc = vector.app.session.doc().expect("a VectorCraft document");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(count, layers.len());
        assert!(
            doc.doc.layers.len() >= layers.len(),
            "one VectorCraft layer per PhotoCraft layer"
        );
    }

    /// The whole bridge: the shared design becomes SVG, VectorCraft opens it,
    /// and its artwork comes back as a PNG.
    #[test]
    fn the_design_goes_to_vectorcraft_and_comes_back_as_a_png() {
        let dir = std::env::temp_dir().join(format!("creativecraft-vc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let svg = dir.join("design.svg");

        let design = crate::design::DesignState::new();
        design
            .export_svg_to(&svg)
            .expect("the design is written as SVG");
        assert!(std::fs::read_to_string(&svg).unwrap().contains("<svg"));

        let mut host = VectorcraftHost::new().expect("VectorCraft starts");
        host.open(&svg).expect("VectorCraft opens the design");

        let png = host
            .export_png(&dir.join("out"))
            .expect("a PNG is exported");
        let image = image::open(&png).expect("the PNG is readable");
        assert!(
            image.width() > 10 && image.height() > 10,
            "the exported artwork has size: {}x{}",
            image.width(),
            image.height()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
