//! 界面层的全部模块：壳（app）、样式与几何常量（theme）、字体（fonts）、
//! 状态与数据通道（state / data）、绘制件（widgets）、提交图（graph）、差异渲染（diffmodel）。

pub(crate) mod app;
pub(crate) mod data;
pub(crate) mod diffmodel;
pub(crate) mod fonts;
pub(crate) mod graph;
pub(crate) mod state;
pub(crate) mod theme;
pub(crate) mod widgets;
