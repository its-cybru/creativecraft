//! The real FigCraft, hosted in the Photo tab's Layout persona.
//!
//! FigCraft keeps its own document (pages, frames, auto layout, components,
//! styles, prototypes). The shared design reaches it as SVG, and its art comes
//! back as a PNG — the same one-switch hand-off as VectorCraft and PhotoCraft.

use eframe::egui;
use figcraft_ui_egui::FigApp;
use serde_json::json;
use std::path::Path;
use std::sync::Once;

/// Index the installed fonts once, before the first SVG import.
///
/// FigCraft's importer lays text out with usvg first, and usvg silently drops
/// text it cannot lay out. The desktop app scans the system fonts at startup;
/// this host has to do it too, or every text layer in the design is lost on the
/// first hand-off (and then written back missing).
static FONTS: Once = Once::new();

fn ensure_fonts() {
    FONTS.call_once(figcraft_text::scan_system_fonts);
}

/// One hosted design app.
pub struct FigcraftHost {
    app: FigApp,
    /// Undo depth at the last hand-off or export; a deeper history means the
    /// user changed the page, so only then is it written back.
    clean_undo: usize,
}

impl FigcraftHost {
    pub fn new() -> Result<Self, String> {
        let mut session = figcraft_engine::Session::new();
        // CreativeCraft is one dark window. FigCraft's saved preference can be
        // light or "follow system", which left the Layout persona as a bright
        // white page inside it, so this instance opens dark like the other tabs.
        session.prefs.theme = "dark".to_string();
        Ok(Self {
            app: FigApp::new(session, Box::new(figcraft_ui_egui::NoServices)),
            clean_undo: 0,
        })
    }

    /// Whether the page changed since the design was handed over or the result
    /// was written back.
    pub fn has_unsaved_artwork(&self) -> bool {
        self.app
            .session
            .ds()
            .map(|doc| doc.history.undo.len() != self.clean_undo)
            .unwrap_or(false)
    }

    fn mark_clean(&mut self) {
        self.clean_undo = self
            .app
            .session
            .ds()
            .map(|doc| doc.history.undo.len())
            .unwrap_or(0);
    }

    /// FigCraft's interface, drawn inside the shell's Photo area.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.app.ui(ui);
    }

    /// Put FigCraft's interface fonts back if another hosted app replaced them
    /// on the shared egui context (VectorCraft and PhotoCraft install their own
    /// families). Without this, every label in the Layout persona renders as
    /// empty boxes after a round trip through Pixel or Vector.
    pub fn ensure_fonts(&self, ctx: &egui::Context) {
        let installed = ctx.fonts(|fonts| {
            fonts
                .definitions()
                .families
                .contains_key(&egui::FontFamily::Name("brand".into()))
        });
        if !installed {
            figcraft_ui_egui::theme::install_fonts(ctx, true, 0);
        }
    }

    /// Open the design as SVG (FigCraft reads SVG natively).
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        ensure_fonts();
        // Close every tab so a re-open replaces the page instead of adding
        // another document tab (and so a same-path file is really re-read).
        while !self.app.session.docs.is_empty() {
            if self
                .app
                .session
                .execute("file.close", &json!({ "index": 0, "force": true }))
                .is_err()
            {
                break;
            }
        }
        self.app
            .session
            .execute("file.open", &json!({ "path": path.to_string_lossy() }))
            .map_err(|error| format!("FigCraft could not open {}: {error}", path.display()))?;
        self.mark_clean();
        Ok(())
    }

    /// Write the page as SVG with live text, so VectorCraft can edit it.
    pub fn export_svg(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        self.app
            .session
            .execute(
                "file.export",
                &json!({
                    "format": "svg",
                    "path": path.to_string_lossy(),
                    "scale": 1,
                    // Keep text as text; outlined glyphs would no longer be
                    // editable after the trip to VectorCraft.
                    "svgOutlineText": false,
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("FigCraft could not export SVG: {error}"))?;
        self.mark_clean();
        Ok(())
    }

    /// Render every element of the page to its own PNG, in stacking order.
    pub fn export_layer_pngs(
        &mut self,
        folder: &Path,
    ) -> Result<Vec<crate::layer::LayerPng>, String> {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("Could not create {}: {error}", folder.display()))?;
        let layers: Vec<(figcraft_engine::doc::NodeId, String, (f64, f64))> = {
            let doc = self
                .app
                .session
                .doc()
                .map_err(|error| format!("FigCraft has no document: {error}"))?;
            let page = doc
                .pages
                .first()
                .and_then(|id| doc.node(*id))
                .ok_or_else(|| "FigCraft has no page.".to_string())?;
            let frame = page.children.first().and_then(|id| doc.node(*id));
            let children = frame.map(|node| &node.children).unwrap_or(&page.children);
            children
                .iter()
                .filter_map(|id| {
                    doc.node(*id).map(|node| {
                        let name = if node.name.is_empty() {
                            format!("Layer {}", node.id.0)
                        } else {
                            node.name.clone()
                        };
                        (node.id, name, page_center(doc, node))
                    })
                })
                .collect()
        };
        let mut exported = Vec::with_capacity(layers.len());
        for (index, (id, name, position)) in layers.into_iter().enumerate() {
            let path = folder.join(format!("{}.png", crate::layer::file_stem(index, &name)));
            self.app
                .session
                .execute(
                    "file.export",
                    &json!({
                        "ids": [id],
                        "format": "png",
                        "path": path.to_string_lossy(),
                        "scale": 1,
                    }),
                )
                .map_err(|error| format!("FigCraft could not render {name}: {error}"))?;
            exported.push(crate::layer::LayerPng {
                name,
                path,
                position: Some(position),
            });
        }
        Ok(exported)
    }

    /// Start a new page with one image layer per supplied PNG, at its position.
    pub fn replace_with_layers(
        &mut self,
        width: u32,
        height: u32,
        layers: &[crate::layer::LayerPng],
    ) -> Result<usize, String> {
        ensure_fonts();
        while !self.app.session.docs.is_empty() {
            if self
                .app
                .session
                .execute("file.close", &json!({ "index": 0, "force": true }))
                .is_err()
            {
                break;
            }
        }
        self.app
            .session
            .execute("file.new", &json!({ "name": "Image layers" }))
            .map_err(|error| format!("FigCraft could not start a page: {error}"))?;
        let frame = self
            .app
            .session
            .execute(
                "frame.create",
                &json!({
                    "x": 0.0,
                    "y": 0.0,
                    "width": width.max(1),
                    "height": height.max(1),
                    "name": "Image",
                }),
            )
            .map_err(|error| format!("FigCraft could not make the frame: {error}"))?["id"]
            .as_u64()
            .ok_or_else(|| "FigCraft did not return the frame.".to_string())?;
        for layer in layers {
            let (iw, ih) = image::image_dimensions(&layer.path)
                .map_err(|error| format!("Could not read {}: {error}", layer.path.display()))?;
            let (cx, cy) = layer
                .position
                .unwrap_or((width as f64 / 2.0, height as f64 / 2.0));
            self.app
                .session
                .execute(
                    "file.placeImage",
                    &json!({
                        "path": layer.path.to_string_lossy(),
                        "x": cx - iw as f64 / 2.0,
                        "y": cy - ih as f64 / 2.0,
                        "width": iw,
                        "height": ih,
                        "parent": frame,
                        "name": layer.name,
                    }),
                )
                .map_err(|error| format!("FigCraft could not place {}: {error}", layer.name))?;
        }
        self.mark_clean();
        Ok(layers.len())
    }

    /// Render the current page to a PNG for the design and the timeline.
    pub fn export_png(&mut self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
        }
        self.app
            .session
            .execute(
                "file.export",
                &json!({
                    "format": "png",
                    "path": path.to_string_lossy(),
                    "scale": 1,
                }),
            )
            .map(|_| ())
            .map_err(|error| format!("FigCraft could not export the page: {error}"))?;
        self.mark_clean();
        Ok(())
    }
}

/// The centre of `node` in page pixels: its local box centre through every
/// ancestor transform. A cropped FigCraft export lands there in a composition.
fn page_center(
    doc: &figcraft_engine::doc::Document,
    node: &figcraft_engine::doc::Node,
) -> (f64, f64) {
    let mut xf = figcraft_engine::doc::geom::Affine::IDENTITY;
    let mut current = Some(node.id);
    while let Some(id) = current {
        let Some(n) = doc.node(id) else { break };
        xf = n.transform * xf;
        current = n.parent;
    }
    let center = xf * figcraft_engine::doc::geom::Point::new(node.width / 2.0, node.height / 2.0);
    (center.x, center.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The design's text must survive the SVG hand-off: FigCraft is the last
    /// link, and a missing text layer here would also delete it from the shared
    /// design when the page comes back.
    #[test]
    fn design_text_reaches_figcraft() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-fig-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let svg = dir.join("design.svg");
        crate::design::DesignState::sample()
            .export_svg_to(&svg)
            .expect("SVG");
        let mut host = FigcraftHost::new().expect("FigCraft starts");
        host.open(&svg).expect("FigCraft opens the design");

        let doc = host.app.session.doc().expect("a document is open");
        let texts: Vec<String> = doc
            .nodes()
            .filter_map(|node| match &node.kind {
                figcraft_engine::doc::NodeKind::Text(text) => Some(text.characters.clone()),
                _ => None,
            })
            .collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            texts.iter().any(|text| text == "Pro plan"),
            "the design's text is a text layer in FigCraft: {texts:?}"
        );
    }

    /// VectorCraft's SVG export must open in FigCraft with its text intact: this
    /// is the file the two apps now exchange instead of a flattened image.
    #[test]
    fn vectorcraft_svg_opens_in_figcraft_with_text() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-vc-to-fig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let seed = dir.join("design.svg");
        crate::design::DesignState::sample()
            .export_svg_to(&seed)
            .expect("SVG");

        let mut vector =
            crate::vectorcraft_host::VectorcraftHost::new().expect("VectorCraft starts");
        vector.open(&seed).expect("VectorCraft opens the design");
        let from_vector = dir.join("artwork.svg");
        vector
            .export_svg(&from_vector)
            .expect("VectorCraft writes SVG");

        let mut fig = FigcraftHost::new().expect("FigCraft starts");
        fig.open(&from_vector)
            .expect("FigCraft opens VectorCraft's SVG");
        let doc = fig.app.session.doc().expect("a document is open");
        let texts: Vec<String> = doc
            .nodes()
            .filter_map(|node| match &node.kind {
                figcraft_engine::doc::NodeKind::Text(text) => Some(text.characters.clone()),
                _ => None,
            })
            .collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            texts.iter().any(|text| text == "Pro plan"),
            "VectorCraft's text survived: {texts:?}"
        );
    }

    /// The reverse direction: FigCraft's SVG export must open and render in
    /// VectorCraft, so an edit made in Layout is editable in Vector.
    #[test]
    fn figcraft_svg_opens_in_vectorcraft() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-fig-to-vc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let seed = dir.join("design.svg");
        crate::design::DesignState::sample()
            .export_svg_to(&seed)
            .expect("SVG");

        let mut fig = FigcraftHost::new().expect("FigCraft starts");
        fig.open(&seed).expect("FigCraft opens the design");
        let from_fig = dir.join("artwork.svg");
        fig.export_svg(&from_fig).expect("FigCraft writes SVG");

        let mut vector =
            crate::vectorcraft_host::VectorcraftHost::new().expect("VectorCraft starts");
        vector
            .open(&from_fig)
            .expect("VectorCraft opens FigCraft's SVG");
        let png = vector
            .export_png(&dir.join("out"))
            .expect("VectorCraft renders it");
        let image = image::open(&png).expect("the render is readable");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            image.width() > 100 && image.height() > 100,
            "FigCraft's page rendered in VectorCraft: {}x{}",
            image.width(),
            image.height()
        );
    }

    /// Per-layer exports must line up in the composition: VectorCraft renders
    /// the whole artboard per object, while FigCraft crops each object and
    /// reports where it sits on the page.
    #[test]
    fn layer_exports_carry_the_page_geometry() {
        let dir =
            std::env::temp_dir().join(format!("creativecraft-layer-png-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("the test folder is created");
        let seed = dir.join("design.svg");
        crate::design::DesignState::sample()
            .export_svg_to(&seed)
            .expect("SVG");

        let mut vector =
            crate::vectorcraft_host::VectorcraftHost::new().expect("VectorCraft starts");
        vector.open(&seed).expect("open");
        let layers = vector
            .export_layer_pngs(&dir.join("vector"))
            .expect("VectorCraft exports layers");
        assert!(layers.len() >= 4, "the design has several objects");
        for layer in &layers {
            let img = image::open(&layer.path).expect("readable");
            assert_eq!(
                (img.width(), img.height()),
                (1200, 800),
                "{} is a full artboard",
                layer.name
            );
            assert!(
                layer.position.is_none(),
                "a full-frame image needs no centre"
            );
        }

        let mut fig = FigcraftHost::new().expect("FigCraft starts");
        fig.open(&seed).expect("open");
        let layers = fig
            .export_layer_pngs(&dir.join("fig"))
            .expect("FigCraft exports layers");
        assert!(layers.len() >= 4, "the page has several elements");
        assert!(
            layers.iter().any(|layer| layer.name == "Pro plan"),
            "named elements survive"
        );
        for layer in &layers {
            let img = image::open(&layer.path).expect("readable");
            assert!(img.width() >= 1 && img.height() >= 1);
            let (x, y) = layer
                .position
                .expect("a cropped image carries its page centre");
            assert!(
                (0.0..=1200.0).contains(&x) && (0.0..=800.0).contains(&y),
                "{} lands on the page at {x},{y}",
                layer.name
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
