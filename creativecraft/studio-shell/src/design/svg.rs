//! The shared design as SVG — the hand-off to VectorCraft.
//!
//! VectorCraft (and FigCraft) read SVG, so the design document is written out
//! with the shapes, text and raster layers it contains. The round trip is
//! deliberately one-way *in* (design → SVG); artwork comes back as a flattened
//! PNG through VectorCraft's own export, which keeps this writer small and
//! predictable instead of re-implementing a full SVG importer.

use super::model::{Document, Kind, Raster, Rasters, Rgba};

/// The document as an SVG string at its own size.
pub(crate) fn export_svg(doc: &Document, rasters: &Rasters) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\">\n",
        w = number(doc.canvas_w),
        h = number(doc.canvas_h)
    ));
    out.push_str(&format!("<title>{}</title>\n", escaped(&doc.title)));

    for node in doc.nodes.iter().filter(|node| node.visible) {
        let rect = &node.rect;
        let opacity = node.opacity.clamp(0.0, 1.0);
        match &node.kind {
            Kind::Frame { fill } => {
                out.push_str(&rect_tag(rect, Some(*fill), 0.0, None));
            }
            Kind::Rect {
                fill,
                radius,
                stroke,
                stroke_color,
            } => {
                out.push_str(&rect_tag(
                    rect,
                    Some(*fill),
                    *radius,
                    (*stroke > 0.0).then_some((*stroke, *stroke_color)),
                ));
            }
            Kind::Text { text, size, color } => {
                // SVG places text on its baseline; our rect's top is the text box.
                let baseline = rect.y + size * 0.8;
                out.push_str(&format!(
                    "<text x=\"{x}\" y=\"{y}\" font-family=\"Inter, Arial, sans-serif\" \
                     font-size=\"{size}\" fill=\"{color}\" fill-opacity=\"{opacity}\">{text}</text>\n",
                    x = number(rect.x),
                    y = number(baseline),
                    size = number(*size),
                    color = hex(*color),
                    opacity = number(opacity * alpha(*color)),
                    text = escaped(text)
                ));
            }
            Kind::Photo { px, .. } | Kind::Paint { px } => {
                let Some(raster) = rasters.map.get(&node.id) else {
                    continue;
                };
                let data = png_base64(raster)?;
                out.push_str(&format!(
                    "<image x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" opacity=\"{opacity}\" \
                     preserveAspectRatio=\"none\" \
                     href=\"data:image/png;base64,{data}\"/>\n",
                    x = number(rect.x),
                    y = number(rect.y),
                    w = number(px.0 as f32),
                    h = number(px.1 as f32),
                    opacity = number(opacity),
                ));
            }
        }
    }

    out.push_str("</svg>\n");
    Ok(out)
}

fn rect_tag(
    rect: &super::model::Rect,
    fill: Option<Rgba>,
    radius: f32,
    stroke: Option<(f32, Rgba)>,
) -> String {
    let mut tag = format!(
        "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"",
        x = number(rect.x),
        y = number(rect.y),
        w = number(rect.w),
        h = number(rect.h)
    );
    if radius > 0.0 {
        tag.push_str(&format!(" rx=\"{}\"", number(radius)));
    }
    if let Some(fill) = fill {
        tag.push_str(&format!(
            " fill=\"{}\" fill-opacity=\"{}\"",
            hex(fill),
            number(alpha(fill))
        ));
    } else {
        tag.push_str(" fill=\"none\"");
    }
    if let Some((width, color)) = stroke {
        tag.push_str(&format!(
            " stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{}\"",
            hex(color),
            number(alpha(color)),
            number(width)
        ));
    }
    tag.push_str("/>\n");
    tag
}

fn png_base64(raster: &Raster) -> Result<String, String> {
    let (w, h) = (raster.w as u32, raster.h as u32);
    let image = image::RgbaImage::from_raw(w, h, raster.pixels.clone())
        .ok_or_else(|| "a raster layer has a malformed pixel buffer".to_string())?;
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|error| format!("Could not encode a layer as PNG: {error}"))?;
    Ok(base64(&bytes))
}

fn hex(color: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", color.0[0], color.0[1], color.0[2])
}

fn alpha(color: Rgba) -> f32 {
    f32::from(color.0[3]) / 255.0
}

/// Trim float noise: `1200` rather than `1200.0000001`.
fn number(value: f32) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if (rounded - rounded.round()).abs() < f32::EPSILON {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Base64 without a dependency: the writer only needs the standard alphabet.
fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(char::from(TABLE[((n >> 18) & 63) as usize]));
        out.push(char::from(TABLE[((n >> 12) & 63) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(TABLE[((n >> 6) & 63) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(TABLE[(n & 63) as usize])
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::model::{Node, Rect};

    fn document() -> Document {
        Document::new("Pricing", 1200.0, 800.0)
    }

    #[test]
    fn shapes_and_text_become_svg() {
        let mut doc = document();
        doc.nodes.push(Node {
            id: 1,
            name: "Card".into(),
            rect: Rect::new(10.0, 20.0, 400.0, 300.0),
            visible: true,
            opacity: 1.0,
            kind: Kind::Rect {
                fill: Rgba([255, 0, 0, 255]),
                radius: 12.0,
                stroke: 0.0,
                stroke_color: Rgba::WHITE,
            },
        });
        doc.nodes.push(Node {
            id: 2,
            name: "Title".into(),
            rect: Rect::new(40.0, 60.0, 200.0, 40.0),
            visible: true,
            opacity: 1.0,
            kind: Kind::Text {
                text: "A & B <ok>".into(),
                size: 32.0,
                color: Rgba::INK,
            },
        });
        let svg = export_svg(&doc, &Rasters::default()).expect("SVG");
        assert!(svg.contains("viewBox=\"0 0 1200 800\""));
        assert!(svg.contains("<rect x=\"10\" y=\"20\" width=\"400\" height=\"300\" rx=\"12\""));
        assert!(svg.contains("fill=\"#ff0000\""));
        assert!(svg.contains("A &amp; B &lt;ok&gt;"), "{svg}");
    }

    #[test]
    fn hidden_layers_are_left_out() {
        let mut doc = document();
        doc.nodes.push(Node {
            id: 1,
            name: "Hidden".into(),
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            visible: false,
            opacity: 1.0,
            kind: Kind::Frame { fill: Rgba::ACCENT },
        });
        let svg = export_svg(&doc, &Rasters::default()).expect("SVG");
        assert!(!svg.contains("<rect"));
    }

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }
}
