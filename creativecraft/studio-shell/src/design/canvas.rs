//! Canvas rendering and direct manipulation, shared by all three personas.

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect as ERect, Sense, Stroke, StrokeKind};

use super::Persona;
use super::model::{Document, History, NodeId, Rasters, Rect, Rgba};

pub struct PaintSettings {
    pub color: Rgba,
    pub radius: f32,
    pub erase: bool,
}

impl Default for PaintSettings {
    fn default() -> Self {
        Self {
            color: Rgba::ACCENT,
            radius: 24.0,
            erase: false,
        }
    }
}

/// Draw the canvas and handle selection, dragging and painting.
pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    rasters: &mut Rasters,
    history: &mut History,
    persona: Persona,
    selected: &mut Option<NodeId>,
    paint: &PaintSettings,
) {
    // The panel this canvas lives in can report more room than it shows (the
    // docked panels outside it), so fit the artboard to the visible area: the
    // clip rect is what the user actually sees.
    let available = ui
        .available_rect_before_wrap()
        .intersect(ui.clip_rect())
        .size()
        .max(egui::vec2(1.0, 1.0));
    let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());

    let pad = 56.0;
    let scale = ((available.x - pad * 2.0) / doc.canvas_w)
        .min((available.y - pad * 2.0) / doc.canvas_h)
        .max(0.05);
    let canvas_size = egui::vec2(doc.canvas_w * scale, doc.canvas_h * scale);
    let origin = response.rect.center() - canvas_size * 0.5;
    let canvas = ERect::from_min_size(origin, canvas_size);

    // Desk and canvas shadow.
    painter.rect_filled(response.rect, 0, Color32::from_rgb(14, 14, 18));
    painter.rect_filled(
        canvas.translate(egui::vec2(0.0, 8.0)).expand(4.0),
        6,
        Color32::from_black_alpha(90),
    );

    let view = painter.with_clip_rect(canvas.intersect(response.rect));

    // Paint order: bottom to top.
    for node in &doc.nodes {
        if !node.visible {
            continue;
        }
        let r = map_rect(node.rect, origin, scale);
        let alpha = node.opacity.clamp(0.0, 1.0);

        match &node.kind {
            super::model::Kind::Frame { fill } => {
                view.rect_filled(r, 0, fill.with_alpha(alpha));
            }
            super::model::Kind::Rect {
                fill,
                radius,
                stroke,
                stroke_color,
            } => {
                let cr = corner(*radius * scale);
                let stroke_w = stroke * scale;
                if stroke_w > 0.01 {
                    view.rect(
                        r,
                        cr,
                        fill.with_alpha(alpha),
                        Stroke::new(stroke_w, stroke_color.with_alpha(alpha)),
                        StrokeKind::Inside,
                    );
                } else {
                    view.rect_filled(r, cr, fill.with_alpha(alpha));
                }
            }
            super::model::Kind::Text { text, size, color } => {
                view.text(
                    r.left_top(),
                    Align2::LEFT_TOP,
                    text,
                    FontId::proportional((size * scale).max(1.0)),
                    color.with_alpha(alpha),
                );
            }
            super::model::Kind::Photo { .. } | super::model::Kind::Paint { .. } => {
                if let Some(raster) = rasters.map.get_mut(&node.id) {
                    let tex = raster.texture(ui.ctx(), &format!("raster-{}", node.id));
                    let tint = Color32::from_white_alpha((alpha * 255.0).round() as u8);
                    view.image(
                        tex,
                        r,
                        ERect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        tint,
                    );
                }
            }
        }
    }

    // Selection outline and handles.
    if let Some(id) = *selected {
        if let Some(node) = doc.node(id) {
            let r = map_rect(node.rect, origin, scale).expand(1.0);
            painter.rect_stroke(
                r,
                0,
                Stroke::new(1.5, Rgba::ACCENT.color32()),
                StrokeKind::Outside,
            );
            for corner_pos in [
                r.left_top(),
                r.right_top(),
                r.left_bottom(),
                r.right_bottom(),
            ] {
                painter.rect_filled(
                    ERect::from_center_size(corner_pos, egui::vec2(7.0, 7.0)),
                    1,
                    Color32::WHITE,
                );
                painter.rect_stroke(
                    ERect::from_center_size(corner_pos, egui::vec2(7.0, 7.0)),
                    1,
                    Stroke::new(1.0, Rgba::ACCENT.color32()),
                    StrokeKind::Inside,
                );
            }

            // Vector persona: show the shape's geometry points as a visual cue.
            if persona == Persona::Vector {
                if let super::model::Kind::Rect { .. } = node.kind {
                    for p in [
                        r.left_top(),
                        r.right_top(),
                        r.left_bottom(),
                        r.right_bottom(),
                    ] {
                        painter.circle_filled(p, 3.0, Rgba::ACCENT.color32());
                    }
                }
            }
        }
    }

    // Pointer → document space.
    let to_doc =
        |p: Pos2| -> Pos2 { egui::pos2((p.x - origin.x) / scale, (p.y - origin.y) / scale) };

    let hover_doc = response.hover_pos().map(to_doc);
    let inside = hover_doc
        .map(|p| p.x >= 0.0 && p.y >= 0.0 && p.x <= doc.canvas_w && p.y <= doc.canvas_h)
        .unwrap_or(false);

    if persona == Persona::Pixel {
        if inside {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }

        let painting = inside && (response.dragged() || response.clicked());
        if painting {
            if let Some(p) = hover_doc {
                paint_at(doc, rasters, *selected, p, paint, scale);
            }
        }
    } else {
        let drag_id = ui.scope_id().with("canvas_drag");

        // Start of a drag: near the bottom-right handle resizes, anywhere else moves/selects.
        if response.drag_started() {
            if let Some(p) = response.interact_pointer_pos() {
                let mut resize = false;
                if let Some(id) = *selected {
                    if let Some(node) = doc.node(id) {
                        let handle = map_rect(node.rect, origin, scale).right_bottom();
                        resize = (p - handle).length() <= 14.0;
                    }
                }
                if resize {
                    history.checkpoint(doc);
                    ui.data_mut(|d| d.insert_temp(drag_id, true));
                } else {
                    let hit = doc.hit(to_doc(p));
                    *selected = hit;
                    if hit.is_some() {
                        history.checkpoint(doc);
                    }
                    ui.data_mut(|d| d.insert_temp(drag_id, false));
                }
            }
        }

        if response.clicked() {
            if let Some(p) = hover_doc {
                *selected = doc.hit(p);
            }
        }

        if response.dragged() {
            let resizing = ui.data(|d| d.get_temp::<bool>(drag_id)).unwrap_or(false);
            let delta = response.drag_delta() / scale;
            if let Some(id) = *selected {
                if let Some(node) = doc.node_mut(id) {
                    if node.visible {
                        if resizing {
                            node.rect.w = (node.rect.w + delta.x).max(8.0);
                            node.rect.h = (node.rect.h + delta.y).max(8.0);
                        } else {
                            node.rect.x += delta.x;
                            node.rect.y += delta.y;
                        }
                    }
                }
            }
        }

        if response.drag_stopped() {
            ui.data_mut(|d| d.remove_temp::<bool>(drag_id));
        }

        // Cursor feedback over the resize handle.
        if let (Some(id), Some(p)) = (*selected, response.hover_pos()) {
            if let Some(node) = doc.node(id) {
                let handle = map_rect(node.rect, origin, scale).right_bottom();
                if (p - handle).length() <= 14.0 {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                }
            }
        }
    }

    // Small hint in the corner of the view.
    let hint = match persona {
        Persona::Pixel => {
            if paint.erase {
                "Erase mode: drag to cut transparency into the selected layer."
            } else {
                "Paint on the selected photo or paint layer. Erase cuts transparency."
            }
        }
        Persona::Vector => "Select a shape to edit its path, fill and corners.",
        Persona::Layout => "Click to select, drag to move, drag the corner to resize.",
    };
    painter.text(
        response.rect.left_bottom() + egui::vec2(12.0, -10.0),
        Align2::LEFT_BOTTOM,
        hint,
        FontId::proportional(11.0),
        Color32::from_gray(110),
    );
}

fn paint_at(
    doc: &Document,
    rasters: &mut Rasters,
    selected: Option<NodeId>,
    p: Pos2,
    paint: &PaintSettings,
    scale: f32,
) {
    // The selected raster layer is the target, so a photo can be erased
    // directly. With no raster selected, paint on the topmost paint layer.
    let selected_target = selected.filter(|id| {
        doc.node(*id)
            .map(|n| n.visible && is_raster(&n.kind) && n.rect.contains(p))
            .unwrap_or(false)
    });
    let target = selected_target.or_else(|| {
        doc.nodes
            .iter()
            .rev()
            .find(|n| {
                n.visible
                    && matches!(n.kind, super::model::Kind::Paint { .. })
                    && n.rect.contains(p)
            })
            .map(|n| n.id)
    });

    let Some(id) = target else {
        return;
    };
    let Some(node) = doc.node(id) else {
        return;
    };
    let Some(raster) = rasters.map.get_mut(&id) else {
        return;
    };

    let rx = (p.x - node.rect.x) / node.rect.w * raster.w as f32;
    let ry = (p.y - node.rect.y) / node.rect.h * raster.h as f32;
    let radius = (paint.radius / scale) * (raster.w as f32 / node.rect.w);
    raster.paint(rx, ry, radius, paint.color, paint.erase);
}

fn is_raster(kind: &super::model::Kind) -> bool {
    matches!(
        kind,
        super::model::Kind::Photo { .. } | super::model::Kind::Paint { .. }
    )
}

fn map_rect(r: Rect, origin: Pos2, scale: f32) -> ERect {
    ERect::from_min_size(
        origin + egui::vec2(r.x, r.y) * scale,
        egui::vec2(r.w, r.h) * scale,
    )
}

fn corner(radius: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(radius.clamp(0.0, 255.0) as u8)
}
