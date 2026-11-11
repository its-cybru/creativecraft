//! The shared design document: one file, one set of objects, three editing views.
//!
//! This is deliberately small. It holds the objects a promo/pricing card needs:
//! frames, rectangles, text, a photo and a paintable raster layer. The three
//! personas (Layout, Vector, Pixel) all read and write this same document.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub type NodeId = u64;

// ── Colour ──────────────────────────────────────────────────────────────────

/// Straight (non-premultiplied) sRGB, stored as bytes so the document is
/// plain data and serialises without depending on egui's types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const WHITE: Self = Self([255, 255, 255, 255]);
    pub const INK: Self = Self([28, 28, 34, 255]);
    pub const MUTED: Self = Self([110, 110, 124, 255]);
    pub const ACCENT: Self = Self([139, 92, 246, 255]);
    pub const CARD: Self = Self([255, 255, 255, 255]);
    pub const CANVAS: Self = Self([18, 18, 24, 255]);

    pub fn color32(self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(self.0[0], self.0[1], self.0[2], self.0[3])
    }

    pub fn with_alpha(self, alpha: f32) -> egui::Color32 {
        let a = (self.0[3] as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
        egui::Color32::from_rgba_unmultiplied(self.0[0], self.0[1], self.0[2], a)
    }

    pub fn from_color32(c: egui::Color32) -> Self {
        let [r, g, b, a] = c.to_srgba_unmultiplied();
        Self([r, g, b, a])
    }
}

// ── Geometry ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    pub fn contains(self, p: egui::Pos2) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
}

// ── Document ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PhotoSource {
    /// A generated placeholder portrait, so the demo works with no files.
    Sample,
    /// A file on disk, relative to the project assets folder or absolute.
    File(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Kind {
    Frame {
        fill: Rgba,
    },
    Rect {
        fill: Rgba,
        radius: f32,
        stroke: f32,
        stroke_color: Rgba,
    },
    Text {
        text: String,
        size: f32,
        color: Rgba,
    },
    Photo {
        source: PhotoSource,
        px: (u32, u32),
    },
    /// A transparent raster layer the Pixel persona can paint on.
    Paint {
        px: (u32, u32),
    },
}

impl Kind {
    pub fn type_name(&self) -> &'static str {
        match self {
            Kind::Frame { .. } => "Frame",
            Kind::Rect { .. } => "Shape",
            Kind::Text { .. } => "Text",
            Kind::Photo { .. } => "Photo",
            Kind::Paint { .. } => "Paint",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub rect: Rect,
    pub visible: bool,
    pub opacity: f32,
    pub kind: Kind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub canvas_w: f32,
    pub canvas_h: f32,
    /// Paint order: index 0 is the bottom layer.
    pub nodes: Vec<Node>,
    pub next_id: NodeId,
    #[serde(skip)]
    pub file: Option<PathBuf>,
    #[serde(skip)]
    pub status: String,
}

impl Document {
    pub fn new(title: impl Into<String>, canvas_w: f32, canvas_h: f32) -> Self {
        Self {
            title: title.into(),
            canvas_w,
            canvas_h,
            nodes: Vec::new(),
            next_id: 1,
            file: None,
            status: String::new(),
        }
    }

    pub fn add(&mut self, name: impl Into<String>, kind: Kind, rect: Rect) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(Node {
            id,
            name: name.into(),
            rect,
            visible: true,
            opacity: 1.0,
            kind,
        });
        id
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn remove(&mut self, id: NodeId) {
        self.nodes.retain(|n| n.id != id);
    }

    /// Topmost node containing the point, ignoring hidden nodes.
    pub fn hit(&self, p: egui::Pos2) -> Option<NodeId> {
        self.nodes
            .iter()
            .rev()
            .find(|n| n.visible && n.rect.contains(p))
            .map(|n| n.id)
    }

    /// The empty document a new session starts with: one 1200x800 canvas,
    /// no layers. The user adds the first shape; nothing is pre-filled.
    pub fn blank() -> Self {
        Self::new("Untitled", 1200.0, 800.0)
    }

    /// The promo card the demo used to start with. Kept for tests and for
    /// anyone reopening a saved sample; the app no longer opens on it.
    pub fn sample() -> Self {
        let mut doc = Self::new("Pricing card", 1200.0, 800.0);

        doc.add(
            "Background",
            Kind::Frame { fill: Rgba::CANVAS },
            Rect::new(0.0, 0.0, 1200.0, 800.0),
        );

        doc.add(
            "Portrait",
            Kind::Photo {
                source: PhotoSource::Sample,
                px: (480, 640),
            },
            Rect::new(80.0, 80.0, 480.0, 640.0),
        );

        doc.add(
            "Card",
            Kind::Rect {
                fill: Rgba::CARD,
                radius: 28.0,
                stroke: 0.0,
                stroke_color: Rgba::WHITE,
            },
            Rect::new(640.0, 120.0, 480.0, 560.0),
        );

        doc.add(
            "Plan",
            Kind::Text {
                text: "Pro plan".into(),
                size: 34.0,
                color: Rgba::MUTED,
            },
            Rect::new(690.0, 210.0, 380.0, 50.0),
        );

        doc.add(
            "Price",
            Kind::Text {
                text: "$49".into(),
                size: 104.0,
                color: Rgba::INK,
            },
            Rect::new(690.0, 260.0, 380.0, 120.0),
        );

        doc.add(
            "Tagline",
            Kind::Text {
                text: "Everything you need to ship.".into(),
                size: 24.0,
                color: Rgba::MUTED,
            },
            Rect::new(690.0, 400.0, 390.0, 40.0),
        );

        doc.add(
            "Button",
            Kind::Rect {
                fill: Rgba::ACCENT,
                radius: 18.0,
                stroke: 0.0,
                stroke_color: Rgba::ACCENT,
            },
            Rect::new(690.0, 560.0, 250.0, 64.0),
        );

        doc.add(
            "Button label",
            Kind::Text {
                text: "Choose plan".into(),
                size: 24.0,
                color: Rgba::WHITE,
            },
            Rect::new(722.0, 577.0, 220.0, 34.0),
        );

        doc.add(
            "Paint layer",
            Kind::Paint { px: (600, 400) },
            Rect::new(0.0, 0.0, 1200.0, 800.0),
        );

        doc
    }

    pub fn project_path() -> PathBuf {
        project_root().join("creativecraft.ccproj")
    }

    pub fn assets_dir() -> PathBuf {
        project_root().join("creativecraft.assets")
    }

    pub fn save(&mut self, rasters: &Rasters) -> Result<(), String> {
        let path = Self::project_path();
        let assets = Self::assets_dir();
        std::fs::create_dir_all(&assets)
            .map_err(|e| format!("Could not create assets folder: {e}"))?;

        for node in &self.nodes {
            if let Some(raster) = rasters.map.get(&node.id) {
                let file = assets.join(format!("{}.png", node.id));
                raster.save_png(&file)?;
            }
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Could not encode project: {e}"))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
        self.file = Some(path);
        self.status = "Saved creativecraft.ccproj".into();
        Ok(())
    }

    pub fn load(rasters: &mut Rasters) -> Result<Self, String> {
        let path = Self::project_path();
        let json = std::fs::read_to_string(&path)
            .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        let mut doc: Self =
            serde_json::from_str(&json).map_err(|e| format!("Could not decode project: {e}"))?;
        doc.file = Some(path);
        doc.status = "Opened creativecraft.ccproj".into();

        rasters.clear();
        for node in &doc.nodes {
            match &node.kind {
                Kind::Photo { px, source } => {
                    let file = Self::assets_dir().join(format!("{}.png", node.id));
                    let raster = if file.exists() {
                        Raster::load_image(&file)
                            .unwrap_or_else(|_| Raster::sample_photo(px.0, px.1))
                    } else {
                        match source {
                            PhotoSource::Sample => Raster::sample_photo(px.0, px.1),
                            PhotoSource::File(p) => Raster::load_image(std::path::Path::new(p))
                                .unwrap_or_else(|_| Raster::sample_photo(px.0, px.1)),
                        }
                    };
                    rasters.map.insert(node.id, raster);
                }
                Kind::Paint { px } => {
                    let file = Self::assets_dir().join(format!("{}.png", node.id));
                    let raster = if file.exists() {
                        Raster::load_image(&file)
                            .unwrap_or_else(|_| Raster::transparent(px.0, px.1))
                    } else {
                        Raster::transparent(px.0, px.1)
                    };
                    rasters.map.insert(node.id, raster);
                }
                _ => {}
            }
        }
        Ok(doc)
    }
}

fn project_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

// ── Undo / redo ─────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
}

impl History {
    const LIMIT: usize = 64;

    pub fn checkpoint(&mut self, doc: &Document) {
        self.undo.push(doc.clone());
        if self.undo.len() > Self::LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(doc.clone());
        let status = doc.status.clone();
        *doc = previous;
        doc.status = status;
        true
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(doc.clone());
        let status = doc.status.clone();
        *doc = next;
        doc.status = status;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

// ── Raster layers ───────────────────────────────────────────────────────────

pub struct Raster {
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<u8>,
    texture: Option<egui::TextureHandle>,
    dirty: bool,
}

impl Raster {
    pub fn transparent(w: u32, h: u32) -> Self {
        let (w, h) = (w.max(1) as usize, h.max(1) as usize);
        Self {
            w,
            h,
            pixels: vec![0; w * h * 4],
            texture: None,
            dirty: true,
        }
    }

    /// A generated portrait: gradient, head, shoulders. A stand-in for a photo.
    pub fn sample_photo(w: u32, h: u32) -> Self {
        let (w, h) = (w.max(1) as usize, h.max(1) as usize);
        let mut pixels = vec![0u8; w * h * 4];
        let horizon = h as f32 * 0.62;

        for y in 0..h {
            for x in 0..w {
                let fx = x as f32 / w as f32;
                let fy = y as f32 / h as f32;
                let mut rgb = [
                    (40.0 + fy * 28.0 + fx * 22.0) as u8,
                    (30.0 + fy * 20.0 + fx * 10.0) as u8,
                    (52.0 + fy * 40.0 + fx * 46.0) as u8,
                ];

                // Head.
                let hx = w as f32 * 0.5;
                let hy = h as f32 * 0.42;
                let hr = h as f32 * 0.19;
                let d = ((x as f32 - hx).powi(2) + (y as f32 - hy).powi(2)).sqrt();
                if d < hr {
                    let light = 1.0 - (d / hr) * 0.25;
                    rgb = [
                        (226.0 * light) as u8,
                        (196.0 * light) as u8,
                        (174.0 * light) as u8,
                    ];
                }

                // Shoulders.
                if y as f32 > horizon {
                    let half = w as f32 * 0.30;
                    let shoulder_y = horizon - (y as f32 - horizon) * 0.12;
                    if (x as f32 - w as f32 * 0.5).abs() < half + shoulder_y * 0.08 {
                        rgb = [88, 78, 116];
                    }
                }

                let i = (y * w + x) * 4;
                pixels[i] = rgb[0];
                pixels[i + 1] = rgb[1];
                pixels[i + 2] = rgb[2];
                pixels[i + 3] = 255;
            }
        }

        Self {
            w,
            h,
            pixels,
            texture: None,
            dirty: true,
        }
    }

    pub fn save_png(&self, path: &std::path::Path) -> Result<(), String> {
        image::save_buffer(
            path,
            &self.pixels,
            self.w as u32,
            self.h as u32,
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("Could not save {}: {e}", path.display()))
    }

    /// Load any image format enabled in `Cargo.toml` (PNG, JPEG, WebP).
    pub fn load_image(path: &std::path::Path) -> Result<Self, String> {
        let img = image::open(path)
            .map_err(|e| format!("Could not open {}: {e}", path.display()))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        Ok(Self {
            w: w as usize,
            h: h as usize,
            pixels: img.into_raw(),
            texture: None,
            dirty: true,
        })
    }

    /// Shrink an imported image so a phone photo does not eat hundreds of
    /// megabytes of GPU memory. Aspect ratio is preserved.
    pub fn downscale(mut self, max_dim: u32) -> Self {
        let longest = self.w.max(self.h);
        if longest as u32 <= max_dim || max_dim == 0 {
            return self;
        }
        let scale = max_dim as f32 / longest as f32;
        let new_w = ((self.w as f32 * scale).round() as u32).max(1);
        let new_h = ((self.h as f32 * scale).round() as u32).max(1);

        let expected = self.w * self.h * 4;
        if self.pixels.len() != expected {
            return self;
        }
        let buffer = std::mem::take(&mut self.pixels);
        let Some(img) = image::RgbaImage::from_raw(self.w as u32, self.h as u32, buffer) else {
            self.pixels = vec![0; expected];
            return self;
        };
        let resized =
            image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle);
        Self {
            w: new_w as usize,
            h: new_h as usize,
            pixels: resized.into_raw(),
            texture: None,
            dirty: true,
        }
    }

    pub fn texture(&mut self, ctx: &egui::Context, name: &str) -> egui::TextureId {
        if self.texture.is_none() || self.dirty {
            let image = egui::ColorImage::from_rgba_unmultiplied([self.w, self.h], &self.pixels);
            self.texture = Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
            self.dirty = false;
        }
        self.texture
            .as_ref()
            .map(|t| t.id())
            .unwrap_or_else(|| egui::TextureId::default())
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
        self.dirty = true;
    }

    pub fn paint(&mut self, cx: f32, cy: f32, radius: f32, color: Rgba, erase: bool) {
        if self.w == 0 || self.h == 0 || radius <= 0.0 {
            return;
        }
        let min_x = (cx - radius).floor().max(0.0) as usize;
        let max_x = (cx + radius).ceil().min(self.w as f32) as usize;
        let min_y = (cy - radius).floor().max(0.0) as usize;
        let max_y = (cy + radius).ceil().min(self.h as f32) as usize;

        for y in min_y..max_y {
            for x in min_x..max_x {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                if d > radius {
                    continue;
                }
                let coverage = ((radius - d) / 1.5).clamp(0.0, 1.0);
                let i = (y * self.w + x) * 4;
                if erase {
                    self.pixels[i + 3] =
                        (self.pixels[i + 3] as f32 * (1.0 - coverage)).round() as u8;
                } else {
                    let sa = coverage * (color.0[3] as f32 / 255.0);
                    let da = self.pixels[i + 3] as f32 / 255.0;
                    let out_a = sa + da * (1.0 - sa);
                    if out_a > 0.0 {
                        for c in 0..3 {
                            let src = color.0[c] as f32;
                            let dst = self.pixels[i + c] as f32;
                            self.pixels[i + c] =
                                ((src * sa + dst * da * (1.0 - sa)) / out_a).round() as u8;
                        }
                        self.pixels[i + 3] = (out_a * 255.0).round() as u8;
                    }
                }
            }
        }
        self.dirty = true;
    }
}

#[derive(Default)]
pub struct Rasters {
    pub map: HashMap<NodeId, Raster>,
}

impl Rasters {
    /// Create missing rasters and drop those whose node is gone.
    pub fn sync(&mut self, doc: &Document) {
        let live: Vec<NodeId> = doc
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, Kind::Photo { .. } | Kind::Paint { .. }))
            .map(|n| n.id)
            .collect();
        self.map.retain(|id, _| live.contains(id));

        for node in &doc.nodes {
            if self.map.contains_key(&node.id) {
                continue;
            }
            match &node.kind {
                Kind::Photo { source, px } => {
                    let raster = match source {
                        PhotoSource::Sample => Raster::sample_photo(px.0, px.1),
                        PhotoSource::File(path) => Raster::load_image(std::path::Path::new(path))
                            .unwrap_or_else(|_| Raster::sample_photo(px.0, px.1)),
                    };
                    self.map.insert(node.id, raster);
                }
                Kind::Paint { px } => {
                    self.map.insert(node.id, Raster::transparent(px.0, px.1));
                }
                _ => {}
            }
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }
}
