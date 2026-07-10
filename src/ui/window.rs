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
