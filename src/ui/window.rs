use crate::app::PathWarpApp;
use egui::{Context, Key};
use log::{debug, warn};

pub fn render(ctx: &Context, app: &mut PathWarpApp) {
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.hide_overlay(ctx);
        return;
    }

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(20, 20, 20, 240))
                .inner_margin(10.0),
        )
        .show(ctx, |ui| {
            let search_response = ui.text_edit_singleline(&mut app.search_query);
            search_response.request_focus();
            if search_response.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                search_response.request_focus();
            }

            let query_lower = app.search_query.to_lowercase();
            let filtered_paths: Vec<String> = app
                .paths
                .iter()
                .filter(|p| p.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();

            if !filtered_paths.is_empty() {
                app.selected_index = app
                    .selected_index
                    .min(filtered_paths.len().saturating_sub(1));
            } else {
                app.selected_index = 0;
            }

            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                app.selected_index = app.selected_index.saturating_sub(1);
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                app.selected_index =
                    (app.selected_index + 1).min(filtered_paths.len().saturating_sub(1));
            }

            if ctx.input(|i| i.key_pressed(Key::Enter))
                && let Some(selected) = filtered_paths.get(app.selected_index)
            {
                if let Some(dialog) = app.target_dialog {
                    match crate::os::dialog::inject_folder_path(dialog.hwnd, selected.as_str()) {
                        Ok(action) => {
                            debug!("path injected by {:?}: {}", action, selected);
                            app.hide_overlay(ctx);
                            return;
                        }
                        Err(err) => {
                            warn!("path inject failed: {}", err);
                        }
                    }
                }
            }

            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    for (idx, path) in filtered_paths.iter().enumerate() {
                        let is_selected = idx == app.selected_index;
                        let label = egui::SelectableLabel::new(is_selected, path.as_str());
                        let response = ui.add(label);
                        if response.clicked() {
                            app.selected_index = idx;
                        }

                        if response.double_clicked()
                            && let Some(dialog) = app.target_dialog
                        {
                            match crate::os::dialog::inject_folder_path(dialog.hwnd, path.as_str()) {
                                Ok(action) => {
                                    debug!("path injected by {:?}: {}", action, path);
                                    app.hide_overlay(ctx);
                                    return;
                                }
                                Err(err) => {
                                    warn!("path inject failed: {}", err);
                                }
                            }
                        }
                    }
                });
            });
        });

    // Handle background drag to move window without blocking clicks on children
    if ctx.input(|i| i.pointer.primary_down()) && !ctx.wants_pointer_input() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}
