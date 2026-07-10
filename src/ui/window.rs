//! 悬浮条的纯渲染器：只读 [`Controller`] 的模型快照绘制界面，把鼠标交互
//! 作为 [`UiEvent`] 回传给调用方（[`crate::app`]），自身不做任何状态决策。
//!
//! 键盘输入不经此处——非激活窗口拿不到键盘焦点，打字/导航由全局钩子驱动
//! 控制器（见 [`crate::os::input_hook`] 与 [`Controller`]）。

use crate::core::controller::Controller;
use egui::Ui;

/// 本帧产生的一次鼠标交互（下标为过滤后列表中的位置）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    ItemClicked(usize),
    ItemDoubleClicked(usize),
}

/// 渲染搜索行（纯展示胶囊：放大镜 + 查询文本/占位符 + 光标）。
fn render_search_row(ui: &mut Ui, query: &str) {
    crate::ui::theme::search_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🔎").color(crate::ui::theme::accent()));
            if query.is_empty() {
                ui.label(egui::RichText::new("输入以筛选路径…").weak());
            } else {
                ui.label(egui::RichText::new(format!("{query}▏")).strong());
            }
        });
    });
}

/// 渲染悬浮条。返回本帧发生的鼠标交互（若有）。
pub fn render(root: &mut Ui, controller: &Controller) -> Option<UiEvent> {
    let filtered = controller.filtered_paths();
    let selected = controller.selected_index();
    let mut event = None;

    egui::CentralPanel::default()
        .frame(crate::ui::theme::overlay_frame())
        .show(root, |ui| {
            render_search_row(ui, controller.query());
            ui.add_space(4.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    for (idx, path) in filtered.iter().enumerate() {
                        let is_selected = idx == selected;
                        let response = ui.add(egui::Button::selectable(is_selected, path.as_str()));
                        // 双击也会触发 clicked()，故先判双击。
                        if response.double_clicked() {
                            event = Some(UiEvent::ItemDoubleClicked(idx));
                        } else if response.clicked() {
                            event = Some(UiEvent::ItemClicked(idx));
                        }
                    }
                });
            });
        });

    event
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::core::controller::{Controller, Env, Event};
    use crate::core::types::KeyAction;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use std::time::Instant;

    /// 构造一个带路径、可选已输入查询的控制器（用于喂给纯渲染器）。
    fn controller_with(paths: &[&str], query: &str) -> Controller {
        let mut controller = Controller::new();
        controller.set_paths(paths.iter().map(|s| (*s).to_string()).collect());
        let env = Env {
            now: Instant::now(),
            foreground_hwnd: 1,
        };
        for ch in query.chars() {
            controller.step(env, Event::Key(KeyAction::Char(ch)));
        }
        controller
    }

    fn harness_for(controller: Controller) -> Harness<'static, (Controller, Option<UiEvent>)> {
        Harness::builder()
            .with_size(egui::vec2(420.0, 320.0))
            .build_ui_state(
                |ui, state: &mut (Controller, Option<UiEvent>)| {
                    // 点击在某一帧被消费，后续帧 render 返回 None；这里latch住首个非空事件。
                    if let Some(event) = render(ui, &state.0) {
                        state.1 = Some(event);
                    }
                },
                (controller, None),
            )
    }

    #[test]
    fn renders_only_filtered_paths() {
        let mut harness = harness_for(controller_with(&["C:\\Work", "D:\\Games"], "work"));
        harness.run();
        assert!(harness.query_by_label("C:\\Work").is_some());
        assert!(
            harness.query_by_label("D:\\Games").is_none(),
            "filtered-out path must not be rendered"
        );
    }

    #[test]
    fn clicking_item_emits_item_clicked_with_index() {
        let mut harness = harness_for(controller_with(&["C:\\Work", "D:\\Games"], ""));
        harness.run();
        harness.get_by_label("D:\\Games").click();
        harness.run();
        assert_eq!(harness.state().1, Some(UiEvent::ItemClicked(1)));
    }

    #[test]
    fn search_row_shows_placeholder_when_empty_and_query_when_typed() {
        // 空查询：显示占位符。
        let mut empty = harness_for(controller_with(&["C:\\Work"], ""));
        empty.run();
        assert!(empty.query_by_label_contains("输入以筛选").is_some());

        // 有查询：回显查询文本（含合成光标）。
        let mut typed = harness_for(controller_with(&["C:\\Work"], "wo"));
        typed.run();
        assert!(typed.query_by_label_contains("wo").is_some());
    }
}
