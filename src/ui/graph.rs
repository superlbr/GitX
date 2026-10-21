//! （拆分自 main.rs；见 PROCESS.md「拆分 main.rs」。）

use eframe::egui::{self, Color32, Rect, pos2, vec2};
use gitx::models::Commit;

use crate::ui::theme::*;

// ------------------------------------------------------------------ 提交图序列
/// 一行的图元。为什么不是直接画而是先列出来：无头判据要能问「这一行有没有一条弯到第二条
/// 车道的合并线」，而落图的形状里竖线、弯线、圆点是三种不同的 shape，问不出语义。
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Seg {
    /// 上面有提交一路排到了这一行：本车道从行顶连到圆点。
    Stem(usize),
    /// 别的车道下面还排着提交：这条道从行顶贯穿到底。
    Through(usize),
    /// 本行提交往下接它的第一个父提交。
    Tail(usize),
    /// 合并的另一边：从本行圆点弯到 `to` 车道在这一行的行底。
    Arm { from: usize, to: usize },
}

#[derive(Clone, Debug)]
pub(crate) struct GraphRow {
    pub(crate) lane: usize,
    pub(crate) segs: Vec<Seg>,
}

/// 把提交列表（新的在上）排成「一行一条道」。`git log` 保证父提交排在子提交下面
/// （gitx-core 的 `every_parent_is_listed_below_its_child` 钉着这条），所以从头扫一遍
/// 就能给每行定车道：
/// * 这一行的提交如果已经在某条道上排队，就沿用那条道——分支接着往下走；
/// * 否则找空道，空道不够才开新道；
/// * 第一个父提交接在本行道底下，多出来的父提交各占一条道并留下一条弯线；
/// * 尾巴上的空道一律裁掉。旧 React 版的 `layoutGraph` **从不释放车道**，
///   每合并一次就多占一条，几百次合并后列宽早爆了；裁尾巴才能把 `COL_GRAPH` 那 49pt 守住。
/// * 同一个提交被排在几条道上（日期序会把分支交错）→ 只认第一条，其余腾出来。
pub(crate) fn layout_graph(commits: &[Commit]) -> Vec<GraphRow> {
    bump_graph_layouts();
    let free = |lanes: &mut Vec<Option<String>>| match lanes.iter().position(|l| l.is_none()) {
        Some(k) => k,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    };
    let mut rows = Vec::with_capacity(commits.len());
    let mut lanes: Vec<Option<String>> = Vec::new();
    for c in commits {
        let queued = lanes.iter().position(|l| l.as_deref() == Some(c.id.as_str()));
        let lane = match queued {
            Some(k) => k,
            None => {
                let k = free(&mut lanes);
                lanes[k] = Some(c.id.clone());
                k
            }
        };
        for k in 0..lanes.len() {
            if k != lane && lanes[k].as_deref() == Some(c.id.as_str()) {
                lanes[k] = None;
            }
        }
        let mut segs = Vec::new();
        if queued.is_some() {
            segs.push(Seg::Stem(lane));
        }
        for k in 0..lanes.len() {
            if k != lane && lanes[k].is_some() {
                segs.push(Seg::Through(k));
            }
        }
        match c.parents.split_first() {
            Some((first, rest)) => {
                segs.push(Seg::Tail(lane));
                lanes[lane] = Some(first.clone());
                for p in rest {
                    let to = match lanes.iter().position(|l| l.as_deref() == Some(p.as_str())) {
                        Some(k) => k,
                        None => free(&mut lanes),
                    };
                    lanes[to] = Some(p.clone());
                    segs.push(Seg::Arm { from: lane, to });
                }
            }
            // 根提交：这条道到此为止，往下不再有东西排队。
            None => lanes[lane] = None,
        }
        while lanes.last().map(|l| l.is_none()).unwrap_or(false) {
            lanes.pop();
        }
        rows.push(GraphRow { lane, segs });
    }
    rows
}

/// 车道中心落在图序列列里的 x。
pub(crate) fn lane_x(min_x: f32, lane: usize) -> f32 {
    min_x + GRAPH_PAD + lane as f32 * LANE_W + LANE_W / 2.0
}

/// 画这一行的图元：先线后点，点盖在线头上看着才像「一个提交」。
pub(crate) fn graph_cell(p: &egui::Painter, row: Rect, g: &GraphRow) {
    let (top, mid, bottom) = (row.min.y, row.center().y, row.max.y);
    let stroke = |lane: usize| egui::Stroke::new(LINE_W, lane_color(lane));
    let x = |lane: usize| lane_x(row.min.x, lane);
    for s in &g.segs {
        match s {
            Seg::Stem(k) => {
                p.line_segment([pos2(x(*k), top), pos2(x(*k), mid)], stroke(*k));
            }
            Seg::Through(k) => {
                p.line_segment([pos2(x(*k), top), pos2(x(*k), bottom)], stroke(*k));
            }
            Seg::Tail(k) => {
                p.line_segment([pos2(x(*k), mid), pos2(x(*k), bottom)], stroke(*k));
            }
            Seg::Arm { from, to } => {
                let (a, b) = (pos2(x(*from), mid), pos2(x(*to), bottom));
                let d = (b.y - a.y) * 0.55;
                p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    [a, a + vec2(0.0, d), b - vec2(0.0, d), b],
                    false,
                    Color32::TRANSPARENT,
                    stroke(*to),
                ));
            }
        }
    }
    p.circle_filled(pos2(x(g.lane), mid), DOT_R, lane_color(g.lane));
}
