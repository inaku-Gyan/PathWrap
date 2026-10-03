//! 悬浮条渲染器：把查询直接绑定到 [`Controller`] 的编辑缓冲区，把鼠标和导航
//! 交互作为 [`UiEvent`] 回传给调用方（[`crate::app`]）。窗口允许正常激活，
//! 因而 `egui::TextEdit` 负责系统输入事件、选区、剪贴板、输入法和闪烁 caret。

use crate::config::{ThemeMode, ThemePreference};
use crate::core::controller::Controller;
use crate::core::types::KeyAction;
use egui::{Align, CornerRadius, FontId, Margin, Stroke, StrokeKind, Ui};

const SEARCH_CONTROL_HEIGHT: f32 = 32.0;
const SEARCH_ICON_SIZE: f32 = 18.0;
const SEARCH_ICON_GAP: f32 = 10.0;
const SEARCH_TEXT_SIZE: f32 = 16.0;
const THEME_BUTTON_WIDTH: f32 = 30.0;
const THEME_ICON_SIZE: f32 = 18.0;

/// 本帧产生的一次鼠标交互（下标为过滤后列表中的位置）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEvent {
    /// 用户点击搜索行，确保搜索框获得 egui 焦点。
    Search,
    /// 文本编辑控件产生的导航/确认/收起动作。
    Key(KeyAction),
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
    query: &mut String,
    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
    editor_id: &mut Option<egui::Id>,
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
                    let edit_response = content_ui.add_sized(
                        text_rect.size(),
                        egui::TextEdit::singleline(query)
                            .id_salt("path_filter_edit")
                            .font(FontId::proportional(SEARCH_TEXT_SIZE))
                            .text_color(ui.visuals().text_color())
                            .hint_text("点击后输入以筛选路径…")
                            .frame(egui::Frame::NONE)
                            .margin(Margin::ZERO)
                            .vertical_align(Align::Center)
                            .return_key(None),
                    );
                    let search_response = ui.interact(
                        search_rect,
                        ui.id().with("path_filter_search"),
                        egui::Sense::click(),
                    );
                    if search_response.clicked() {
                        edit_response.request_focus();
                    }
                    (search_response, edit_response)
                });
                (frame_response.response, frame_response.inner)
            },
        );
        let theme_event = render_theme_button(ui, theme_preference, theme_mode);
        let (frame_response, search_response) = search.inner;
        (frame_response, search_response, theme_event)
    });

    let (frame_response, search_response, theme_event) = row.inner;
    *editor_id = Some(search_response.1.id);
    // The base frame remains neutral. Hovering the search area adds a subtle one-pixel
    // accent; focus is expressed by TextEdit's blinking caret instead of a thick outer ring.
    if search_response.0.hovered() {
        ui.painter().rect_stroke(
            frame_response.rect,
            CornerRadius::same(6),
            Stroke::new(1.0, crate::ui::theme::focus(ui.ctx()).gamma_multiply(0.55)),
            StrokeKind::Inside,
        );
    }

    let key_event = search_response.1.has_focus().then(|| {
        ui.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                Some(UiEvent::Key(KeyAction::Escape))
            } else if input.key_pressed(egui::Key::Enter) {
                Some(UiEvent::Key(KeyAction::Enter))
            } else if input.key_pressed(egui::Key::ArrowUp) {
                Some(UiEvent::Key(KeyAction::Up))
            } else if input.key_pressed(egui::Key::ArrowDown) {
                Some(UiEvent::Key(KeyAction::Down))
            } else {
                None
            }
        })
    });
    let key_event = key_event.flatten();

    // The overlay can receive focus even when the pointer lands on its blank background or
    // another custom-painted region. Restore the editor only when it is no longer the logical
    // target; repeating `request_focus` while it already owns focus would interrupt IME
    // composition in egui.
    let viewport_focused = ui.input(|input| input.focused);
    if (viewport_focused || theme_event.is_some()) && !search_response.1.has_focus() {
        search_response.1.request_focus();
    }

    theme_event
        .or(key_event)
        .or_else(|| search_response.0.clicked().then_some(UiEvent::Search))
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
pub fn render(root: &mut Ui, controller: &mut Controller) -> Option<UiEvent> {
    render_with_theme(root, controller, ThemePreference::Auto, ThemeMode::Dark)
}

/// Render the overlay using the preference and resolved mode owned by the app shell.
pub fn render_with_theme(
    root: &mut Ui,
    controller: &mut Controller,
    theme_preference: ThemePreference,
    theme_mode: ThemeMode,
) -> Option<UiEvent> {
    let mut event = None;
    let mut editor_id = None;

    egui::CentralPanel::default()
        .frame(crate::ui::theme::overlay_frame(root.ctx()))
        .show(root, |ui| {
            if let Some(header_event) = render_header(
                ui,
                controller.query_mut(),
                theme_preference,
                theme_mode,
                &mut editor_id,
            ) {
                event = Some(header_event);
            }
            // TextEdit mutates the controller's source buffer directly. Re-clamp the selected
            // row before laying out the filtered list so edits take effect in the same frame.
            controller.query_edited();
            let filtered = controller.filtered_paths();
            let selected = controller.selected_index();
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

    if matches!(event, Some(UiEvent::Item(_) | UiEvent::ItemDouble(_)))
        && let Some(editor_id) = editor_id
    {
        root.ctx()
            .memory_mut(|memory| memory.request_focus(editor_id));
    }

    event
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::core::controller::Controller;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// 构造一个带路径、可选已输入查询的控制器（用于喂给纯渲染器）。
    fn controller_with(paths: &[&str], query: &str) -> Controller {
        let mut controller = Controller::new();
        controller.set_paths(paths.iter().map(|s| (*s).to_string()).collect());
        controller.query_mut().push_str(query);
        controller.query_edited();
        controller
    }

    fn harness_for(controller: Controller) -> Harness<'static, (Controller, Option<UiEvent>)> {
        Harness::builder()
            .with_size(egui::vec2(420.0, 320.0))
            .build_ui_state(
                |ui, state: &mut (Controller, Option<UiEvent>)| {
                    // 点击在某一帧被消费，后续帧 render 返回 None；这里latch住首个非空事件。
                    if let Some(event) = render(ui, &mut state.0) {
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
        assert_eq!(harness.state().1, Some(UiEvent::Item(1)));
    }

    #[test]
    fn clicking_search_row_emits_search_clicked() {
        let mut harness = harness_for(controller_with(&["C:\\Work"], ""));
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .click();
        harness.run();
        assert_eq!(harness.state().1, Some(UiEvent::Search));
    }

    #[test]
    fn search_uses_text_edit_without_custom_caret() {
        let mut harness = harness_for(controller_with(&["C:\\Work"], ""));
        harness.run();
        let edit = harness.get_by_role(egui::accesskit::Role::TextInput);
        edit.click();
        harness.run();
        assert!(
            harness.query_by_label_contains("▏").is_none(),
            "the old hand-drawn caret must not be rendered"
        );
    }

    #[test]
    fn focused_overlay_keeps_editor_focus_over_background_clicks() {
        let mut harness = harness_for(controller_with(&["C:\\Work"], ""));
        harness.ctx.input_mut(|input| input.focused = true);
        harness.run();

        let blank = egui::pos2(410.0, 300.0);
        harness.event(egui::Event::PointerMoved(blank));
        harness.event(egui::Event::PointerButton {
            pos: blank,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
        harness.event(egui::Event::PointerButton {
            pos: blank,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run();

        assert!(
            harness
                .get_by_role(egui::accesskit::Role::TextInput)
                .is_focused(),
            "a focused overlay must keep its TextEdit as the keyboard target"
        );
    }

    #[test]
    fn active_ime_composition_is_not_interrupted_by_focus_maintenance() {
        let mut harness = harness_for(controller_with(&["C:\\Work"], ""));
        harness.ctx.input_mut(|input| input.focused = true);
        harness.run();

        harness.event(egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "ni".to_owned(),
            active_range_chars: Some(0..2),
        }));
        harness.run();

        assert_eq!(harness.state().0.query(), "ni");

        let ime = harness
            .output()
            .platform_output
            .ime
            .as_ref()
            .expect("focused TextEdit should publish IME output");
        assert!(
            !ime.should_interrupt_composition,
            "maintaining focus must not interrupt an active IME composition"
        );

        harness.event(egui::Event::Ime(egui::ImeEvent::Commit("你".to_owned())));
        harness.run();

        assert_eq!(harness.state().0.query(), "你");
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
        assert_eq!(
            empty.get_by_role(egui::accesskit::Role::TextInput).value(),
            Some(String::new())
        );

        // 有查询：TextEdit 的可访问值回显查询文本。
        let mut typed = harness_for(controller_with(&["C:\\Work"], "wo"));
        typed.run();
        assert_eq!(
            typed.get_by_role(egui::accesskit::Role::TextInput).value(),
            Some("wo".to_string())
        );
    }
}
