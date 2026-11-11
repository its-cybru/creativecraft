//! The real PhotoCraft, hosted in the Photo tab's Pixel persona.
//!
//! PhotoCraft keeps its own document (a Photoshop-style stack: layers, masks,
//! adjustments, text, smart objects). It is linked to the shared design the same
//! way VectorCraft is: the design is handed over as a flattened image, and the
//! edited result comes back into the design as its pixel layer.
//!
//! Its file codecs live behind host-supplied services (`photocraft-io`), so the
//! host provides Import/Export/Write here.

use eframe::App as _;
use eframe::egui::{self, FontFamily};
use photocraft_ui_egui::PhotocraftApp;
use serde_json::json;
use std::path::Path;

/// One hosted editor.
pub struct PhotocraftHost {
    app: PhotocraftApp,
    /// PhotoCraft's document revision at the last hand-off or export. A moved
    /// revision means the user actually edited the image, so leaving only then
    /// writes it back (and switching without edits stays instant).
    clean_revision: u64,
}

impl PhotocraftHost {
    pub fn new() -> Result<Self, String> {
        let mut services = photocraft_ui_egui::Services::default();
        services.import = Some(Box::new(|name, bytes, options| {
            photocraft_io::import_with_options(name, bytes, options)
                .map(|result| (result.document, result.warnings))
                .map_err(|error| error.to_string())
        }));
        services.export = Some(Box::new(|document, name, _settings| {
            photocraft_io::export(document, name, &photocraft_io::ExportOptions::default())
                .map(|result| (result.bytes, result.warnings))
                .map_err(|error| error.to_string())
        }));
        services.write = Some(Box::new(|path: &str, bytes: &[u8]| {
            std::fs::write(path, bytes).map_err(|error| error.to_string())
        }));
        let app = PhotocraftApp::new(photocraft_engine::Session::new(), services);
        // PhotoCraft's default is Studio (Dark), the near-black theme with the violet accent,
        // so the hosted Pixel persona and the rest of the studio share one dark chrome. A theme
        // picked in Preferences > Interface still wins for that session.
        Ok(Self {
            app,
            clean_revision: 0,
        })
    }

    /// Whether the image changed since the design was handed over or the result
    /// was written back.
    pub fn has_unsaved_artwork(&self) -> bool {
        self.app
            .session
            .active()
            .is_some_and(|doc| doc.revision != self.clean_revision)
    }

    fn mark_clean(&mut self) {
        self.clean_revision = self
            .app
            .session
            .active()
            .map(|doc| doc.revision)
            .unwrap_or(0);
    }

    /// PhotoCraft's interface, drawn inside the shell's Photo area.
    pub fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }

    /// PhotoCraft's logic pass: canvases, GPU work, dialogs and history.
    pub fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.ensure_fonts(ctx);
        self.app.logic(ctx, frame);
    }

    /// Keep PhotoCraft's interface families installed.
    ///
    /// VectorCraft (and FigCraft) replace egui's shared font definitions, which
    /// drops the `medium`/`semibold` families PhotoCraft labels use — its tab
    /// and layer names then draw as empty boxes. The check is a map lookup.
    fn ensure_fonts(&mut self, ctx: &egui::Context) {
        let installed = ctx.fonts(|fonts| {
            let families = &fonts.definitions().families;
            families.contains_key(&FontFamily::Name("medium".into()))
                && families.contains_key(&FontFamily::Name("semibold".into()))
        });
        if !installed {
            photocraft_ui_egui::theme::install_fonts(ctx);
        }
    }

    /// Open an image (the design's PNG) as a new PhotoCraft document.
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let _ = self.app.session.execute("file.closeAll", json!({}));
        let bytes = std::fs::read(path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("design.png");
        let imported = photocraft_io::import_with_options(
            name,
            &bytes,
            &photocraft_io::ImportOptions::default(),
        )
        .map_err(|error| format!("PhotoCraft could not read {}: {error}", path.display()))?;
        self.app
            .session
            .open_document(imported.document, Some(path.to_string_lossy().to_string()));
        self.mark_clean();
        Ok(())
    }

    /// Render every top-level layer to its own PNG, with its page centre.
    ///
    /// PhotoCraft's layer stack then arrives in EffectCraft as separate
    /// composition layers, the same way VectorCraft/FigCraft elements do.
    pub fn export_layer_pngs(
        &mut self,
        folder: &Path,
    ) -> Result<Vec<crate::layer::LayerPng>, String> {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("Could not create {}: {error}", folder.display()))?;
        let list = self
            .app
            .session
            .execute("layer.list", json!({ "flat": true }))
            .map_err(|error| format!("PhotoCraft could not list its layers: {error}"))?;
        let rows = list["layers"]
            .as_array()
            .ok_or_else(|| "PhotoCraft returned no layer list.".to_string())?;
        let mut layers = Vec::new();
        for row in rows {
            if row["depth"].as_u64() != Some(0) {
                continue;
            }
            if !row["visible"].as_bool().unwrap_or(false) {
                continue;
            }
            let Some(bounds) = row["bounds"].as_array() else {
                continue;
            };
            let value = |index: usize| bounds.get(index).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let (x, y, w, h) = (value(0), value(1), value(2), value(3));
            if w < 1.0 || h < 1.0 {
                continue;
            }
            let Some(id) = row["id"].as_u64() else {
                continue;
            };
            let name = row["name"].as_str().unwrap_or("Layer").to_string();
            let path = folder.join(format!(
                "{}.png",
                crate::layer::file_stem(layers.len(), &name)
            ));
            self.app
                .session
                .execute(
                    "layer.exportAs",
                    json!({ "layer": id, "path": path.to_string_lossy() }),
                )
                .map_err(|error| format!("PhotoCraft could not render {name}: {error}"))?;
            layers.push(crate::layer::LayerPng {
                name,
                path,
                position: Some((x + w / 2.0, y + h / 2.0)),
            });
        }
        Ok(layers)
    }

    /// Start a new document with one layer per supplied PNG, at its position.
    pub fn replace_with_layers(
        &mut self,
        width: u32,
        height: u32,
        layers: &[crate::layer::LayerPng],
    ) -> Result<usize, String> {
        let _ = self.app.session.execute("file.closeAll", json!({}));
        self.app
            .session
            .execute(
                "file.new",
                json!({
                    "width": width.max(1),
                    "height": height.max(1),
                    "background": "transparent",
                }),
            )
            .map_err(|error| format!("PhotoCraft could not start a document: {error}"))?;
        for layer in layers {
            let (cx, cy) = layer
                .position
                .unwrap_or((width as f64 / 2.0, height as f64 / 2.0));
            self.app
                .session
                .execute(
                    "file.placeEmbedded",
                    json!({
                        "path": layer.path.to_string_lossy(),
                        "center": [cx, cy],
                        "scale": 100.0,
                        "fit": false,
                        "smartObject": false,
                    }),
                )
                .map_err(|error| format!("PhotoCraft could not place {}: {error}", layer.name))?;
        }
        self.mark_clean();
        Ok(layers.len())
    }

    /// Save the document as a PNG at `path` (for the design layer / timeline).
    pub fn export_png(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        let bytes = {
            let state = self
                .app
                .session
                .active()
                .ok_or_else(|| "PhotoCraft has no open document.".to_string())?;
            photocraft_io::export(
                &state.doc,
                "design.png",
                &photocraft_io::ExportOptions::default(),
            )
            .map_err(|error| format!("PhotoCraft could not export the image: {error}"))?
            .bytes
        };
        std::fs::write(path, bytes)
            .map_err(|error| format!("Could not save {}: {error}", path.display()))?;
        self.mark_clean();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VectorCraft's layers can open in PhotoCraft as one image layer each.
    #[test]
    fn vector_layers_open_in_photocraft() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-vc-to-px-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let seed = dir.join("design.svg");
        crate::design::DesignState::sample()
            .export_svg_to(&seed)
            .expect("SVG");
        let mut vector =
            crate::vectorcraft_host::VectorcraftHost::new().expect("VectorCraft starts");
        vector.open(&seed).expect("VectorCraft opens the design");
        let layers = vector
            .export_layer_pngs(&dir.join("layers"))
            .expect("VectorCraft exports its layers");

        let mut photo = PhotocraftHost::new().expect("PhotoCraft starts");
        let count = photo
            .replace_with_layers(1200, 800, &layers)
            .expect("the layers open in PhotoCraft");
        let doc = photo.app.session.active().expect("a PhotoCraft document");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(count, layers.len());
        assert!(
            doc.doc.layers.len() >= layers.len(),
            "one PhotoCraft layer per VectorCraft layer"
        );
    }

    /// PhotoCraft's layers must reach EffectCraft as separate layers.
    #[test]
    fn photocraft_layers_become_effectcraft_layers() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-pixel-layers-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let png = dir.join("design.png");
        crate::design::DesignState::sample()
            .export_png_to(&png)
            .expect("the design renders to a PNG");

        let mut host = PhotocraftHost::new().expect("PhotoCraft starts");
        host.open(&png).expect("PhotoCraft opens the design");
        let layers = host
            .export_layer_pngs(&dir.join("layers"))
            .expect("PhotoCraft exports its layers");
        assert!(!layers.is_empty(), "the image has at least one layer");
        for layer in &layers {
            let img = image::open(&layer.path).expect("the layer PNG is readable");
            assert!(img.width() > 10 && img.height() > 10);
            let (x, y) = layer.position.expect("a layer carries its page centre");
            assert!((0.0..=1200.0).contains(&x) && (0.0..=800.0).contains(&y));
        }

        let mut fx = crate::effectcraft_host::EffectcraftHost::new().expect("EffectCraft starts");
        let (count, comp) = fx
            .animate_layers("PhotoCraft image", 1200, 800, &layers)
            .expect("the composition is built");
        fx.open_comp(comp).expect("the comp opens");
        let built = fx.active_comp_layers().expect("the comp is active");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(count, layers.len());
        assert_eq!(built, layers.len(), "one comp layer per layer");
    }
}
