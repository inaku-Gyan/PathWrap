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

/** Below is the unit test module. **/
// Read about unit testing in Rust: https://doc.rust-lang.org/book/ch11-03-test-organization.html#unit-tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_paths_case_insensitively() {
        let paths = vec![
            "C:\\Work".to_string(),
            "D:\\Games".to_string(),
            "C:\\workspace\\PathWrap".to_string(),
        ];

        let result = filtered_paths(&paths, "WORK");
        assert_eq!(
            result,
            vec![
                "C:\\Work".to_string(),
                "C:\\workspace\\PathWrap".to_string()
            ]
        );
    }

    #[test]
    fn keeps_all_paths_when_query_empty() {
        let paths = vec!["A".to_string(), "B".to_string()];
        assert_eq!(filtered_paths(&paths, ""), paths);
    }

    #[test]
    fn normalizes_selected_index_when_out_of_range() {
        assert_eq!(normalized_selected_index(10, 3), 2);
        assert_eq!(normalized_selected_index(1, 0), 0);
    }

    #[test]
    fn moves_selection_with_bounds() {
        assert_eq!(next_selected_index(0, 3, true, false), 0);
        assert_eq!(next_selected_index(0, 3, false, true), 1);
        assert_eq!(next_selected_index(2, 3, false, true), 2);
        assert_eq!(next_selected_index(5, 3, false, false), 2);
        assert_eq!(next_selected_index(0, 0, false, true), 0);
    }
}
