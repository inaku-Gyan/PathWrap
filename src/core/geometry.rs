//! 停靠位置与工作区裁剪。所有输入、输出均为物理像素，支持负坐标显示器。

use super::types::DialogInfo;

const OVERLAY_HEIGHT_LOGICAL: i64 = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// 优先贴在对话框下方，其次上方；两侧都不足时使用空间较大的一侧并裁剪。
/// 对话框占满工作区时允许重叠，以保持悬浮层可见。无有效工作区时不下发位置。
pub fn dock_geometry(dialog: &DialogInfo) -> Option<Placement> {
    let area = dialog.work_area;
    let left = i64::from(area.left);
    let top = i64::from(area.top);
    let right = i64::from(area.right);
    let bottom = i64::from(area.bottom);
    if left >= right || top >= bottom || dialog.width <= 0 || dialog.height <= 0 {
        return None;
    }

    // 中间运算使用 i64，避免负坐标、超宽窗口或异常 DPI 使 i32 加减溢出。
    let dpi = i64::from(if dialog.dpi == 0 { 96 } else { dialog.dpi });
    let desired_height = (OVERLAY_HEIGHT_LOGICAL * dpi / 96).clamp(1, i64::from(i32::MAX));
    let width = i64::from(dialog.width).min(right - left);
    let x = i64::from(dialog.x).clamp(left, right - width);
    let dialog_top = i64::from(dialog.y);
    let dialog_bottom = dialog_top + i64::from(dialog.height);

    let below = dialog_bottom.clamp(top, bottom);
    let above = dialog_top.clamp(top, bottom);
    let space_below = bottom - below;
    let space_above = above - top;
    let (y, height) = if space_below >= desired_height {
        (below, desired_height)
    } else if space_above >= desired_height {
        (above - desired_height, desired_height)
    } else if space_below > 0 && space_below >= space_above {
        (below, space_below)
    } else if space_above > 0 {
        (top, space_above)
    } else {
        let height = desired_height.min(bottom - top);
        (dialog_bottom.clamp(top, bottom - height), height)
    };

    Some(Placement {
        x: i32::try_from(x).ok()?,
        y: i32::try_from(y).ok()?,
        width: i32::try_from(width).ok()?,
        height: i32::try_from(height).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::WorkArea;

    fn dialog() -> DialogInfo {
        DialogInfo {
            hwnd: 1,
            pid: 2,
            process_created: 3,
            x: 100,
            y: 200,
            width: 600,
            height: 400,
            dpi: 96,
            work_area: WorkArea {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            },
        }
    }

    #[test]
    fn normal_docking_preserves_visual_alignment() {
        assert_eq!(
            dock_geometry(&dialog()),
            Some(Placement {
                x: 100,
                y: 600,
                width: 600,
                height: 140
            })
        );
    }

    #[test]
    fn bottom_taskbar_forces_full_height_above() {
        let mut target = dialog();
        target.y = 600;
        assert_eq!(
            dock_geometry(&target),
            Some(Placement {
                x: 100,
                y: 460,
                width: 600,
                height: 140
            })
        );
    }

    #[test]
    fn insufficient_space_uses_the_larger_side_and_clips_height() {
        let mut target = dialog();
        target.work_area.bottom = 600;
        target.y = 100;
        target.height = 450;
        assert_eq!(
            dock_geometry(&target),
            Some(Placement {
                x: 100,
                y: 0,
                width: 600,
                height: 100
            })
        );
        target.y = 50;
        assert_eq!(
            dock_geometry(&target),
            Some(Placement {
                x: 100,
                y: 500,
                width: 600,
                height: 100
            })
        );
        target.height = 500;
        assert_eq!(
            dock_geometry(&target),
            Some(Placement {
                x: 100,
                y: 550,
                width: 600,
                height: 50
            })
        );
    }

    #[test]
    fn horizontal_edges_and_side_taskbar_clip_inside_the_work_area() {
        let mut target = dialog();
        target.work_area.left = 48;
        target.x = -100;
        assert_eq!(
            dock_geometry(&target).map(|r| (r.x, r.width)),
            Some((48, 600))
        );
        target.x = 1700;
        assert_eq!(
            dock_geometry(&target).map(|r| (r.x, r.width)),
            Some((1320, 600))
        );
        target.width = 2400;
        assert_eq!(
            dock_geometry(&target).map(|r| (r.x, r.width)),
            Some((48, 1872))
        );
    }

    #[test]
    fn negative_origin_monitor_uses_target_dpi() {
        let mut target = dialog();
        target.work_area = WorkArea {
            left: -1920,
            top: -300,
            right: 0,
            bottom: 740,
        };
        target.x = -1800;
        target.y = 200;
        target.dpi = 144;
        assert_eq!(
            dock_geometry(&target),
            Some(Placement {
                x: -1800,
                y: -10,
                width: 600,
                height: 210
            })
        );
    }

    #[test]
    fn dpi_is_applied_before_selecting_a_side() {
        let mut target = dialog();
        target.y = 350;
        assert_eq!(
            dock_geometry(&target).map(|r| (r.y, r.height)),
            Some((750, 140))
        );
        target.dpi = 192;
        target.work_area.bottom = 1000;
        assert_eq!(
            dock_geometry(&target).map(|r| (r.y, r.height)),
            Some((70, 280))
        );
        target.dpi = 0;
        assert_eq!(dock_geometry(&target).map(|r| r.height), Some(140));
    }

    #[test]
    fn full_work_area_dialog_keeps_overlay_visible_inside() {
        let mut target = dialog();
        target.y = 0;
        target.height = 1040;
        assert_eq!(
            dock_geometry(&target),
            Some(Placement {
                x: 100,
                y: 900,
                width: 600,
                height: 140
            })
        );
        target.work_area = WorkArea {
            left: 40,
            top: 80,
            right: 180,
            bottom: 170,
        };
        target.y = -100;
        assert_eq!(
            dock_geometry(&target),
            Some(Placement {
                x: 40,
                y: 80,
                width: 140,
                height: 90
            })
        );
    }

    #[test]
    fn invalid_dimensions_produce_no_dock_request() {
        let mut target = dialog();
        target.work_area.right = target.work_area.left;
        assert_eq!(dock_geometry(&target), None);
        target = dialog();
        target.work_area.bottom = -1;
        assert_eq!(dock_geometry(&target), None);
        target = dialog();
        target.width = 0;
        assert_eq!(dock_geometry(&target), None);
        target.width = 600;
        target.height = -1;
        assert_eq!(dock_geometry(&target), None);
    }

    #[test]
    fn extreme_coordinates_and_dpi_never_escape_the_work_area() {
        for area in [
            WorkArea {
                left: -1920,
                top: -300,
                right: 0,
                bottom: 740,
            },
            WorkArea {
                left: 48,
                top: 40,
                right: 1920,
                bottom: 1040,
            },
            WorkArea {
                left: i32::MIN,
                top: i32::MIN,
                right: i32::MAX,
                bottom: i32::MAX,
            },
            WorkArea {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            },
        ] {
            for x in [i32::MIN, -2000, 0, 2000, i32::MAX] {
                for y in [i32::MIN, -2000, 0, 2000, i32::MAX] {
                    for dpi in [0, 96, 120, 144, 192, u32::MAX] {
                        let mut target = dialog();
                        target.work_area = area;
                        target.x = x;
                        target.y = y;
                        target.width = i32::MAX;
                        target.height = i32::MAX;
                        target.dpi = dpi;
                        let Some(rect) = dock_geometry(&target) else {
                            panic!("valid rectangle must fit")
                        };
                        assert!(rect.width > 0 && rect.height > 0);
                        assert!(rect.x >= area.left && rect.y >= area.top);
                        assert!(i64::from(rect.x) + i64::from(rect.width) <= i64::from(area.right));
                        assert!(
                            i64::from(rect.y) + i64::from(rect.height) <= i64::from(area.bottom)
                        );
                    }
                }
            }
        }
    }
}
