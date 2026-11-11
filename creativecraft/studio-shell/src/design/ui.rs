//! The design workspace UI: toolbar, persona tools, inspector and canvas.

use std::path::{Path, PathBuf};

use eframe::egui;
use egui::{Color32, RichText};

use super::canvas;
use super::model::{Kind, NodeId, PhotoSource, Raster, Rect, Rgba};
use super::{DesignState, Persona};

const PANEL_BG: Color32 = Color32::from_rgb(24, 24, 28);
const TEXT_DIM: Color32 = Color32::from_rgb(150, 150, 162);
const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);

pub fn show(
    state: &mut DesignState,
    ui: &mut egui::Ui,
    persona: Persona,
    timeline_available: bool,
) {
    dropped_photos(state, ui);
    shortcuts(state, ui);
    toolbar(state, ui, timeline_available);
    tools(state, ui, persona);
    inspector(state, ui, persona);

    egui::CentralPanel::default().show(ui, |ui| {
        canvas::show(
            ui,
            &mut state.doc,
            &mut state.rasters,
            &mut state.history,
            persona,
            &mut state.selected,
            &state.paint,
        );
    });
}

fn shortcuts(state: &mut DesignState, ui: &egui::Ui) {
    if ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let (undo, redo, save, open, export) = ui.input(|i| {
        let ctrl = i.modifiers.ctrl;
        (
            ctrl && i.key_pressed(egui::Key::Z) && !i.modifiers.shift,
            ctrl && ((i.modifiers.shift && i.key_pressed(egui::Key::Z))
                || i.key_pressed(egui::Key::Y)),
            ctrl && i.key_pressed(egui::Key::S),
            ctrl && i.key_pressed(egui::Key::O),
            ctrl && i.key_pressed(egui::Key::E),
        )
    });
    if undo {
        state.undo();
    }
    if redo {
        state.redo();
    }
    if save {
        state.save();
    }
    if open {
        state.open();
    }
    if export {
        state.export_png();
    }
}

fn toolbar(state: &mut DesignState, ui: &mut egui::Ui, timeline_available: bool) {
    egui::Panel::top("design_toolbar")
        .exact_size(36.0)
        .frame(egui::Frame::new().fill(PANEL_BG))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(state.doc.title.as_str())
                        .strong()
                        .size(13.0)
                        .color(Color32::WHITE),
                );
                ui.add_space(6.0);
                if ui
                    .add_enabled(state.history.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    state.undo();
                }
                if ui
                    .add_enabled(state.history.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    state.redo();
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    state.save();
                }
                if ui.button("Open").clicked() {
                    state.open();
                }
                if ui
                    .button("Export PNG")
                    .on_hover_text("Save the whole canvas as a PNG (Ctrl+E)")
                    .clicked()
                {
                    state.export_png();
                }
                if timeline_available
                    && ui
                        .button("Place on timeline")
                        .on_hover_text("Export this design and insert it at the FilmCraft playhead")
                        .clicked()
                {
                    state.request_place_on_timeline();
                }
                if timeline_available
                    && ui
                        .button("Frame from timeline")
                        .on_hover_text(
                            "Import the frame under the Edit playhead as a photo layer here",
                        )
                        .clicked()
                {
                    state.request_frame_from_timeline();
                }
                ui.separator();
                let status = if state.doc.status.is_empty() {
                    "One document · three personas"
                } else {
                    state.doc.status.as_str()
                };
                ui.label(RichText::new(status).size(11.0).color(TEXT_DIM));
            });
        });
}

fn tools(state: &mut DesignState, ui: &mut egui::Ui, persona: Persona) {
    egui::Panel::left("design_tools")
        .exact_size(220.0)
        .frame(egui::Frame::new().fill(PANEL_BG))
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(persona.icon()).size(17.0));
                ui.label(
                    RichText::new(persona.label())
                        .strong()
                        .size(14.0)
                        .color(Color32::WHITE),
                );
            });
            ui.label(RichText::new(persona.blurb()).size(11.0).color(TEXT_DIM));
            ui.add_space(6.0);
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    match persona {
                        Persona::Layout => layout_tools(state, ui),
                        Persona::Vector => vector_tools(state, ui),
                        Persona::Pixel => pixel_tools(state, ui),
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    layers(state, ui);
                });
        });
}

fn layout_tools(state: &mut DesignState, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.label(RichText::new("Add").strong().size(12.0));
    if ui.button("+ Frame").clicked() {
        add_node(
            state,
            "Frame",
            Kind::Frame {
                fill: Rgba([36, 36, 44, 255]),
            },
            Rect::new(120.0, 120.0, 560.0, 420.0),
        );
    }
    if ui.button("+ Rectangle").clicked() {
        add_node(
            state,
            "Rectangle",
            Kind::Rect {
                fill: Rgba::ACCENT,
                radius: 16.0,
                stroke: 0.0,
                stroke_color: Rgba::WHITE,
            },
            Rect::new(420.0, 260.0, 360.0, 220.0),
        );
    }
    if ui.button("+ Text").clicked() {
        add_node(
            state,
            "Text",
            Kind::Text {
                text: "New text".into(),
                size: 32.0,
                color: Rgba::WHITE,
            },
            Rect::new(450.0, 300.0, 320.0, 48.0),
        );
    }

    ui.add_space(10.0);
    ui.label(RichText::new("Align selected").strong().size(12.0));
    if let Some(id) = state.selected {
        let canvas_w = state.doc.canvas_w;
        let canvas_h = state.doc.canvas_h;
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("Left").clicked() {
                if let Some(n) = state.doc.node_mut(id) {
                    n.rect.x = 0.0;
                }
            }
            if ui.small_button("Center").clicked() {
                if let Some(n) = state.doc.node_mut(id) {
                    n.rect.x = (canvas_w - n.rect.w) * 0.5;
                }
            }
            if ui.small_button("Right").clicked() {
                if let Some(n) = state.doc.node_mut(id) {
                    n.rect.x = canvas_w - n.rect.w;
                }
            }
            if ui.small_button("Top").clicked() {
                if let Some(n) = state.doc.node_mut(id) {
                    n.rect.y = 0.0;
                }
            }
            if ui.small_button("Middle").clicked() {
                if let Some(n) = state.doc.node_mut(id) {
                    n.rect.y = (canvas_h - n.rect.h) * 0.5;
                }
            }
            if ui.small_button("Bottom").clicked() {
                if let Some(n) = state.doc.node_mut(id) {
                    n.rect.y = canvas_h - n.rect.h;
                }
            }
        });
    } else {
        ui.label(
            RichText::new("Select a layer first.")
                .size(11.0)
                .color(TEXT_DIM),
        );
    }
}

fn vector_tools(state: &mut DesignState, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.label(RichText::new("Add").strong().size(12.0));
    if ui.button("+ Rectangle").clicked() {
        add_node(
            state,
            "Rectangle",
            Kind::Rect {
                fill: Rgba::ACCENT,
                radius: 16.0,
                stroke: 0.0,
                stroke_color: Rgba::WHITE,
            },
            Rect::new(420.0, 260.0, 360.0, 220.0),
        );
    }
    if ui.button("+ Text").clicked() {
        add_node(
            state,
            "Text",
            Kind::Text {
                text: "New text".into(),
                size: 32.0,
                color: Rgba::WHITE,
            },
            Rect::new(450.0, 300.0, 320.0, 48.0),
        );
    }
    ui.add_space(10.0);
    ui.label(
        RichText::new("Select a shape on the canvas, then edit its path in the inspector.")
            .size(11.0)
            .color(TEXT_DIM),
    );
}

fn pixel_tools(state: &mut DesignState, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.label(RichText::new("Photo").strong().size(12.0));
    if ui.button("Import photo...").clicked() {
        if let Some(path) = pick_photo() {
            let target = default_photo_target(state);
            import_photo(state, &path, target);
        }
    }
    ui.label(
        RichText::new("Replaces the selected photo, or adds a new layer. You can also drop an image on the window.")
            .size(11.0)
            .color(TEXT_DIM),
    );

    ui.add_space(8.0);
    ui.separator();
    ui.label(RichText::new("Brush").strong().size(12.0));

    let target_name = state
        .selected
        .and_then(|id| state.doc.node(id))
        .filter(|n| n.visible && is_raster(&n.kind))
        .map(|n| n.name.clone())
        .or_else(|| {
            state
                .doc
                .nodes
                .iter()
                .rev()
                .find(|n| n.visible && matches!(&n.kind, Kind::Paint { .. }))
                .map(|n| n.name.clone())
        });
    ui.label(
        RichText::new(match target_name {
            Some(name) => format!("Brush paints: {name}"),
            None => "Brush paints: nothing selected".into(),
        })
        .size(11.0)
        .color(TEXT_DIM),
    );

    let mut color = state.paint.color.color32();
    ui.horizontal(|ui| {
        if ui.color_edit_button_srgba(&mut color).changed() {
            state.paint.color = Rgba::from_color32(color);
        }
        ui.label("Colour");
    });
    ui.add(egui::Slider::new(&mut state.paint.radius, 2.0..=120.0).text("Size"));
    ui.checkbox(&mut state.paint.erase, "Erase (cut out)");
    ui.label(
        RichText::new("Erase removes pixels from the selected photo or paint layer.")
            .size(11.0)
            .color(TEXT_DIM),
    );

    ui.add_space(8.0);
    let paint_layer = state
        .doc
        .nodes
        .iter()
        .find(|n| matches!(&n.kind, Kind::Paint { .. }))
        .map(|n| n.id);

    match paint_layer {
        Some(id) => {
            if ui.button("Clear paint layer").clicked() {
                if let Some(raster) = state.rasters.map.get_mut(&id) {
                    raster.clear();
                }
            }
            ui.label(
                RichText::new("Strokes paint into the layer, above the card.")
                    .size(11.0)
                    .color(TEXT_DIM),
            );
        }
        None => {
            if ui.button("+ Paint layer").clicked() {
                state.history.checkpoint(&state.doc);
                let id = state.doc.add(
                    "Paint layer",
                    Kind::Paint { px: (600, 400) },
                    Rect::new(0.0, 0.0, state.doc.canvas_w, state.doc.canvas_h),
                );
                state.selected = Some(id);
                state.rasters.sync(&state.doc);
            }
        }
    }
}

fn is_raster(kind: &Kind) -> bool {
    matches!(kind, Kind::Photo { .. } | Kind::Paint { .. })
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        })
        .unwrap_or(false)
}

fn pick_photo() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
}

/// A selected photo is replaced. If the document still contains only the demo's
/// sample portrait, that slot is filled instead of adding a second photo.
fn default_photo_target(state: &DesignState) -> Option<NodeId> {
    if let Some(id) = state.selected {
        if state
            .doc
            .node(id)
            .is_some_and(|n| matches!(&n.kind, Kind::Photo { .. }))
        {
            return Some(id);
        }
    }

    let mut photos = state
        .doc
        .nodes
        .iter()
        .filter(|n| matches!(&n.kind, Kind::Photo { .. }));
    let first = photos.next()?;
    if photos.next().is_none()
        && matches!(
            &first.kind,
            Kind::Photo {
                source: PhotoSource::Sample,
                ..
            }
        )
    {
        Some(first.id)
    } else {
        None
    }
}

pub(super) fn import_photo(state: &mut DesignState, path: &Path, target: Option<NodeId>) {
    // Dialog and drag-and-drop paths are already absolute. Only resolve a
    // relative path, and avoid Windows' `\\?\` prefix in the inspector.
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    };
    let raster = match Raster::load_image(&path) {
        Ok(raster) => raster.downscale(2048),
        Err(error) => {
            state.doc.status = error;
            return;
        }
    };

    let target = target.filter(|id| {
        state
            .doc
            .node(*id)
            .is_some_and(|n| matches!(&n.kind, Kind::Photo { .. }))
    });
    let replaced = target.is_some();
    let px = (raster.w as u32, raster.h as u32);
    let aspect = raster.w as f32 / raster.h as f32;
    let kind = Kind::Photo {
        source: PhotoSource::File(path.display().to_string()),
        px,
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string();

    state.history.checkpoint(&state.doc);

    let id = match target {
        Some(id) => {
            if let Some(node) = state.doc.node_mut(id) {
                node.rect = fit_rect(node.rect, aspect);
                node.kind = kind;
            }
            id
        }
        None => {
            let box_rect = Rect::new(
                state.doc.canvas_w * 0.225,
                state.doc.canvas_h * 0.15,
                state.doc.canvas_w * 0.55,
                state.doc.canvas_h * 0.7,
            );
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("Photo")
                .to_string();
            state.doc.add(name, kind, fit_rect(box_rect, aspect))
        }
    };

    state.rasters.map.insert(id, raster);
    state.selected = Some(id);
    state.doc.status = if replaced {
        format!("Replaced photo with {file_name} ({} × {})", px.0, px.1)
    } else {
        format!("Imported {file_name} ({} × {})", px.0, px.1)
    };
}

/// Fit an image of `aspect` (width / height) inside `bounds`, centred.
fn fit_rect(bounds: Rect, aspect: f32) -> Rect {
    let aspect = aspect.max(0.0001);
    let mut w = bounds.w.max(1.0);
    let mut h = w / aspect;
    if h > bounds.h {
        h = bounds.h.max(1.0);
        w = h * aspect;
    }
    Rect::new(
        bounds.x + (bounds.w - w) * 0.5,
        bounds.y + (bounds.h - h) * 0.5,
        w,
        h,
    )
}

fn dropped_photos(state: &mut DesignState, ui: &egui::Ui) {
    let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
        i.raw
            .dropped_files
            .iter()
            .map(|file| file.path().to_path_buf())
            .collect()
    });
    if dropped.is_empty() {
        return;
    }

    let Some(path) = dropped.into_iter().find(|path| is_supported_image(path)) else {
        state.doc.status = "Drop a PNG, JPEG or WebP image.".into();
        return;
    };
    let target = default_photo_target(state);
    import_photo(state, &path, target);
}

fn add_node(state: &mut DesignState, name: &str, kind: Kind, rect: Rect) {
    state.history.checkpoint(&state.doc);
    let id = state.doc.add(name, kind, rect);
    state.selected = Some(id);
    state.rasters.sync(&state.doc);
}

fn layers(state: &mut DesignState, ui: &mut egui::Ui) {
    ui.label(RichText::new("Layers").strong().size(12.0));

    let selected = state.selected;
    let mut select = None;
    let mut toggle = None;
    let mut delete = None;

    for node in state.doc.nodes.iter().rev() {
        ui.horizontal(|ui| {
            let eye = if node.visible { "on" } else { "off" };
            if ui.small_button(eye).on_hover_text("Show / hide").clicked() {
                toggle = Some(node.id);
            }
            let label = format!("{}  {}", glyph(&node.kind), node.name);
            if ui
                .selectable_label(selected == Some(node.id), label)
                .clicked()
            {
                select = Some(node.id);
            }
            if ui.small_button("x").on_hover_text("Delete layer").clicked() {
                delete = Some(node.id);
            }
        });
    }

    if let Some(id) = select {
        state.selected = Some(id);
    }
    if let Some(id) = toggle {
        if let Some(node) = state.doc.node_mut(id) {
            node.visible = !node.visible;
        }
    }
    if let Some(id) = delete {
        state.history.checkpoint(&state.doc);
        state.doc.remove(id);
        if state.selected == Some(id) {
            state.selected = None;
        }
        state.rasters.sync(&state.doc);
    }
}

fn glyph(kind: &Kind) -> &'static str {
    match kind {
        Kind::Frame { .. } => "■",
        Kind::Rect { .. } => "■",
        Kind::Text { .. } => "T",
        Kind::Photo { .. } => "🖼",
        Kind::Paint { .. } => "✒",
    }
}

fn inspector(state: &mut DesignState, ui: &mut egui::Ui, persona: Persona) {
    egui::Panel::right("design_inspector")
        .exact_size(260.0)
        .frame(egui::Frame::new().fill(PANEL_BG))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Inspector")
                            .strong()
                            .size(13.0)
                            .color(Color32::WHITE),
                    );
                    ui.separator();

                    match state.selected {
                        Some(id) => node_inspector(state, ui, id, persona),
                        None => {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new("Nothing selected.")
                                    .size(12.0)
                                    .color(TEXT_DIM),
                            );
                            ui.label(
                                RichText::new("Pick a layer on the left or click the canvas.")
                                    .size(11.0)
                                    .color(TEXT_DIM),
                            );
                        }
                    }

                    ui.add_space(16.0);
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "Document  {} × {}",
                            state.doc.canvas_w, state.doc.canvas_h
                        ))
                        .size(11.0)
                        .color(TEXT_DIM),
                    );
                    ui.label(
                        RichText::new("Same file for all three personas.")
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                });
        });
}

fn node_inspector(state: &mut DesignState, ui: &mut egui::Ui, id: NodeId, persona: Persona) {
    let mut delete = false;
    let mut clear_paint = false;
    let mut reset_photo = false;
    let mut replace_photo = false;

    if let Some(node) = state.doc.node_mut(id) {
        ui.add_space(4.0);
        ui.label(
            RichText::new(node.kind.type_name())
                .size(11.0)
                .color(ACCENT),
        );
        ui.add(egui::TextEdit::singleline(&mut node.name));
        ui.checkbox(&mut node.visible, "Visible");
        ui.add(egui::Slider::new(&mut node.opacity, 0.0..=1.0).text("Opacity"));

        ui.add_space(6.0);
        ui.separator();

        egui::Grid::new("geometry")
            .num_columns(4)
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut node.rect.x).speed(1.0));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut node.rect.y).speed(1.0));
                ui.end_row();
                ui.label("W");
                ui.add(egui::DragValue::new(&mut node.rect.w).speed(1.0));
                ui.label("H");
                ui.add(egui::DragValue::new(&mut node.rect.h).speed(1.0));
                ui.end_row();
            });
        node.rect.w = node.rect.w.max(4.0);
        node.rect.h = node.rect.h.max(4.0);

        ui.separator();

        match &mut node.kind {
            Kind::Frame { fill } => {
                color_row(ui, "Fill", fill);
            }
            Kind::Rect {
                fill,
                radius,
                stroke,
                stroke_color,
            } => {
                color_row(ui, "Fill", fill);
                ui.add(egui::Slider::new(radius, 0.0..=200.0).text("Corners"));
                ui.add(egui::Slider::new(stroke, 0.0..=40.0).text("Stroke"));
                if *stroke > 0.0 {
                    color_row(ui, "Stroke colour", stroke_color);
                }
                if persona == Persona::Vector {
                    ui.label(
                        RichText::new("Vector view: fill, corners and stroke.")
                            .size(10.0)
                            .color(TEXT_DIM),
                    );
                }
            }
            Kind::Text { text, size, color } => {
                ui.label("Text");
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.label("Size");
                    ui.add(egui::DragValue::new(size).speed(1.0));
                });
                *size = size.max(4.0);
                color_row(ui, "Colour", color);
            }
            Kind::Photo { source, px } => {
                let what = match source {
                    super::model::PhotoSource::Sample => "Generated sample",
                    super::model::PhotoSource::File(path) => path.as_str(),
                };
                ui.label(RichText::new(what).size(11.0).color(TEXT_DIM));
                ui.label(
                    RichText::new(format!("{} × {} px", px.0, px.1))
                        .size(11.0)
                        .color(TEXT_DIM),
                );
                if ui.button("Replace photo...").clicked() {
                    replace_photo = true;
                }
                if ui.button("Reset to sample").clicked() {
                    reset_photo = true;
                }
                ui.label(
                    RichText::new("Select this layer in Pixel, then erase to cut it out.")
                        .size(10.0)
                        .color(TEXT_DIM),
                );
            }
            Kind::Paint { px } => {
                ui.label(
                    RichText::new(format!("{} × {} px · transparent", px.0, px.1))
                        .size(11.0)
                        .color(TEXT_DIM),
                );
                if ui.button("Clear paint").clicked() {
                    clear_paint = true;
                }
            }
        }

        ui.add_space(12.0);
        ui.separator();
        if ui.button("Delete layer").clicked() {
            delete = true;
        }
    }

    if replace_photo {
        if let Some(path) = pick_photo() {
            import_photo(state, &path, Some(id));
        }
    }
    if reset_photo {
        state.history.checkpoint(&state.doc);
        let px = (480, 640);
        if let Some(node) = state.doc.node_mut(id) {
            node.kind = Kind::Photo {
                source: PhotoSource::Sample,
                px,
            };
            node.rect = fit_rect(node.rect, px.0 as f32 / px.1 as f32);
        }
        state
            .rasters
            .map
            .insert(id, Raster::sample_photo(px.0, px.1));
        state.doc.status = "Reset photo to sample".into();
    }
    if clear_paint {
        if let Some(raster) = state.rasters.map.get_mut(&id) {
            raster.clear();
        }
    }
    if delete {
        state.history.checkpoint(&state.doc);
        state.doc.remove(id);
        state.selected = None;
        state.rasters.sync(&state.doc);
    }
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut Rgba) {
    ui.horizontal(|ui| {
        let mut c = color.color32();
        if ui.color_edit_button_srgba(&mut c).changed() {
            *color = Rgba::from_color32(c);
        }
        ui.label(label);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_png(w: u32, h: u32) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "creativecraft-import-{}-{}.png",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Raster::transparent(w, h).save_png(&path).unwrap();
        path
    }

    fn sample_photo(state: &DesignState) -> NodeId {
        state
            .doc
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    &node.kind,
                    Kind::Photo {
                        source: PhotoSource::Sample,
                        ..
                    }
                )
            })
            .map(|node| node.id)
            .unwrap()
    }

    #[test]
    fn fit_rect_keeps_aspect_and_centres() {
        let bounds = Rect::new(10.0, 20.0, 100.0, 100.0);

        let wide = fit_rect(bounds, 2.0);
        assert!((wide.w - 100.0).abs() < 0.01);
        assert!((wide.h - 50.0).abs() < 0.01);
        assert!((wide.y - 45.0).abs() < 0.01);

        let tall = fit_rect(bounds, 0.5);
        assert!((tall.w - 50.0).abs() < 0.01);
        assert!((tall.h - 100.0).abs() < 0.01);
        assert!((tall.x - 35.0).abs() < 0.01);
    }

    #[test]
    fn import_adds_a_photo_without_distorting_it() {
        let path = temp_png(300, 100);
        let mut state = DesignState::new();
        let before = state.doc.nodes.len();

        import_photo(&mut state, &path, None);

        assert_eq!(state.doc.nodes.len(), before + 1);
        let id = state.selected.expect("imported photo is selected");
        let node = state.doc.node(id).expect("imported node exists");
        assert!(matches!(&node.kind, Kind::Photo { .. }));
        assert!((node.rect.w / node.rect.h - 3.0).abs() < 0.01);

        let raster = state.rasters.map.get(&id).expect("raster exists");
        assert_eq!((raster.w, raster.h), (300, 100));

        state.undo();
        assert_eq!(state.doc.nodes.len(), before);
        state.redo();
        assert_eq!(state.doc.nodes.len(), before + 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn import_replaces_the_sample_photo_and_undo_restores_it() {
        let path = temp_png(300, 100);
        let mut state = DesignState::sample();
        let id = sample_photo(&state);
        let before = state.doc.nodes.len();

        import_photo(&mut state, &path, Some(id));

        assert_eq!(state.doc.nodes.len(), before);
        assert_eq!(state.selected, Some(id));
        let node = state.doc.node(id).expect("photo still exists");
        assert!(matches!(
            &node.kind,
            Kind::Photo {
                source: PhotoSource::File(_),
                ..
            }
        ));
        assert!((node.rect.w / node.rect.h - 3.0).abs() < 0.01);

        state.undo();

        let node = state.doc.node(id).expect("photo survives undo");
        assert!(matches!(
            &node.kind,
            Kind::Photo {
                source: PhotoSource::Sample,
                ..
            }
        ));
        assert_eq!(
            state.rasters.map.get(&id).map(|r| (r.w, r.h)),
            Some((480, 640))
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn downscale_keeps_aspect() {
        let raster = Raster::transparent(100, 50).downscale(10);
        assert_eq!((raster.w, raster.h), (10, 5));

        let untouched = Raster::transparent(20, 10).downscale(2048);
        assert_eq!((untouched.w, untouched.h), (20, 10));
    }

    #[derive(Debug)]
    struct FakeDrop(PathBuf);

    impl egui::DroppedFile for FakeDrop {
        fn path(&self) -> &Path {
            &self.0
        }

        fn bytes(&self) -> Result<Vec<u8>, String> {
            std::fs::read(&self.0).map_err(|error| error.to_string())
        }
    }

    #[test]
    fn a_dropped_image_is_imported_once() {
        let path = temp_png(120, 80);
        let mut state = DesignState::new();
        // Remove the demo portrait so this import has to add a layer, which
        // makes a duplicate import visible as an extra node.
        state
            .doc
            .nodes
            .retain(|node| !matches!(&node.kind, Kind::Photo { .. }));
        state.rasters.sync(&state.doc);
        let before = state.doc.nodes.len();

        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 700.0),
            )),
            dropped_files: vec![std::sync::Arc::new(FakeDrop(path.clone()))],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(raw, |ui| {
            dropped_photos(&mut state, ui);
        });
        output.textures_delta.clear();
        assert_eq!(state.doc.nodes.len(), before + 1);

        // The raw input is consumed, so another frame must not import it again.
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            dropped_photos(&mut state, ui);
        });
        output.textures_delta.clear();
        assert_eq!(state.doc.nodes.len(), before + 1);

        let _ = std::fs::remove_file(path);
    }
}
