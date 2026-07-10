//! 悬浮层的视觉主题：统一的深色调色板、间距、圆角，以及悬浮卡片外观。
//!
//! 单一样式来源——面板外观通过 [`overlay_frame`] 提供给 [`crate::ui::window`]，
//! 避免主题与内联样式各写一套导致不一致。

use egui::{
    Color32, Context, FontData, FontDefinitions, FontFamily, Frame, Margin, Rounding, Stroke,
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
pub fn overlay_frame(ctx: &Context) -> Frame {
    Frame::none()
        .fill(ctx.style().visuals.panel_fill)
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .stroke(Stroke::new(1.0, BORDER))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 2.0),
            blur: 12.0,
            spread: 0.0,
            color: Color32::from_black_alpha(120),
        })
}

/// 搜索行的胶囊外观。
pub fn search_frame() -> Frame {
    Frame::none()
        .fill(Color32::from_rgba_premultiplied(255, 255, 255, 10))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(8.0, 5.0))
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
            .insert("system_cjk".to_owned(), FontData::from_owned(bytes));
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
    let mut style = (*ctx.style()).clone();

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
    let rounding = Rounding::same(6.0);
    visuals.widgets.noninteractive.rounding = rounding;
    visuals.widgets.inactive.rounding = rounding;
    visuals.widgets.hovered.rounding = rounding;
    visuals.widgets.active.rounding = rounding;
    visuals.widgets.open.rounding = rounding;

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);

    ctx.set_style(style);
}
