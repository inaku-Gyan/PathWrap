//! 悬浮条的纯渲染器：只读 [`Controller`] 的模型快照绘制界面，把鼠标交互
//! 作为 [`UiEvent`] 回传给调用方（[`crate::app`]），自身不做任何状态决策。
//!
//! 键盘输入不经此处——非激活窗口拿不到键盘焦点；用户点击搜索行或列表后，
//! 打字/导航由全局钩子驱动控制器（见 [`crate::os::input_hook`] 与 [`Controller`]）。

use crate::config::{ThemeMode, ThemePreference};
use crate::core::controller::Controller;
use egui::{CornerRadius, Stroke, StrokeKind, Ui};

const SEARCH_CONTROL_HEIGHT: f32 = 32.0;
const SEARCH_ICON_SIZE: f32 = 18.0;
const SEARCH_ICON_GAP: f32 = 10.0;
const SEARCH_TEXT_SIZE: f32 = 16.0;
const THEME_BUTTON_WIDTH: f32 = 30.0;
const THEME_ICON_SIZE: f32 = 18.0;

/// 本帧产生的一次鼠标交互（下标为过滤后列表中的位置）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    /// 用户点击搜索行，显式把后续键盘输入交给悬浮层筛选。
    Search,
    Item(usize),
    ItemDouble(usize),
    Theme(ThemeAction),
}

/// A theme interaction emitted by the compact header control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeAction {
    /// Switch between the two explicit palettes.
    Toggle,
    /// Select one of the persisted theme preferences from the context menu.
    Set(ThemePreference),
}

/// Render the search row and the compact theme control in one visual header.
fn render_header(
    ui: &mut Ui,
    query: &str,
    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
    capture_active: bool,
) -> Option<UiEvent> {
    let row = ui.horizontal(|ui| {
        // Keep the control at the trailing edge without introducing a second
        // toolbar row or changing the overlay's fixed height. The search frame
        // is allocated separately so the theme button no longer sits inside its
        // large rounded border.
        let search_width =
            (ui.available_width() - THEME_BUTTON_WIDTH - ui.spacing().item_spacing.x).max(0.0);
        let search_frame = crate::ui::theme::search_frame(ui.ctx());
        let frame_margin = search_frame.total_margin();
        let search_frame_height = SEARCH_CONTROL_HEIGHT + frame_margin.top + frame_margin.bottom;
        let search = ui.allocate_ui_with_layout(
            egui::vec2(search_width, search_frame_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let frame_response = search_frame.show(ui, |ui| {
                    let (search_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), SEARCH_CONTROL_HEIGHT),
                        egui::Sense::empty(),
                    );
                    let icon_rect = egui::Rect::from_min_size(
                        search_rect.min,
                        egui::vec2(SEARCH_ICON_SIZE, search_rect.height()),
                    );
                    paint_search_icon(ui, icon_rect);
                    let text_rect = egui::Rect::from_min_max(
                        egui::pos2(
                            search_rect.left() + SEARCH_ICON_SIZE + SEARCH_ICON_GAP,
                            search_rect.top(),
                        ),
                        search_rect.max,
                    );
                    let mut content_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(text_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    if query.is_empty() {
                        if capture_active {
                            content_ui.add(
                                egui::Label::new(
                                    egui::RichText::new("▏")
                                        .size(SEARCH_TEXT_SIZE)
                                        .color(crate::ui::theme::accent(ui.ctx())),
                                )
                                .sense(egui::Sense::empty()),
                            );
                        }
                        content_ui.add(
                            egui::Label::new(
                                egui::RichText::new("点击后输入以筛选路径…")
                                    .size(SEARCH_TEXT_SIZE)
                                    .weak(),
                            )
                            .sense(egui::Sense::empty()),
                        );
                    } else {
                        content_ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("{query}▏"))
                                    .size(SEARCH_TEXT_SIZE)
                                    .strong(),
                            )
                            .sense(egui::Sense::empty()),
                        );
                    }
                    ui.interact(
                        search_rect,
                        ui.id().with("path_filter_search"),
                        egui::Sense::click(),
                    )
                });
                (frame_response.response, frame_response.inner)
            },
        );
        let theme_event = render_theme_button(ui, theme_preference, theme_mode);
        let (frame_response, search_response) = search.inner;
        (frame_response, search_response, theme_event)
    });

    let (frame_response, search_response, theme_event) = row.inner;
    // The Frame response includes its padding and stroke; use it for the
    // visible search-frame ring instead of the smaller clickable search rect.
    let focused = capture_active || search_response.has_focus();
    if search_response.hovered() || focused {
        let stroke = if focused {
            Stroke::new(2.0, crate::ui::theme::focus(ui.ctx()))
        } else {
            Stroke::new(1.0, crate::ui::theme::focus(ui.ctx()).gamma_multiply(0.55))
        };
        ui.painter().rect_stroke(
            frame_response.rect,
            CornerRadius::same(6),
            stroke,
            StrokeKind::Inside,
        );
    }

    theme_event.or_else(|| search_response.clicked().then_some(UiEvent::Search))
}

fn render_theme_button(
    ui: &mut Ui,
    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
) -> Option<UiEvent> {
    let description = match theme_preference {
        ThemePreference::Auto => format!(
            "跟随系统（当前{}）。左键切换浅色/深色，右键选择主题",
            theme_mode.label()
        ),
        preference => format!(
            "{}模式。左键切换浅色/深色，右键选择主题",
            preference.label()
        ),
    };

    let response = ui
        .add(egui::Button::new("").min_size(egui::vec2(THEME_BUTTON_WIDTH, 28.0)))
        .on_hover_text(description.clone());
    paint_theme_icon(ui, response.rect, theme_mode);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, description.clone())
    });

    let mut action = response
        .clicked()
        .then_some(UiEvent::Theme(ThemeAction::Toggle));
    response.context_menu(|ui| {
        ui.set_min_width(132.0);
        ui.label("主题");
        for preference in [
            ThemePreference::Auto,
            ThemePreference::Light,
            ThemePreference::Dark,
        ] {
            if ui
                .selectable_label(theme_preference == preference, preference.label())
                .clicked()
            {
                action = Some(UiEvent::Theme(ThemeAction::Set(preference)));
                ui.close();
            }
        }
    });

    if response.hovered() || response.has_focus() || response.context_menu_opened() {
        ui.painter().rect_stroke(
            response.rect,
            CornerRadius::same(6),
            Stroke::new(1.0, crate::ui::theme::focus(ui.ctx())),
            StrokeKind::Inside,
        );
    }
    action
}

fn paint_search_icon(ui: &Ui, rect: egui::Rect) {
    let center = rect.center() + egui::vec2(-1.5, -1.5);
    let stroke = Stroke::new(1.8, crate::ui::theme::accent(ui.ctx()));
    ui.painter().circle_stroke(center, 5.0, stroke);
    ui.painter().line_segment(
        [center + egui::vec2(3.5, 3.5), center + egui::vec2(8.0, 8.0)],
        stroke,
    );
}

fn paint_theme_icon(ui: &Ui, rect: egui::Rect, mode: ThemeMode) {
    let center = rect.center();
    let stroke = Stroke::new(1.5, crate::ui::theme::accent(ui.ctx()));
    let radius = THEME_ICON_SIZE * 0.39;
    match mode {
        ThemeMode::Light => {
            ui.painter().circle_stroke(center, radius * 0.57, stroke);
            for index in 0..8 {
                let angle = index as f32 * std::f32::consts::TAU / 8.0;
                let direction = egui::Vec2::angled(angle);
                ui.painter().line_segment(
                    [
                        center + direction * radius,
                        center + direction * (radius * 1.36),
                    ],
                    stroke,
                );
            }
        }
        ThemeMode::Dark => {
            let outer = arc_points(
                center,
                radius,
                -std::f32::consts::FRAC_PI_2,
                -3.0 * std::f32::consts::FRAC_PI_2,
                16,
            );
            let inner_center = center + egui::vec2(radius * 0.43, -0.5);
            let inner = arc_points(
                inner_center,
                radius * 0.79,
                std::f32::consts::FRAC_PI_2,
                -std::f32::consts::FRAC_PI_2,
                16,
            );
            ui.painter().add(egui::Shape::line(outer.clone(), stroke));
            ui.painter().add(egui::Shape::line(inner.clone(), stroke));
            ui.painter()
                .line_segment([outer[0], inner[inner.len() - 1]], stroke);
            ui.painter()
                .line_segment([outer[outer.len() - 1], inner[0]], stroke);
        }
    }
}

fn arc_points(
    center: egui::Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    segments: usize,
) -> Vec<egui::Pos2> {
    (0..=segments)
        .map(|index| {
            let progress = index as f32 / segments as f32;
            let angle = start_angle + (end_angle - start_angle) * progress;
            center + egui::Vec2::angled(angle) * radius
        })
        .collect()
}

/// 渲染悬浮条。返回本帧发生的鼠标交互（若有）。
#[allow(dead_code)]
pub fn render(root: &mut Ui, controller: &Controller) -> Option<UiEvent> {
    render_with_theme(root, controller, ThemePreference::Auto, ThemeMode::Dark)
}

/// Render the overlay using the preference and resolved mode owned by the app shell.
pub fn render_with_theme(
    root: &mut Ui,
    controller: &Controller,
    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
) -> Option<UiEvent> {
    render_with_theme_and_capture(root, controller, theme_preference, theme_mode, false)
}

/// Render the overlay with the non-activating window's explicit input-capture state.
pub fn render_with_theme_and_capture(
    root: &mut Ui,
    controller: &Controller,
    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
    capture_active: bool,
) -> Option<UiEvent> {
    let filtered = controller.filtered_paths();
    let selected = controller.selected_index();
    let mut event = None;

    egui::CentralPanel::default()
        .frame(crate::ui::theme::overlay_frame(root.ctx()))
        .show(root, |ui| {
            if let Some(header_event) = render_header(
                ui,
                controller.query(),
                theme_preference,
                theme_mode,
                capture_active,
            ) {
                event = Some(header_event);
            }
            ui.add_space(4.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    for (idx, path) in filtered.iter().enumerate() {
                        let is_selected = idx == selected;
                        let response = ui.add(egui::Button::selectable(is_selected, path.as_str()));
                        if is_selected {
                            let rect = response.rect;
                            let accent = crate::ui::theme::selection_stroke(ui.ctx());
                            ui.painter().rect_stroke(
                                rect,
                                CornerRadius::same(6),
                                Stroke::new(1.0, accent),
                                StrokeKind::Inside,
                            );
                            ui.painter().rect_filled(
                                egui::Rect::from_min_max(
                                    egui::pos2(rect.left(), rect.top() + 4.0),
                                    egui::pos2(rect.left() + 3.0, rect.bottom() - 4.0),
                                ),
                                CornerRadius::same(2),
                                accent,
                            );
                        }
                        // 双击也会触发 clicked()，故先判双击。
                        if response.double_clicked() {
                            event = Some(UiEvent::ItemDouble(idx));
                        } else if response.clicked() {
                            event = Some(UiEvent::Item(idx));
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

    fn capture_harness_for(controller: Controller) -> Harness<'static, (Controller, bool)> {
        Harness::builder()
            .with_size(egui::vec2(420.0, 320.0))
            .build_ui_state(
                |ui, state: &mut (Controller, bool)| {
                    if matches!(
                        render_with_theme_and_capture(
                            ui,
                            &state.0,
                            ThemePreference::Auto,
                            ThemeMode::Dark,
                            state.1,
                        ),
                        Some(UiEvent::Search)
                    ) {
                        state.1 = true;
                    }
                },
                (controller, false),
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
        assert_eq!(harness.state().1, Some(UiEvent::Item(1)));
    }

    #[test]
    fn clicking_search_row_emits_search_clicked() {
        let mut harness = harness_for(controller_with(&["C:\\Work"], ""));
        harness.run();
        harness
            .query_by_label_contains("点击后输入以筛选")
            .expect("search row placeholder")
            .click();
        harness.run();
        assert_eq!(harness.state().1, Some(UiEvent::Search));
    }

    #[test]
    fn captured_search_shows_caret_after_click() {
        let mut harness = capture_harness_for(controller_with(&["C:\\Work"], ""));
        harness.run();
        harness
            .query_by_label_contains("点击后输入以筛选")
            .expect("search row placeholder")
            .click();
        harness.run();
        assert!(
            harness.query_by_label_contains("▏").is_some(),
            "captured empty search must show its caret"
        );
    }

    #[test]
    fn clicking_theme_button_emits_toggle() {
        let mut harness = harness_for(controller_with(&["C:\\Work"], ""));
        harness.run();
        let theme = harness
            .query_by_label_contains("跟随系统（当前深色）")
            .expect("theme button accessibility label");
        theme.click();
        harness.run();
        assert_eq!(harness.state().1, Some(UiEvent::Theme(ThemeAction::Toggle)));
    }

    #[test]
    fn theme_context_menu_can_select_light_mode() {
        let mut harness = harness_for(controller_with(&["C:\\Work"], ""));
        harness.run();
        harness
            .query_by_label_contains("跟随系统（当前深色）")
            .expect("theme button accessibility label")
            .click_secondary();
        harness.run();
        harness.get_by_label("浅色").click();
        harness.run();
        assert_eq!(
            harness.state().1,
            Some(UiEvent::Theme(ThemeAction::Set(ThemePreference::Light)))
        );
    }

    #[test]
    fn search_row_shows_placeholder_when_empty_and_query_when_typed() {
        // 空查询：显示占位符。
        let mut empty = harness_for(controller_with(&["C:\\Work"], ""));
        empty.run();
        assert!(empty.query_by_label_contains("点击后输入以筛选").is_some());

        // 有查询：回显查询文本（含合成光标）。
        let mut typed = harness_for(controller_with(&["C:\\Work"], "wo"));
        typed.run();
        assert!(typed.query_by_label_contains("wo").is_some());
    }
}
