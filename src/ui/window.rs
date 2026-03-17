use crate::app::PathWarpApp;
use egui::{Context, Key};

pub fn render(ctx: &Context, app: &mut PathWarpApp) {
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close); // 按照任务要求隐藏/关闭
    }

    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(20, 20, 20, 240))
                .inner_margin(10.0),
        )
        .show(ctx, |ui| {
            // Search bar
            let search_response = ui.text_edit_singleline(&mut app.search_query);
            search_response.request_focus(); // Always focused
            if search_response.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter)) {
                search_response.request_focus();
            }

            // Filter items
            let query_lower = app.search_query.to_lowercase();
            let filtered_paths: Vec<&String> = app
                .paths
                .iter()
                .filter(|p| p.to_lowercase().contains(&query_lower))
                .collect();

            // Ensure selected index is within bounds
            if !filtered_paths.is_empty() {
                app.selected_index = app
                    .selected_index
                    .min(filtered_paths.len().saturating_sub(1));
            }

            // Keyboard navigation
            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                app.selected_index = app.selected_index.saturating_sub(1);
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                app.selected_index =
                    (app.selected_index + 1).min(filtered_paths.len().saturating_sub(1));
            }
            if ctx.input(|i| i.key_pressed(Key::Enter))
                && let Some(selected) = filtered_paths.get(app.selected_index) {
                    println!("Selected path: {}", selected); // Task 2.2 asks to print for now
                }

            ui.separator();

            // List View
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (idx, path) in filtered_paths.iter().enumerate() {
                    let is_selected = idx == app.selected_index;
                    let label_text = path.as_str();

                    // Make the selectable label fill the available width
                    let mut rect = ui.available_rect_before_wrap();
                    // We need a proper height for the label, so let's use standard interact size
                    rect.max.y = rect.min.y + ui.spacing().interact_size.y;
                    
                    let label = egui::SelectableLabel::new(is_selected, label_text);
                    let response = ui.add_sized([ui.available_width(), ui.spacing().interact_size.y], label);
                    
                    if response.clicked() {
                        app.selected_index = idx;
                        println!("Selected path: {}", path);
                    }
                }
            });
        });

    // Handle background drag to move window without blocking clicks on children
    if ctx.input(|i| i.pointer.primary_down()) {
        if !ctx.wants_pointer_input() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }
}
