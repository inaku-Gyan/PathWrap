use crate::app::PathWarpApp;
use egui::{Context, Key};

fn filtered_paths(paths: &[String], search_query: &str) -> Vec<String> {
    let query_lower = search_query.to_lowercase();
    paths
        .iter()
        .filter(|p| p.to_lowercase().contains(&query_lower))
        .cloned()
        .collect()
}

fn normalized_selected_index(selected_index: usize, filtered_len: usize) -> usize {
    if filtered_len == 0 {
        0
    } else {
        selected_index.min(filtered_len.saturating_sub(1))
    }
}

fn next_selected_index(
    selected_index: usize,
    filtered_len: usize,
    arrow_up_pressed: bool,
    arrow_down_pressed: bool,
) -> usize {
    let mut next = normalized_selected_index(selected_index, filtered_len);
    if arrow_up_pressed {
        next = next.saturating_sub(1);
    }
    if arrow_down_pressed {
        next = (next + 1).min(filtered_len.saturating_sub(1));
    }
    next
}

#[cfg(test)]
pub fn test_filtered_paths(paths: &[String], search_query: &str) -> Vec<String> {
    filtered_paths(paths, search_query)
}

#[cfg(test)]
pub fn test_normalized_selected_index(selected_index: usize, filtered_len: usize) -> usize {
    normalized_selected_index(selected_index, filtered_len)
}

#[cfg(test)]
pub fn test_next_selected_index(
    selected_index: usize,
    filtered_len: usize,
    arrow_up_pressed: bool,
    arrow_down_pressed: bool,
) -> usize {
    next_selected_index(
        selected_index,
        filtered_len,
        arrow_up_pressed,
        arrow_down_pressed,
    )
}

pub fn render(ctx: &Context, app: &mut PathWarpApp) {
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.hide_overlay_by_user(ctx);
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

            let filtered_paths = filtered_paths(&app.paths, &app.search_query);
            app.selected_index = next_selected_index(
                app.selected_index,
                filtered_paths.len(),
                ctx.input(|i| i.key_pressed(Key::ArrowUp)),
                ctx.input(|i| i.key_pressed(Key::ArrowDown)),
            );

            if ctx.input(|i| i.key_pressed(Key::Enter))
                && let Some(selected) = filtered_paths.get(app.selected_index)
                && let Some(dialog) = app.target_dialog
            {
                crate::os::dialog::inject_folder_path(dialog.hwnd, selected.as_str());
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
                            if let Some(dialog) = app.target_dialog {
                                crate::os::dialog::inject_folder_path(dialog.hwnd, path.as_str());
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
