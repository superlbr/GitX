//! GitX 的 egui 界面层：原生桌面外壳，接的是**真数据**。
//!
//! 界面模块都在 `src/ui/`；核心层是同一个包的 lib target（`gitx::`，见 `src/lib.rs`），
//! git 函数直接调，中间没有 IPC。
//! 布局几何来自实测参考图（测量记录见 docs/PROCESS.md），常量名就是图上那四块边界。
//! 页只有一页：上面日志，下面「文件列表 | 差异」；选日志第一行那个虚拟行就是工作区。

mod ui;

#[cfg(test)]
#[path = "geometry.rs"]
mod geometry_tests;

use crate::ui::data::style;
use crate::ui::state::Gitx;
use crate::ui::theme::*;
use eframe::egui;

/// 真窗口唯一的配置。拆成函数只为一件事：窗口约束在无头画布上量不到，判据只能读这个对象
/// （见 `the_window_itself_is_floored_at_the_measured_size`）。
fn viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_inner_size([WIN_W, WIN_H])
        .with_min_inner_size([MIN_W, MIN_H])
        // 标题栏不画（见下面三条开关），所以 Dock / Mission Control / `cmd+tab`
        // 里认这个应用只能靠这里。版本一并写上：系统那一列没有别的可读。
        .with_title(format!("GitX {}", crate::ui::VERSION))
        // 窗口图标。macOS 的 Dock 图标来自 `.app` 里的 `.icns`（`scripts/bundle-macos.sh`），
        // 这一条是给 Linux/Windows 和窗口管理器用的；两边读的是同一张 `assets/icon.png`。
        .with_icon(crate::ui::icon::app_icon())
        // 参考图里那一行 52pt 是**标题栏本身**：红绿灯 + 仓库名 + 11 枚按钮同在一行。
        // 我们原来在它下面又画了一条工具栏，于是窗口顶部白占一条 28pt 的系统标题栏，
        // 而且仓库名排在 x=9 —— 正被红绿灯盖着。这三条开关把两条带合成一条。
        // `title_shown(false)`：仓库名由我们自己画（macOS 的标题会居中，参考图那个是左对齐的）。
        .with_titlebar_shown(false)
        .with_fullsize_content_view(true)
        .with_title_shown(false)
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: viewport(),
        ..Default::default()
    };
    eframe::run_native(
        "GitX",
        opts,
        Box::new(|cc| {
            style(&cc.egui_ctx);
            Ok(Box::new(Gitx::new()))
        }),
    )
}
