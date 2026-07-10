use crate::app::PathWarpApp;
use egui::Context;

/// 本帧来自键盘钩子的导航/确认意图（Char/Backspace 已由 app 直接消费）。
#[derive(Debug, Default, Clone, Copy)]
pub struct FrameIntents {
    pub nav_up: bool,
    pub nav_down: bool,
    pub confirm: bool,
}

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

fn handle_path_item_interaction(
    selected_index: usize,
    item_index: usize,
    clicked: bool,
    double_clicked: bool,
    has_target_dialog: bool,
) -> (usize, bool) {
    let next_selected_index = if clicked { item_index } else { selected_index };
    let should_inject = double_clicked && has_target_dialog;
    (next_selected_index, should_inject)
}

pub fn render(ctx: &Context, app: &mut PathWarpApp, intents: FrameIntents) {
    let filtered = filtered_paths(&app.paths, &app.search_query);
    app.selected_index = next_selected_index(
        app.selected_index,
        filtered.len(),
        intents.nav_up,
        intents.nav_down,
    );

    // 收集本帧要注入的目标，实际注入放到闭包外执行，避免与 UI 借用冲突。
    let mut inject_target: Option<(isize, String)> = None;

    if intents.confirm
        && let Some(selected) = filtered.get(app.selected_index)
        && let Some(dialog) = app.target_dialog
    {
        inject_target = Some((dialog.hwnd, selected.clone()));
    }

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(20, 20, 20, 240))
                .inner_margin(10.0),
        )
        .show(ctx, |ui| {
            // 搜索行为纯展示：输入来自全局键盘钩子，而非 egui 焦点。
            ui.label(format!("🔎 {}", app.search_query));
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    for (idx, path) in filtered.iter().enumerate() {
                        let is_selected = idx == app.selected_index;
                        let label = egui::SelectableLabel::new(is_selected, path.as_str());
                        let response = ui.add(label);
                        let (next_idx, should_inject) = handle_path_item_interaction(
                            app.selected_index,
                            idx,
                            response.clicked(),
                            response.double_clicked(),
                            app.target_dialog.is_some(),
                        );
                        app.selected_index = next_idx;
                        if should_inject && let Some(dialog) = app.target_dialog {
                            inject_target = Some((dialog.hwnd, path.clone()));
                        }
                    }
                });
            });
        });

    if let Some((hwnd, path)) = inject_target {
        crate::os::dialog::inject_folder_path(hwnd, &path);
        // 选定后隐藏并抑制当前对话框会话，避免立即被重新拉起。
        app.hide_overlay_by_user();
    }
}

/* Below is the unit test module. **/
// Read about unit testing in Rust: https://doc.rust-lang.org/book/ch11-03-test-organization.html#unit-tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_paths_case_insensitively() {
        let paths = vec![
            "C:\\Work".to_string(),
            "D:\\Games".to_string(),
            "C:\\workspace\\PathWarp".to_string(),
        ];

        let result = filtered_paths(&paths, "WORK");
        assert_eq!(
            result,
            vec![
                "C:\\Work".to_string(),
                "C:\\workspace\\PathWarp".to_string()
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

    #[test]
    fn single_click_only_changes_selection_without_injection() {
        let (next_idx, should_inject) = handle_path_item_interaction(0, 2, true, false, true);
        assert_eq!(next_idx, 2);
        assert!(!should_inject);
    }

    #[test]
    fn double_click_triggers_injection_when_dialog_exists() {
        let (next_idx, should_inject) = handle_path_item_interaction(1, 1, true, true, true);
        assert_eq!(next_idx, 1);
        assert!(should_inject);
    }

    #[test]
    fn double_click_does_not_inject_without_dialog() {
        let (next_idx, should_inject) = handle_path_item_interaction(0, 0, true, true, false);
        assert_eq!(next_idx, 0);
        assert!(!should_inject);
    }
}
