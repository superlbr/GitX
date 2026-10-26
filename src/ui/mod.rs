//! 界面层的全部模块：壳（app）、样式与几何常量（theme）、字体（fonts）、
//! 状态与数据通道（state / data）、绘制件（widgets）、提交图（graph）、差异渲染（diffmodel）。

/// 版本号只有一处来源：`Cargo.toml` 的 `version`。窗口标题、状态栏右下角、
/// 以及 release 工作流的标签核对都读它，别在别处再写一个字面量。
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) mod app;
pub(crate) mod data;
pub(crate) mod diffmodel;
pub(crate) mod fonts;
pub(crate) mod graph;
pub(crate) mod icon;
pub(crate) mod state;
pub(crate) mod theme;
pub(crate) mod toast;
pub(crate) mod widgets;
