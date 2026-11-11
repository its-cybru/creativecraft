//! Flatten the design document to a real PNG on the CPU.
//!
//! The canvas draws through egui, but an export must not depend on the window
//! size, the current zoom or the screen's pixels-per-point. So we redraw the
//! same document here at its native size: 1200 × 800 becomes a 1200 × 800 PNG.
//!
//! To keep edges smooth we render at 2× (2400 × 1600) and downsample with a
//! Lanczos filter. Everything is composited in premultiplied alpha, then
//! converted back to the straight alpha PNG expects.

use fontdue::Font;
use image::{RgbaImage, imageops::FilterType};

use super::model::{Document, Kind, Raster, Rasters, Rect};

/// How many render pixels per document pixel. 2 means one extra downsample.
const SUPERSAMPLE: u32 = 2;

/// Render every visible node into an opaque-or-transparent RGBA image that is
/// exactly `canvas_w × canvas_h` document pixels.
pub fn render_png(doc: &Document, rasters: &Rasters) -> Result<RgbaImage, String> {
    let out_w = doc.canvas_w.round().max(1.0) as u32;
    let out_h = doc.canvas_h.round().max(1.0) as u32;
    let mut buffer = Buffer::new(out_w * SUPERSAMPLE, out_h * SUPERSAMPLE);

    let font = Font::from_bytes(
        epaint_default_fonts::UBUNTU_LIGHT,
        fontdue::FontSettings::default(),
    )
    .map_err(|error| format!("Could not load the export font: {error}"))?;

    for node in &doc.nodes {
        if !node.visible || node.opacity <= 0.0 {
            continue;
        }
        let opacity = node.opacity.clamp(0.0, 1.0);
        let rect = scale_rect(node.rect, SUPERSAMPLE as f32);

        match &node.kind {
            Kind::Frame { fill } => {
                buffer.fill_rect(rect, *fill, opacity);
            }
            Kind::Rect {
                fill,
                radius,
                stroke,
                stroke_color,
            } => {
                buffer.fill_round_rect(rect, *radius * SUPERSAMPLE as f32, *fill, opacity);
                if *stroke > 0.0 {
                    buffer.stroke_round_rect(
                        rect,
                        *radius * SUPERSAMPLE as f32,
                        *stroke * SUPERSAMPLE as f32,
                        *stroke_color,
                        opacity,
                    );
                }
            }
            Kind::Text { text, size, color } => {
                draw_text(
                    &mut buffer,
                    &font,
                    text,
                    *size * SUPERSAMPLE as f32,
                    rect,
                    *color,
                    opacity,
                );
            }
            Kind::Photo { .. } | Kind::Paint { .. } => {
                if let Some(raster) = rasters.map.get(&node.id) {
                    buffer.draw_raster(rect, raster, opacity);
                }
            }
        }
    }

    Ok(buffer.downsample(out_w, out_h))
}

// ── Pixel buffer ────────────────────────────────────────────────────────────

struct Buffer {
    w: usize,
    h: usize,
    /// Premultiplied sRGB, row-major RGBA.
    pixels: Vec<u8>,
}

impl Buffer {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w: w.max(1) as usize,
            h: h.max(1) as usize,
            pixels: vec![0; w as usize * h as usize * 4],
        }
    }

    fn blend(&mut self, x: usize, y: usize, color: [u8; 3], alpha: f32) {
        if alpha <= 0.0 || x >= self.w || y >= self.h {
            return;
        }
        let alpha = alpha.clamp(0.0, 1.0);
        let inv = 1.0 - alpha;
        let i = (y * self.w + x) * 4;
        for c in 0..3 {
            let source = color[c] as f32 * alpha;
            let dest = self.pixels[i + c] as f32;
            self.pixels[i + c] = (source + dest * inv).round().clamp(0.0, 255.0) as u8;
        }
        let dest_alpha = self.pixels[i + 3] as f32 / 255.0;
        let out_alpha = alpha + dest_alpha * inv;
        self.pixels[i + 3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    fn fill_rect(&mut self, rect: Rect, fill: super::model::Rgba, opacity: f32) {
        let x0 = rect.x.max(0.0) as usize;
        let y0 = rect.y.max(0.0) as usize;
        let x1 = rect.right().min(self.w as f32).max(0.0) as usize;
        let y1 = rect.bottom().min(self.h as f32).max(0.0) as usize;
        let alpha = fill.0[3] as f32 / 255.0 * opacity;

        for y in y0..y1 {
            for x in x0..x1 {
                self.blend(x, y, [fill.0[0], fill.0[1], fill.0[2]], alpha);
            }
        }
    }

    fn fill_round_rect(&mut self, rect: Rect, radius: f32, fill: super::model::Rgba, opacity: f32) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let radius = radius.clamp(0.0, rect.w.min(rect.h) * 0.5);
        let alpha = fill.0[3] as f32 / 255.0 * opacity;
        self.for_round_rect(rect, radius, |buffer, x, y, distance| {
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            buffer.blend(x, y, [fill.0[0], fill.0[1], fill.0[2]], alpha * coverage);
        });
    }

    fn stroke_round_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        width: f32,
        stroke: super::model::Rgba,
        opacity: f32,
    ) {
        if rect.w <= 0.0 || rect.h <= 0.0 || width <= 0.0 {
            return;
        }
        let radius = radius.clamp(0.0, rect.w.min(rect.h) * 0.5);
        let alpha = stroke.0[3] as f32 / 255.0 * opacity;
        self.for_round_rect(rect, radius, |buffer, x, y, distance| {
            // Inside stroke: the band between the outer edge and the inner edge.
            let outer = (0.5 - distance).clamp(0.0, 1.0);
            let inner = (0.5 - (distance + width)).clamp(0.0, 1.0);
            let coverage = (outer - inner).clamp(0.0, 1.0);
            buffer.blend(
                x,
                y,
                [stroke.0[0], stroke.0[1], stroke.0[2]],
                alpha * coverage,
            );
        });
    }

    fn for_round_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        mut visit: impl FnMut(&mut Buffer, usize, usize, f32),
    ) {
        let x0 = (rect.x.floor() - 1.0).max(0.0) as usize;
        let y0 = (rect.y.floor() - 1.0).max(0.0) as usize;
        let x1 = (rect.right().ceil() + 1.0).min(self.w as f32).max(0.0) as usize;
        let y1 = (rect.bottom().ceil() + 1.0).min(self.h as f32).max(0.0) as usize;

        let half_w = rect.w * 0.5;
        let half_h = rect.h * 0.5;
        let centre = egui::pos2(rect.x + half_w, rect.y + half_h);

        for y in y0..y1 {
            for x in x0..x1 {
                let px = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                let distance = rounded_box_distance(px, centre, half_w, half_h, radius);
                visit(self, x, y, distance);
            }
        }
    }

    fn draw_raster(&mut self, rect: Rect, raster: &Raster, opacity: f32) {
        if rect.w <= 0.0 || rect.h <= 0.0 || raster.w == 0 || raster.h == 0 {
            return;
        }
        let x0 = rect.x.floor().max(0.0) as usize;
        let y0 = rect.y.floor().max(0.0) as usize;
        let x1 = rect.right().ceil().min(self.w as f32).max(0.0) as usize;
        let y1 = rect.bottom().ceil().min(self.h as f32).max(0.0) as usize;

        for y in y0..y1 {
            let v = ((y as f32 + 0.5 - rect.y) / rect.h).clamp(0.0, 1.0);
            for x in x0..x1 {
                let u = ((x as f32 + 0.5 - rect.x) / rect.w).clamp(0.0, 1.0);
                let [r, g, b, a] = sample_premultiplied(raster, u, v);
                let alpha = a * opacity;
                if alpha <= 0.0 {
                    continue;
                }
                let i = (y * self.w + x) * 4;
                let inv = 1.0 - alpha;
                self.pixels[i] = (r * 255.0 * opacity + self.pixels[i] as f32 * inv)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                self.pixels[i + 1] = (g * 255.0 * opacity + self.pixels[i + 1] as f32 * inv)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                self.pixels[i + 2] = (b * 255.0 * opacity + self.pixels[i + 2] as f32 * inv)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                self.pixels[i + 3] = ((alpha + self.pixels[i + 3] as f32 / 255.0 * inv) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Downsample premultiplied data, then unpremultiply for the PNG.
    fn downsample(&self, out_w: u32, out_h: u32) -> RgbaImage {
        let Some(source) = RgbaImage::from_raw(self.w as u32, self.h as u32, self.pixels.clone())
        else {
            return RgbaImage::new(out_w.max(1), out_h.max(1));
        };
        let mut out = image::imageops::resize(&source, out_w, out_h, FilterType::Lanczos3);
        for pixel in out.pixels_mut() {
            let [r, g, b, a] = pixel.0;
            if a == 0 {
                pixel.0 = [0, 0, 0, 0];
            } else {
                let alpha = a as u32;
                pixel.0 = [
                    ((r as u32 * 255 + alpha / 2) / alpha).min(255) as u8,
                    ((g as u32 * 255 + alpha / 2) / alpha).min(255) as u8,
                    ((b as u32 * 255 + alpha / 2) / alpha).min(255) as u8,
                    a,
                ];
            }
        }
        out
    }
}

/// Signed distance to a rounded box. Negative inside, positive outside.
fn rounded_box_distance(
    point: egui::Pos2,
    centre: egui::Pos2,
    half_w: f32,
    half_h: f32,
    radius: f32,
) -> f32 {
    let qx = (point.x - centre.x).abs() - (half_w - radius);
    let qy = (point.y - centre.y).abs() - (half_h - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

/// Bilinear sample using the GPU convention: texel centres sit at `(i + 0.5) / size`.
/// All four channels are premultiplied so transparent pixels cannot bleed colour.
fn sample_premultiplied(raster: &Raster, u: f32, v: f32) -> [f32; 4] {
    let x = (u * raster.w as f32 - 0.5).clamp(0.0, (raster.w - 1) as f32);
    let y = (v * raster.h as f32 - 0.5).clamp(0.0, (raster.h - 1) as f32);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(raster.w - 1);
    let y1 = (y0 + 1).min(raster.h - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let p00 = premultiplied_texel(raster, x0, y0);
    let p10 = premultiplied_texel(raster, x1, y0);
    let p01 = premultiplied_texel(raster, x0, y1);
    let p11 = premultiplied_texel(raster, x1, y1);

    let mut out = [0.0; 4];
    for c in 0..4 {
        let top = p00[c] + (p10[c] - p00[c]) * tx;
        let bottom = p01[c] + (p11[c] - p01[c]) * tx;
        out[c] = top + (bottom - top) * ty;
    }
    out
}

fn premultiplied_texel(raster: &Raster, x: usize, y: usize) -> [f32; 4] {
    let i = (y * raster.w + x) * 4;
    let alpha = raster.pixels[i + 3] as f32 / 255.0;
    [
        raster.pixels[i] as f32 / 255.0 * alpha,
        raster.pixels[i + 1] as f32 / 255.0 * alpha,
        raster.pixels[i + 2] as f32 / 255.0 * alpha,
        alpha,
    ]
}

// ── Text ────────────────────────────────────────────────────────────────────

fn draw_text(
    buffer: &mut Buffer,
    font: &Font,
    text: &str,
    px: f32,
    rect: Rect,
    color: super::model::Rgba,
    opacity: f32,
) {
    let px = px.max(1.0);
    let Some(line) = font.horizontal_line_metrics(px) else {
        return;
    };
    let color_alpha = color.0[3] as f32 / 255.0 * opacity;
    if color_alpha <= 0.0 {
        return;
    }

    for (index, text_line) in text.split('\n').enumerate() {
        let baseline = rect.y + line.ascent + index as f32 * line.new_line_size;
        draw_text_line(
            buffer,
            font,
            text_line,
            px,
            rect.x,
            baseline,
            color,
            color_alpha,
        );
    }
}

fn draw_text_line(
    buffer: &mut Buffer,
    font: &Font,
    line: &str,
    px: f32,
    x: f32,
    baseline: f32,
    color: super::model::Rgba,
    color_alpha: f32,
) {
    let mut pen_x = x;
    let mut previous: Option<char> = None;

    for ch in line.chars() {
        if ch == '\n' {
            break;
        }
        if ch == '\r' {
            continue;
        }
        if let Some(left) = previous {
            pen_x += font.horizontal_kern(left, ch, px).unwrap_or(0.0);
        }

        let glyph_index = font.lookup_glyph_index(ch);
        let (metrics, bitmap) = font.rasterize(ch, px);
        if glyph_index != 0 && metrics.width > 0 && metrics.height > 0 {
            let left = pen_x.round() as i32 + metrics.xmin;
            let top = (baseline - metrics.ymin as f32 - metrics.height as f32).round() as i32;

            for row in 0..metrics.height {
                let y = top + row as i32;
                if y < 0 || y as usize >= buffer.h {
                    continue;
                }
                for column in 0..metrics.width {
                    let x = left + column as i32;
                    if x < 0 || x as usize >= buffer.w {
                        continue;
                    }
                    let coverage = bitmap[row * metrics.width + column] as f32 / 255.0;
                    if coverage > 0.0 {
                        buffer.blend(
                            x as usize,
                            y as usize,
                            [color.0[0], color.0[1], color.0[2]],
                            color_alpha * coverage,
                        );
                    }
                }
            }
        }

        pen_x += metrics.advance_width;
        previous = Some(ch);
    }
}

// ── Geometry ────────────────────────────────────────────────────────────────

fn scale_rect(rect: Rect, scale: f32) -> Rect {
    Rect::new(
        rect.x * scale,
        rect.y * scale,
        rect.w * scale,
        rect.h * scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::model::{Kind, Rect, Rgba};

    fn sample_state() -> (Document, Rasters) {
        let doc = Document::sample();
        let mut rasters = Rasters::default();
        rasters.sync(&doc);
        (doc, rasters)
    }

    #[test]
    fn export_is_exactly_the_document_size() {
        let (doc, rasters) = sample_state();
        let image = render_png(&doc, &rasters).expect("export succeeds");
        assert_eq!(image.dimensions(), (1200, 800));
    }

    #[test]
    fn background_and_card_have_the_expected_colours() {
        let (doc, rasters) = sample_state();
        let image = render_png(&doc, &rasters).expect("export succeeds");

        assert_eq!(image.get_pixel(10, 10).0, [18, 18, 24, 255]);
        assert_eq!(image.get_pixel(700, 140).0, [255, 255, 255, 255]);
    }

    #[test]
    fn card_corners_are_rounded() {
        let (doc, rasters) = sample_state();
        let image = render_png(&doc, &rasters).expect("export succeeds");

        // Just inside the card's top-left bounding box, but outside its 28 px corner.
        assert_eq!(image.get_pixel(642, 122).0, [18, 18, 24, 255]);
        // Well inside the card.
        assert_eq!(image.get_pixel(700, 140).0, [255, 255, 255, 255]);
    }

    #[test]
    fn text_is_drawn() {
        let (doc, rasters) = sample_state();
        let image = render_png(&doc, &rasters).expect("export succeeds");

        let mut dark_pixels = 0;
        for y in 260..380 {
            for x in 690..1070 {
                if image.get_pixel(x, y).0[0] < 100 {
                    dark_pixels += 1;
                }
            }
        }
        assert!(dark_pixels > 500, "price text should be visible");
    }

    #[test]
    fn hidden_layers_are_not_exported() {
        let (mut doc, mut rasters) = sample_state();
        for node in &mut doc.nodes {
            if node.name == "Card" {
                node.visible = false;
            }
        }
        rasters.sync(&doc);
        let image = render_png(&doc, &rasters).expect("export succeeds");
        // The hidden white card leaves the dark background behind.
        assert_eq!(image.get_pixel(700, 140).0, [18, 18, 24, 255]);
    }

    #[test]
    fn an_empty_document_is_transparent() {
        let doc = Document::new("Empty", 64.0, 48.0);
        let rasters = Rasters::default();
        let image = render_png(&doc, &rasters).expect("export succeeds");
        assert_eq!(image.dimensions(), (64, 48));
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(image.get_pixel(32, 24).0, [0, 0, 0, 0]);
    }

    #[test]
    fn stroke_is_drawn_inside_the_shape() {
        let mut doc = Document::new("Stroke", 100.0, 100.0);
        doc.add(
            "Box",
            Kind::Rect {
                fill: Rgba::WHITE,
                radius: 0.0,
                stroke: 10.0,
                stroke_color: Rgba([255, 0, 0, 255]),
            },
            Rect::new(20.0, 20.0, 60.0, 60.0),
        );
        let rasters = Rasters::default();
        let image = render_png(&doc, &rasters).expect("export succeeds");

        let stroke = image.get_pixel(22, 50).0;
        assert_eq!((stroke[0], stroke[1], stroke[2]), (255, 0, 0));
        assert!(stroke[3] >= 250, "stroke alpha was {}", stroke[3]);

        let fill = image.get_pixel(50, 50).0;
        assert_eq!((fill[0], fill[1], fill[2]), (255, 255, 255));
        assert!(fill[3] >= 250, "fill alpha was {}", fill[3]);

        assert_eq!(image.get_pixel(10, 50).0, [0, 0, 0, 0]);
    }

    #[test]
    fn transparency_and_opacity_survive_export() {
        let mut doc = Document::new("Opacity", 40.0, 40.0);
        doc.add(
            "Half",
            Kind::Frame {
                fill: Rgba([255, 0, 0, 255]),
            },
            Rect::new(0.0, 0.0, 40.0, 40.0),
        );
        if let Some(node) = doc.nodes.last_mut() {
            node.opacity = 0.5;
        }
        let rasters = Rasters::default();
        let image = render_png(&doc, &rasters).expect("export succeeds");
        let [r, g, b, a] = image.get_pixel(20, 20).0;
        assert_eq!((r, g, b), (255, 0, 0));
        assert!((a as i32 - 128).abs() <= 1, "alpha was {a}");
    }

    #[test]
    fn painted_pixels_reach_the_png() {
        let mut doc = Document::new("Paint", 100.0, 100.0);
        let id = doc.add(
            "Paint",
            Kind::Paint { px: (100, 100) },
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
        let mut rasters = Rasters::default();
        rasters.sync(&doc);
        rasters.map.get_mut(&id).expect("paint raster").paint(
            50.0,
            50.0,
            20.0,
            Rgba([0, 255, 0, 255]),
            false,
        );

        let image = render_png(&doc, &rasters).expect("export succeeds");
        let [r, g, b, a] = image.get_pixel(50, 50).0;
        assert!(a > 200, "alpha was {a}");
        assert!(g > 200 && r < 60 && b < 60, "pixel was {r},{g},{b}");
    }

    #[test]
    fn an_erased_photo_exports_with_transparency() {
        let mut doc = Document::new("Erase", 100.0, 100.0);
        doc.add(
            "Photo",
            Kind::Photo {
                source: crate::design::model::PhotoSource::Sample,
                px: (100, 100),
            },
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
        let mut rasters = Rasters::default();
        rasters.sync(&doc);
        let id = doc.nodes[0].id;
        rasters
            .map
            .get_mut(&id)
            .expect("photo raster")
            .paint(50.0, 50.0, 30.0, Rgba::ACCENT, true);

        let image = render_png(&doc, &rasters).expect("export succeeds");
        assert_eq!(image.get_pixel(50, 50).0[3], 0);
        assert!(image.get_pixel(5, 5).0[3] > 0);
    }
}
