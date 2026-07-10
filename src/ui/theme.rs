//! 悬浮层的视觉主题：统一的深色调色板、间距、圆角，以及悬浮卡片外观。
//!
//! 单一样式来源——面板外观通过 [`overlay_frame`] 提供给 [`crate::ui::window`]，
//! 避免主题与内联样式各写一套导致不一致。

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, Frame, Margin, Stroke,
    Visuals,
};

// 调色板（zinc 深色系 + 蓝色强调）。
const BG: Color32 = Color32::from_rgb(24, 24, 27); // zinc-900
const TEXT: Color32 = Color32::from_rgb(228, 228, 231); // zinc-200
const BORDER: Color32 = Color32::from_rgb(63, 63, 70); // zinc-700
const HOVER: Color32 = Color32::from_rgb(39, 39, 42); // zinc-800
const ACCENT: Color32 = Color32::from_rgb(37, 99, 235); // blue-600
const ACCENT_LIGHT: Color32 = Color32::from_rgb(96, 165, 250); // blue-400

pub fn setup_theme(ctx: &Context) {
    install_fonts(ctx);
    apply_style(ctx);
}

/// 悬浮卡片的统一外观：填充 + 圆角 + 描边 + 阴影，视觉上与上方对话框脱开。
pub fn overlay_frame() -> Frame {
    Frame::NONE
        .fill(BG)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 8))
        .stroke(Stroke::new(1.0, BORDER))
        .shadow(egui::epaint::Shadow {
            offset: [0, 2],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(120),
        })
}

/// 搜索行的胶囊外观。
pub fn search_frame() -> Frame {
    Frame::NONE
        .fill(Color32::from_rgba_premultiplied(255, 255, 255, 10))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 5))
}

pub fn accent() -> Color32 {
    ACCENT_LIGHT
}

/// 加载覆盖中英文的系统字体（微软雅黑），避免中文路径显示为方块。
fn install_fonts(ctx: &Context) {
    const CANDIDATES: [&str; 3] = [
        r"C:\Windows\Fonts\msyh.ttc",   // 微软雅黑
        r"C:\Windows\Fonts\msyhl.ttc",  // 微软雅黑 Light
        r"C:\Windows\Fonts\simsun.ttc", // 宋体（兜底）
    ];

    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = FontDefinitions::default();
        fonts
            .font_data
            .insert("system_cjk".to_owned(), FontData::from_owned(bytes).into());
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "system_cjk".to_owned());
        }
        ctx.set_fonts(fonts);
        log::debug!("loaded system CJK font: {path}");
        return;
    }

    log::warn!("no CJK-capable system font found; non-ASCII paths may render as boxes");
}

fn apply_style(ctx: &Context) {
    // 悬浮条只有深色一套外观，不跟随系统主题切换。
    ctx.set_theme(egui::Theme::Dark);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.override_text_color = Some(TEXT);

    // 选中项：蓝色强调。
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT_LIGHT);

    // 悬停项背景。
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.hovered.bg_fill = HOVER;
    visuals.widgets.active.weak_bg_fill = ACCENT;

    // 统一圆角。
    let corner_radius = CornerRadius::same(6);
    visuals.widgets.noninteractive.corner_radius = corner_radius;
    visuals.widgets.inactive.corner_radius = corner_radius;
    visuals.widgets.hovered.corner_radius = corner_radius;
    visuals.widgets.active.corner_radius = corner_radius;
    visuals.widgets.open.corner_radius = corner_radius;

    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
    });
}
