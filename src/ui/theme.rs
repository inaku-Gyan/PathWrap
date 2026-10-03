//! 悬浮层的视觉主题：统一的调色板、间距、圆角，以及悬浮卡片外观。
//!
//! 主题偏好和持久化模型位于 [`crate::config`]；本模块只负责把一个已经
//! 解析好的浅色/深色模式应用到 egui，并从当前 `Visuals` 读取组件颜色。

use crate::config::{ThemeMode, ThemePreference};
use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, Frame, Margin, Shadow,
    Stroke, Visuals,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub background: Color32,
    pub text: Color32,
    pub border: Color32,
    pub hover: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub accent_text: Color32,
    pub search_fill: Color32,
}

// Zinc 调色板 + 蓝色强调。浅色模式保留同一套强调色，只调整背景、文字、
// 边框和交互态，以便在两种模式间切换时仍有一致的层级关系。
const DARK_PALETTE: ThemePalette = ThemePalette {
    background: Color32::from_rgb(24, 24, 27),
    text: Color32::from_rgb(228, 228, 231),
    border: Color32::from_rgb(63, 63, 70),
    hover: Color32::from_rgb(39, 39, 42),
    accent: Color32::from_rgb(96, 165, 250),
    selection: Color32::from_rgb(37, 99, 235),
    accent_text: Color32::WHITE,
    search_fill: Color32::from_rgba_premultiplied(255, 255, 255, 10),
};

const LIGHT_PALETTE: ThemePalette = ThemePalette {
    background: Color32::from_rgb(250, 250, 249),
    text: Color32::from_rgb(24, 24, 27),
    border: Color32::from_rgb(212, 212, 216),
    hover: Color32::from_rgb(244, 244, 245),
    accent: Color32::from_rgb(37, 99, 235),
    selection: Color32::from_rgb(37, 99, 235),
    accent_text: Color32::WHITE,
    search_fill: Color32::from_rgba_premultiplied(0, 0, 0, 12),
};

/// Apply the default `auto` preference for callers that do not have a loaded
/// configuration yet.
#[allow(dead_code)]
pub fn setup_theme(ctx: &Context) {
    setup_theme_with_preference(ctx, ThemePreference::Auto);
}

/// Load the CJK font and apply a persisted preference resolved against the
/// current Windows application theme.
pub fn setup_theme_with_preference(ctx: &Context, preference: ThemePreference) {
    install_fonts(ctx);
    apply_theme(ctx, preference.resolve(system_theme()));
}

/// Apply a concrete palette mode.  This is the seam the future settings UI can
/// call after changing a preference without touching window activation logic.
pub fn apply_theme(ctx: &Context, mode: ThemeMode) {
    let palette = palette(mode);
    ctx.set_theme(match mode {
        ThemeMode::Dark => egui::Theme::Dark,
        ThemeMode::Light => egui::Theme::Light,
    });

    let mut visuals = match mode {
        ThemeMode::Dark => Visuals::dark(),
        ThemeMode::Light => Visuals::light(),
    };
    visuals.dark_mode = matches!(mode, ThemeMode::Dark);
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.background;
    visuals.window_stroke = Stroke::new(1.0, palette.border);
    visuals.override_text_color = Some(palette.text);
    visuals.faint_bg_color = palette.search_fill;
    visuals.extreme_bg_color = palette.background;
    visuals.text_edit_bg_color = Some(palette.background);
    visuals.hyperlink_color = palette.accent;
    visuals.window_shadow = Shadow {
        offset: [0, 2],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(if matches!(mode, ThemeMode::Dark) {
            120
        } else {
            90
        }),
    };

    // 选中项：蓝色强调，白色文字在两种模式下都保持可读。
    visuals.selection.bg_fill = palette.selection;
    visuals.selection.stroke = Stroke::new(1.0, palette.accent_text);

    // 悬停、按下和普通按钮态共享同一套边框/文字对比度。
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_stroke = Stroke::new(1.0, palette.border);
        widget.fg_stroke = Stroke::new(1.0, palette.text);
    }
    visuals.widgets.noninteractive.bg_fill = palette.background;
    visuals.widgets.noninteractive.weak_bg_fill = palette.search_fill;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.hovered.weak_bg_fill = palette.hover;
    visuals.widgets.hovered.bg_fill = palette.hover;
    visuals.widgets.active.weak_bg_fill = palette.selection;
    visuals.widgets.active.bg_fill = palette.selection;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.accent_text);
    visuals.widgets.open.weak_bg_fill = palette.hover;
    visuals.widgets.open.bg_fill = palette.hover;

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

/// Return the palette associated with a resolved mode.
pub const fn palette(mode: ThemeMode) -> ThemePalette {
    match mode {
        ThemeMode::Dark => DARK_PALETTE,
        ThemeMode::Light => LIGHT_PALETTE,
    }
}

/// 悬浮卡片的统一外观：填充 + 圆角 + 描边 + 阴影，视觉上与上方对话框脱开。
pub fn overlay_frame(ctx: &Context) -> Frame {
    let visuals = current_visuals(ctx);
    Frame::NONE
        .fill(visuals.window_fill)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 8))
        .stroke(visuals.window_stroke)
        .shadow(visuals.window_shadow)
}

/// 搜索行的胶囊外观。
pub fn search_frame(ctx: &Context) -> Frame {
    let visuals = current_visuals(ctx);
    Frame::NONE
        .fill(visuals.faint_bg_color)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 5))
}

pub fn accent(ctx: &Context) -> Color32 {
    current_visuals(ctx).hyperlink_color
}

fn current_visuals(ctx: &Context) -> Visuals {
    ctx.style_of(ctx.theme()).visuals.clone()
}

/// Detect the Windows application theme.  The registry value is user-scoped,
/// does not require elevation, and is the same preference used by Win32 apps.
#[cfg(windows)]
pub fn system_theme() -> ThemeMode {
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::{PCWSTR, w};

    const PERSONALIZE: PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");

    fn read_light_theme_value(value_name: PCWSTR) -> Option<bool> {
        let mut value = 0_u32;
        let mut size = 4_u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PERSONALIZE,
                value_name,
                RRF_RT_REG_DWORD,
                None,
                Some((&mut value as *mut u32).cast()),
                Some(&mut size),
            )
        };
        if status == ERROR_SUCCESS && size >= 4 {
            Some(value != 0)
        } else {
            None
        }
    }

    read_light_theme_value(w!("AppsUseLightTheme"))
        .or_else(|| read_light_theme_value(w!("SystemUsesLightTheme")))
        .map_or(ThemeMode::Dark, |is_light| {
            if is_light {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            }
        })
}

/// Non-Windows builds are used for deterministic unit tests.  Keep the same
/// safe fallback as a missing Windows registry value.
#[cfg(not(windows))]
pub fn system_theme() -> ThemeMode {
    ThemeMode::Dark
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palettes_cover_dark_and_light_visual_states() {
        let dark = palette(ThemeMode::Dark);
        let light = palette(ThemeMode::Light);
        assert_ne!(dark.background, light.background);
        assert_ne!(dark.text, light.text);
        assert_ne!(dark.border, light.border);
        assert_ne!(dark.hover, light.hover);
        assert_ne!(dark.search_fill, light.search_fill);
    }

    #[test]
    fn applying_light_theme_updates_all_overlay_visuals() {
        let ctx = Context::default();
        apply_theme(&ctx, ThemeMode::Light);
        let visuals = current_visuals(&ctx);
        let expected = palette(ThemeMode::Light);

        assert!(!visuals.dark_mode);
        assert_eq!(visuals.window_fill, expected.background);
        assert_eq!(visuals.panel_fill, expected.background);
        assert_eq!(visuals.override_text_color, Some(expected.text));
        assert_eq!(visuals.selection.bg_fill, expected.selection);
        assert_eq!(visuals.widgets.hovered.bg_fill, expected.hover);
        assert_eq!(visuals.window_stroke.color, expected.border);
        assert_eq!(visuals.faint_bg_color, expected.search_fill);
    }

    #[test]
    fn frame_helpers_follow_the_active_theme() {
        let ctx = Context::default();
        apply_theme(&ctx, ThemeMode::Light);
        assert_eq!(
            overlay_frame(&ctx).fill,
            palette(ThemeMode::Light).background
        );
        assert_eq!(
            search_frame(&ctx).fill,
            palette(ThemeMode::Light).search_fill
        );
        assert_eq!(accent(&ctx), palette(ThemeMode::Light).accent);
    }
}
