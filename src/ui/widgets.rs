//! （拆分自 main.rs；见 PROCESS.md「拆分 main.rs」。）

use eframe::egui::{self, Align, Align2, Color32, FontId, Label, Pos2, Rect, RichText, Sense, Stroke, TextStyle, Vec2, pos2, vec2};
use gitx::models::{ChangeStatus, FileStatus};

use crate::ui::theme::*;
use crate::ui::state::*;
use crate::ui::data::*;
use crate::ui::graph::*;

/// 一行要画的东西。**带所有权**：绘制时只画视口里那几十行，从 `Commit` 拷出四段字符串
/// 比把整份提交搬进闭包再处理生命周期便宜得多。
pub(crate) struct Cells {
    pub(crate) sha: String,
    pub(crate) subject: String,
    pub(crate) author: String,
    pub(crate) date: String,
    /// 描述列前面那几枚药丸，可以空。内容在快照时就算好了（见 `Snapshot::badges`）。
    pub(crate) badges: Vec<RefBadge>,
    /// 图表列那个**空心环**。只有虚拟行「Uncommitted changes」有它：提交是实心圆点，
    /// 工作区不是提交，所以借一个环占住同一格。`Some(lane)` 里那个数是**下一行**（第一条
    /// 提交）的车道，环下面那条竖线跟着它——参考图里那条线是连到下面那个圆点的。
    pub(crate) ring: Option<usize>,
    /// 环下面那条竖线画不画。日志被筛过的时候不画：它连的是「下面那条提交」，而那条
    /// 可能正是被筛掉的，画出来等于凭空接上一根断头线（和图列整列留空同一个理由）。
    pub(crate) stem: bool,
}

/// 筛选条那三枚弹出按钮**右侧**的小箭头（参考图每一枚都有）。字形不是随手挑的：
/// `every_rune_the_shell_paints_has_a_glyph` 会把每一枚被画的字对着字体表查一遍，
/// 换了个字体表里没有的就当场红——`▾`（U+25BE）就是这么死的，这份字体链里只有 `▼`。
pub(crate) const ARROW: &str = "▼";
pub(crate) const ALL_BRANCHES: &str = "所有分支";
pub(crate) const SHOW_REMOTE: &str = "显示远程分支";
pub(crate) const HIDE_REMOTE: &str = "隐藏远程分支";
pub(crate) const DATE_ORDER: &str = "日期顺序";
pub(crate) const TOPO_ORDER: &str = "层级顺序";

/// 筛选条上一枚弹出按钮：按钮上是**当前那一项** + ▼，点开列出其余的。
/// 返回被点中的那项文案，没点中就是 `None`。
/// 走 `MenuButton::from_button(...small())` 而不是 `Ui::menu_button`：后者自己造默认尺寸的
/// `Button`，`.small()` 那档行高就丢了（hunk 按钮同款问题，见 PROCESS 的 egui 事实）。
pub(crate) fn popup_button(ui: &mut egui::Ui, label: &str, items: &[String]) -> Option<String> {
    let mut picked: Option<String> = None;
    // 返回值里那对 `(Response, Option<InnerResponse<()>>)` 没东西要接：菜单的收拢由默认的
    // `CloseOnClick` 管，点中了哪一项记在 `picked` 上。
    egui::containers::menu::MenuButton::from_button(
        egui::Button::new(format!("{label} {ARROW}")).small(),
    )
    .ui(ui, |ui| {
        for item in items {
            if ui.button(item.as_str()).clicked() {
                picked = Some(item.clone());
            }
        }
    });
    picked
}

/// 「显示远程分支」关掉时，把远程药丸从**画**里挑出去。
/// 取列表的参数一个字不动——这一枚和「层级顺序」那枚是两件事：那枚换的是 git 的排序参数，
/// 这枚只决定看不看得到。
pub(crate) fn visible_badges(badges: &[RefBadge], show_remote: bool) -> Vec<RefBadge> {
    if show_remote {
        return badges.to_vec();
    }
    badges.iter().filter(|b| b.kind != RefKind::Remote).cloned().collect()
}

/// 药丸前面那枚小图标。**不能随手挑**：`⑂`（U+2482）在 Noto Sans SC 的 cmap 里根本没有，
/// 落出来是豆腐块。这里每一枚都是 `every_rune_the_shell_paints_has_a_glyph` 查过的。
pub(crate) fn pill_glyph(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Branch | RefKind::Remote => "⤴",
        RefKind::Tag => "◇",
        RefKind::Ahead => "",
    }
}

/// 药丸底色。分支与远程跟踪在参考图里是**同一种**浅蓝（`upstream/main` 和 `main` 一个色），
/// 差别只在名字；「超前 N 个版本」是深藏青白字。标签那一枚没有参考图依据（那张图里没标签）。
pub(crate) fn pill_fill(kind: RefKind) -> Color32 {
    match kind {
        RefKind::Branch | RefKind::Remote => PILL_BG,
        RefKind::Tag => PILL_TAG_BG,
        RefKind::Ahead => PILL_AHEAD_BG,
    }
}

/// 描述列开头那一串药丸。返回**下一枚该摆的 x**，调用方拿它当描述文字的起点。
///
/// 宽度只能现量（`layout_no_wrap`）：药丸要贴着文字收口，而字宽归字体管——
/// 这和「车道在 worker 上排」不是一类开销，一帧最多十枚。
pub(crate) fn pill_row(p: &egui::Painter, row: Rect, badges: &[RefBadge], x: f32) -> f32 {
    let font = FontId::proportional(PILL_FONT);
    let top = row.center().y - PILL_H / 2.0;
    let mut x = x;
    for (i, badge) in badges.iter().enumerate() {
        // 「超前 N 个版本」紧贴前一枚，合起来是一颗胶囊；其余之间留 2pt 缝。
        let flush = badge.kind == RefKind::Ahead;
        if i > 0 && !flush {
            x += PILL_GAP;
        }
        let text = format!("{}{}", pill_glyph(badge.kind), badge.label);
        let fg = if badge.kind == RefKind::Ahead { Color32::WHITE } else { Color32::from_rgb(0x1a, 0x1a, 0x1a) };
        let galley = p.layout_no_wrap(text, font.clone(), fg);
        let w = galley.size().x + 2.0 * PILL_PAD;
        let rect = Rect::from_min_max(pos2(x, top), pos2(x + w, top + PILL_H));
        // 被 Ahead 贴住的那一枚右边收平，Ahead 自己左边收平——不然接缝处是两道圆弧。
        let glued = matches!(badges.get(i + 1).map(|b| b.kind), Some(RefKind::Ahead));
        let corners = match (flush, glued) {
            (true, _) => egui::CornerRadius { nw: 0, ne: PILL_R, sw: 0, se: PILL_R },
            (false, true) => egui::CornerRadius { nw: PILL_R, ne: 0, sw: PILL_R, se: 0 },
            _ => egui::CornerRadius::same(PILL_R),
        };
        p.rect(rect, corners, pill_fill(badge.kind), egui::Stroke::new(1.0, PILL_EDGE), egui::StrokeKind::Inside);
        p.galley(
            pos2(rect.min.x + PILL_PAD, row.center().y - galley.size().y / 2.0),
            galley,
            fg,
        );
        x += w;
    }
    x
}

/// 日志一行的画法：整行一个可点矩形，格子里用 painter 直接落字，
/// 只有描述列走 `Label::truncate`——长提交信息要省略号收尾，不能换行撑高行。
/// `graph` 为 `None` 时那一列留空：虚拟行（工作区）没有提交可连，筛选过的日志断了行也连不上。
///
/// 交回 `Response` 而不是 `bool`：调用方还要给这一行挂右键菜单，菜单得挂在同一个
/// 命中矩形上，用 `clicked()` 折成 bool 之后那枚矩形就找不回来了。
pub(crate) fn log_row(ui: &mut egui::Ui, width: f32, cells: &Cells, active: bool, graph: Option<&GraphRow>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, ROW_H), Sense::click());
    let p = ui.painter_at(rect);
    if active {
        p.rect_filled(rect, 0.0, SELECTED);
    } else if cells.ring.is_some() {
        // 参考图里虚拟行**不选中也是灰底**（220,220,220），它表示的是「这一行不是提交」，
        // 不是「鼠标在这儿」。
        p.rect_filled(rect, 0.0, UNCOMMITTED_BG);
    }
    // 日志行**不画 hover 底色**：这一列的行本来就靠选中色区分，鼠标扫过去整条带子跟着闪，
    // 反而让用户以为「亮起来的那行才是我点到的」——而它和实际命中的行并不总是一致。
    if let Some(g) = graph {
        graph_cell(&p, rect, g);
    } else if let Some(lane) = cells.ring {
        ring_cell(&p, rect, lane, cells.stem);
    }
    // 虚拟行整行加粗（参考图：`*`、「Uncommitted changes」、「今天 14:46」三处都是粗的）。
    // 这里只能做到**颜色更深**：egui 0.37 的 `RichText::strong()` 改的是取色而不是字重，
    // 而自带的那份 Noto Sans SC 只有 Regular 一个字面——真粗体要再带一份 Bold（OFL 允许，
    // 但那是第二个 8MB）。
    let strong = active || cells.ring.is_some();
    let fg = if active { Color32::WHITE } else { INK };
    let font = ui.style().text_styles[&TextStyle::Body].clone();
    let th = ui.text_style_height(&TextStyle::Body);
    let right = rect.max.x;
    // 四列全部**靠左**，且提交号从图列右边起（参考图量的：图表列 242、提交 284.5、描述 361.5、
    // 作者 825、日期 1017——每一格的数据都从自己列的左缘开始，没有右对齐的）。
    // 之前 sha 与日期/作者走 `RIGHT_CENTER`，还把提交列画在图列**里面**（`rect.min.x`），
    // 圆点正好压在提交号上。
    p.text(band(rect, rect.min.x + COL_GRAPH + COL_INSET, COL_SHA - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.sha, font.clone(), if active { Color32::WHITE } else { INK });
    p.text(band(rect, right - COL_DATE + COL_INSET, COL_DATE - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.date, font.clone(), if active { Color32::WHITE } else { INK });
    p.text(band(rect, right - COL_DATE - COL_AUTHOR + COL_INSET, COL_AUTHOR - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.author, font, fg);
    let x0 = rect.min.x + COL_GRAPH + COL_SHA + COL_INSET;
    // 药丸占掉描述列的开头，文字从药丸后面起笔（参考图量的：药丸 357.5..476、文字起笔 481）。
    let x = if cells.badges.is_empty() { x0 } else { pill_row(&p, rect, &cells.badges, x0) + PILL_AFTER };
    let w = (right - COL_DATE - COL_AUTHOR - x - 20.0).max(60.0);
    let mut rich = RichText::new(cells.subject.as_str()).size(12.5);
    if strong {
        rich = rich.strong();
    }
    // 参考图量的：普通行 (38,38,38)、虚拟行更深一档 (33,33,33)、选中行白字。
    // `strong()` 只改取色不改字重（自带字体只有 Regular 一个字面），所以颜色在这儿自己钉住。
    rich = rich.color(if active {
        Color32::WHITE
    } else if strong {
        INK_STRONG
    } else {
        INK
    });
    put_left(ui, band(rect, x, w, th), |ui| {
        // `selectable(false)` 不是「不许选文字」这么轻：egui 默认 `selectable_labels = true`
        // （style.rs:1599），开着文字选择的 `Label` 会给自己叠一层 `Sense::click_and_drag()`
        // （label.rs:154-170），它就压在整行那个 `Sense::click()` 矩形**上面**，
        // 按下被文字选择抢走、行什么都收不到；顺带把光标改成插入态
        // （label_text_selection.rs:645）——「鼠标移到描述上就点不中」是这一条。
        // 判据：`a_click_on_a_rows_text_selects_the_row`。
        ui.add(Label::new(rich).truncate().selectable(false));
    });
    fix_pitch(ui, rect);
    resp
}

/// 虚拟行在图表列里的那枚**空心环**：白底灰边，占住实心圆点那一格，下面接一条竖线连到
/// 第一条提交的圆点（参考图：环在 x=243、下面那条蓝线一路穿过前面的圆点）。
pub(crate) fn ring_cell(p: &egui::Painter, row: Rect, lane: usize, stem: bool) {
    let c = pos2(lane_x(row.min.x, 0), row.center().y);
    if stem {
        p.line_segment([c, pos2(c.x, row.max.y)], egui::Stroke::new(LINE_W, lane_color(lane)));
    }
    p.circle(c, DOT_R + 1.0, Color32::WHITE, egui::Stroke::new(RING_W, RING_GREY));
}

/// `Ui::put` 内部走 `scope_dyn`，它按**子 ui 的 min_rect**推进父游标——而子 ui 是垂直居中的，
/// min_rect 只有文字那一条（高 ≈17.6），比整行（`ROW_H` 21）矮，游标于是**倒回**行中间。
/// `ScrollArea::show_rows` 用 `cursor.min.y / row_height` 反解可见区间，倒回的游标会让每行
/// 都比上一行矮半截、窗口和落点错位。所以画完必须把游标按回整行底部。
pub(crate) fn fix_pitch(ui: &mut egui::Ui, row: Rect) {
    ui.advance_cursor_after_rect(row);
}

/// 行内一格的可画矩形：垂直居中、左上角 x、宽 `w`、高 `th`。
pub(crate) fn band(row: Rect, x: f32, w: f32, th: f32) -> Rect {
    let top = row.center().y - th / 2.0;
    Rect::from_min_max(pos2(x, top), pos2(x + w, top + th))
}

/// 在布局流里占一条**通宽、定高**的空矩形，返回它的绝对矩形（不接任何输入）。
/// 整条带子自己画底色时用这个：`ui.horizontal` 会把行高交给控件说了算，
/// 而这里要的是「这一条就是 28pt，里面爱画什么画什么」。
pub(crate) fn rect_of(ui: &mut egui::Ui, h: f32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    rect
}

/// 在给定矩形里**靠左**放控件。别用 `Ui::put`：它内部是/// `Layout::centered_and_justified`（egui-0.37.0/src/ui.rs:1691），短文字被居中、
/// 带空格的长文字被**拉宽对齐**，一格里的行首 x 每行都不一样。
/// SourceTree 的描述列与差异正文都是齐左的。
pub(crate) fn put_left<T>(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui) -> T) -> T {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| add(ui),
    )
    .inner
}

/// 卡上一行不能点的说明文字。走 `put_left` + `Label` 而不是直接落 galley：
/// 说明可能很长（「打在：修掉 xxx」那条提交标题），要 `Label` 的 `truncate` 才不溢出卡片。
pub(crate) fn modal_text(ui: &mut egui::Ui, card: Rect, y: f32, text: &str, fg: Color32, size: f32) {
    put_left(
        ui,
        Rect::from_min_max(pos2(card.min.x + 12.0, y), pos2(card.max.x - 12.0, y + MODAL_ROW_H)),
        |ui| {
            ui.add(Label::new(RichText::new(text).size(size).color(fg)).truncate().selectable(false));
        },
    );
}

/// 卡里可选的一行（合并挑分支用的就是它）：hover 上浅灰，点中返回 true。
pub(crate) fn modal_row(ui: &mut egui::Ui, card: Rect, y: f32, label: &str) -> bool {
    let r = Rect::from_min_max(pos2(card.min.x + 10.0, y), pos2(card.max.x - 10.0, y + MODAL_ROW_H));
    let resp = ui.allocate_rect(r, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 4.0, HOVER);
    }
    modal_text(ui, card, y, label, INK, 12.0);
    resp.clicked()
}

/// 主按钮（和 Enter 是同一条路）把卡上的东西变成一个动作。
/// 名字空着返回 `None` —— 关卡留着让用户补，比关掉再让他从头打一遍好。
pub(crate) fn primary_action(m: &mut Modal) -> Option<Action> {
    let raw = m.text.as_deref().unwrap_or("").trim().to_owned();
    match &mut m.form {
        Form::NewBranch { start } => (!raw.is_empty()).then(|| Action::CreateBranch {
            name: raw.clone(),
            start: start.clone(),
            checkout: true,
        }),
        Form::NewTag { oid } => (!raw.is_empty())
            .then(|| Action::CreateTag { name: raw.clone(), target: oid.clone() }),
        Form::StashSave { include_untracked } => Some(Action::StashSave {
            message: if raw.is_empty() { "没有说明的贮藏".to_owned() } else { raw.clone() },
            include_untracked: *include_untracked,
        }),
        Form::Confirm { then } => then.take(),
        Form::Clone { dest } => {
            let dest = dest.trim().to_owned();
            (!raw.is_empty() && !dest.is_empty()).then(|| Action::Clone { url: raw.clone(), dest })
        }
        Form::Merge { .. } => None,
    }
}

/// hunk 头右端那枚小按钮：白底、1pt 灰边、圆角 4、10.5 号灰字，**最少** 55×15
/// （参考图那两枚量的就是这个尺寸；「取消暂存区块」六个字比 55 宽，按文字撑开）。
/// 返回 `(点中了没, 实际宽度)`——宽度要交回去，左边那枚才知道该从哪儿起。
///
/// 为什么自己画而不用 `egui::Button`：Button 的高度由 `spacing.button_padding` 说了算
/// （默认在文字上下各加 3pt，10.5 号的字撑出 19pt），会溢到相邻那行上去。
/// 这一格要的就是参考图那 15pt。
pub(crate) fn hunk_button(ui: &mut egui::Ui, text: &str, right: f32, cy: f32) -> (bool, f32) {
    let p = ui.painter();
    let g = p.layout_no_wrap(text.to_owned(), FontId::proportional(10.5), HUNK_BTN_FG);
    let w = (g.size().x + 12.0).max(HUNK_BTN_W);
    let r = Rect::from_min_max(pos2(right - w, cy - HUNK_BTN_H / 2.0), pos2(right, cy + HUNK_BTN_H / 2.0));
    p.rect_filled(r, 4.0, Color32::WHITE);
    p.rect_stroke(r, 4.0, Stroke::new(1.0, HUNK_BTN_STROKE), egui::StrokeKind::Middle);
    p.galley(r.center() - g.size() / 2.0, g.clone(), HUNK_BTN_FG);
    let clicked = ui.allocate_rect(r, Sense::click()).clicked();
    (clicked, w)
}

pub(crate) fn header_row(ui: &mut egui::Ui, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEADER_H), Sense::hover());
    let p = ui.painter();
    let font = &ui.style().text_styles[&TextStyle::Small];
    let right = rect.max.x;
    // 表头跟着数据走同一侧：参考图那四个词（提交 / 描述 / 作者 / 日期）离自己列的左边线
    // 都是 5pt，跟数据一模一样；只有「图表」是在列里**居中**的（159..185，列 141..198）。
    let g = band(rect, rect.min.x, COL_GRAPH, 13.0);
    p.text(g.center_top(), Align2::CENTER_TOP, "图表", font.clone(), INK);
    for (x, w, label) in [
        (rect.min.x + COL_GRAPH + COL_INSET, COL_SHA - COL_INSET, "提交"),
        (rect.min.x + COL_GRAPH + COL_SHA + COL_INSET, 200.0, "描述"),
        (right - COL_DATE - COL_AUTHOR + COL_INSET, COL_AUTHOR - COL_INSET, "作者"),
        (right - COL_DATE + COL_INSET, COL_DATE - COL_INSET, "日期"),
    ] {
        p.text(band(rect, x, w, 13.0).left_top(), Align2::LEFT_TOP, label, font.clone(), INK);
    }
    // 列边界上的四条竖线（1pt 宽、16pt 高）。参考图：198 / 275 / 739 / 929，正好是
    // 提交、描述、作者、日期四列的左缘各减 `COL_INSET`。
    let y0 = rect.center().y - RULE_H / 2.0;
    for x in [
        rect.min.x + COL_GRAPH,
        rect.min.x + COL_GRAPH + COL_SHA,
        right - COL_DATE - COL_AUTHOR,
        right - COL_DATE,
    ] {
        p.line_segment([pos2(x, y0), pos2(x, y0 + RULE_H)], Stroke::new(1.0, RULE));
    }
    // 表头上下各一条横线，把这一条从筛选条与日志里隔开（参考图 y=78、y=103）。
    p.line_segment(
        [pos2(rect.min.x, rect.min.y + 0.5), pos2(right, rect.min.y + 0.5)],
        Stroke::new(1.0, RULE),
    );
    p.line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(right, rect.max.y - 0.5)],
        Stroke::new(1.0, RULE),
    );
}

// ------------------------------------------------------------------------ 小组件
/// 图标在上、中文在下的竖按钮（参考图第 4 条）。
pub(crate) fn tool_button(ui: &mut egui::Ui, i: usize) -> bool {
    let (icon, label, hint) = TOOLS[i];
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(50.0, TOOLBAR_H - 12.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, HOVER);
    }
    let fg = ui.visuals().text_color();
    let p = ui.painter();
    p.text(pos2(rect.center().x, rect.min.y + 10.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), fg);
    p.text(pos2(rect.center().x, rect.max.y - 7.0), Align2::CENTER_CENTER, label, FontId::proportional(9.5), fg);
    resp.on_hover_text(hint).clicked()
}

/// 一枚仓库 tab。宽度贴着名字（`layout_no_wrap` 现量），高 = 整行：
/// 选中 = 白底 + 描边，未选 = 透明灰字、悬停泛白。
/// 不用 `selectable_label`：它的底色铺满整格且不带圆角，看着不像 tab。
pub(crate) fn tab_button(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    const TAB_PAD: f32 = 8.0;
    const TAB_R: f32 = 4.0;
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), INK);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * TAB_PAD, TAB_H - 4.0), Sense::click());
    let p = ui.painter();
    let fill = if active { Color32::WHITE } else if resp.hovered() { HOVER } else { Color32::TRANSPARENT };
    let edge = if active { SIDE_PILL_BG } else { Color32::TRANSPARENT };
    p.rect(rect, egui::CornerRadius::same(TAB_R as u8), fill, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside);
    let fg = if active || resp.hovered() { INK } else { MUTED };
    p.galley(rect.min + vec2(TAB_PAD, (rect.height() - galley.size().y) / 2.0), galley, fg);
    resp.clicked()
}

/// 侧栏的一行：整行可点，文字**钉死**在参考图量到的那一列（`SIDE_TEXT_X`），
/// 选中时先铺一枚灰色药丸（`SIDE_PILL_L..R`），`icon` 画在图标格中心。返回有没有被点。
///
/// 不走 `ui.label` 是因为布局系统给不了「标签左缘 = 面板 x=41」这个约束：
/// 滚动区、内边距、间距每一层都会挪它，而这一栏的四个数全是从参考图上量的绝对位置。
/// 侧栏的一行。**返回 Response 而不是 bool**：分支、标签、贮藏那几行要挂右键菜单，
/// 而 `context_menu` 只能问 Response。调用方自己问 `.clicked()`。
pub(crate) fn side_row(
    ui: &mut egui::Ui,
    h: f32,
    text: &str,
    size: f32,
    fg: Color32,
    selected: bool,
    icon: Option<fn(&egui::Painter, Pos2)>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    // 行矩形的左边就是内容框的左边；减掉内缩才换回参考图那套面板坐标。
    let base = rect.min.x - SIDE_INSET as f32;
    let p = ui.painter();
    if selected {
        p.add(egui::Shape::rect_filled(
            Rect::from_min_max(
                pos2(base + SIDE_PILL_L, rect.center().y - ROW_H / 2.0),
                pos2(base + SIDE_PILL_R, rect.center().y + ROW_H / 2.0),
            ),
            5.0,
            SIDE_PILL_BG,
        ));
    }
    if let Some(icon) = icon {
        icon(p, pos2(base + SIDE_ICON_X, rect.center().y));
    }
    let x = base + SIDE_TEXT_X;
    // 文字放在自己那一格靠左截断（分支名可以长到放不下），点击仍然由整行的 `resp` 接。
    put_left(ui, Rect::from_min_max(pos2(x, rect.min.y), pos2(base + SIDEBAR_W - 4.0, rect.max.y)), |ui| {
        // 同日志行：文字的 `Label` 别开 `selectable_labels`，不然它压在整行的点击矩形上面，
        // 按下被文字选择抢走（见 `log_row` 里那段注释）。
        ui.add(Label::new(RichText::new(text).size(size).color(fg)).truncate().selectable(false));
    });
    resp
}

/// 一帧里从界面各处收上来的「接下来要发生的事」。行点击、右键菜单、卡片按钮都往这里放，
/// 面板收尾时统一落地。
///
/// 为什么要这一层：回调们在 `Panel::show` 的闭包里，那时候 `self` 正被借走，
/// 只能往外面递一个值。以前每个面板自己攒两三个局部变量（`checkout` / `goto` / `note`），
/// 加上卡片和确认之后就变成六个，收口处还得各写一遍顺序。
#[derive(Default)]
pub(crate) struct Intent {
    pub(crate) act: Option<Action>,
    pub(crate) card: Option<Modal>,
    pub(crate) goto: Option<Sel>,
    /// 要在日志里翻出来看的那条提交（按 oid，不是按下标 —— 侧栏那些行手上没有下标）。
    pub(crate) reveal: Option<String>,
    pub(crate) note: Option<String>,
}

impl Intent {
    pub(crate) fn run(&mut self, a: Action) {
        self.act = Some(a);
    }

    pub(crate) fn card(&mut self, m: Modal) {
        self.card = Some(m);
    }
}

/// 会丢东西的那几个动作不能点一下就跑，先摊一张确认卡（SourceTree 也是这么拦的）。
/// 拦在**出口**而不是每一处菜单里：以后多加一个入口也不会漏。
pub(crate) fn needs_confirm(a: &Action) -> bool {
    matches!(a, Action::DeleteBranch(_) | Action::DeleteTag(_) | Action::StashPop(_) | Action::StashDrop(_))
}

/// 确认卡的标题与那句说明。文案的重点是「丢的是哪一份、还能不能找回来」。
pub(crate) fn confirm_of(a: &Action) -> (&'static str, String) {
    match a {
        Action::DeleteBranch(name) => (
            "删除分支",
            format!("分支 {name} 会被删掉。没合并过的分支 git 自己会拦下来，不会静默丢提交。"),
        ),
        Action::DeleteTag(name) => ("删除标签", format!("标签 {name} 从本地删掉；远端那份要另外推一次才会没。")),
        Action::StashPop(i) => ("弹出贮藏", format!("stash@{{{i}}} 回到工作区，同时从贮藏列表里删掉。")),
        Action::StashDrop(i) => ("丢弃贮藏", format!("stash@{{{i}}} 那份改动直接丢掉，找不回来。")),
        _ => ("确认", String::new()),
    }
}

/// 右键菜单里的一行。点中就把动作交出去、并把菜单关掉（`ui.close()` 往上找最近的那层
/// `ClosableTag`，所以菜单项套在 `ui.horizontal` 里照样关得掉菜单）。
pub(crate) fn menu_item(ui: &mut egui::Ui, label: &str, then: impl FnOnce() -> Action, intent: &mut Intent) {
    if ui.button(label).clicked() {
        intent.run(then());
        ui.close();
    }
}

/// 菜单里那一行直接变成一个动作，交进调用方手上那个 `Option<Action>`。
/// 和 `menu_item` 的区别只是出口：文件表那一层本来就在收 out-param，不多绕一道 `Intent`。
pub(crate) fn menu_act(ui: &mut egui::Ui, label: &str, then: impl FnOnce() -> Action, act: &mut Option<Action>) {
    if ui.button(label).clicked() {
        *act = Some(then());
        ui.close();
    }
}

/// 菜单里那一行指向一张卡（要填名字 / 要挑东西的动作）。和 `menu_item` 分开是因为
/// 交出去的东西不一样：一个是动作，一个是「先问一句」。
pub(crate) fn menu_card(ui: &mut egui::Ui, label: &str, then: impl FnOnce() -> Modal, intent: &mut Intent) {
    if ui.button(label).clicked() {
        intent.card(then());
        ui.close();
    }
}

/// 菜单里「跳到那条提交」那一行。它谁也不改，只是把日志挪过去。
pub(crate) fn menu_go(ui: &mut egui::Ui, label: &str, oid: String, intent: &mut Intent) {
    if ui.button(label).clicked() {
        intent.reveal = Some(oid);
        ui.close();
    }
}

/// 日志一行的右键菜单。`subject` 只用来给「打标签」那张卡当说明——卡上的说明必须是
/// **打开那一刻**那条提交的话，之后日志翻页不能让它换内容。
pub(crate) fn commit_menu(ui: &mut egui::Ui, oid: &str, subject: &str, intent: &mut Intent) {
    let full = oid.to_owned();
    menu_item(ui, "检出这个提交", || Action::Checkout(full.clone()), intent);
    menu_card(ui, "从这里新建分支", || Modal::new_branch_at(Some(full.clone())), intent);
    menu_card(ui, "在这条提交上打标签", || Modal::new_tag(full.clone(), subject.to_owned()), intent);
    menu_item(ui, "合并到当前分支", || Action::Merge(full.clone()), intent);
    if ui.button("复制完整提交号").clicked() {
        ui.ctx().copy_text(full.clone());
        ui.close();
    }
}

/// 侧栏的一个分类：图标 + 浅灰标签占一行 47pt，下面挂筛选过的子项。
/// `on` 为真的那一条既铺药丸又带蓝点——参考图里这两件事同时发生在当前分支上。
///
/// 两个回调都不许捕获 `self`（它们跑在 `Panel::show` 的闭包里，那时 `self` 正被借走），
/// 要发生什么一律往交回去的那个 `Intent` 里放。`on_menu` 只在**这一行的右键菜单真的被打开**
/// 那一帧才被调用（egui 在里面判）。
pub(crate) fn side_category<T>(
    ui: &mut egui::Ui,
    title: &str,
    icon: fn(&egui::Painter, Pos2),
    filter: &str,
    items: &[T],
    mut label_of: impl FnMut(&T) -> String,
    on: impl Fn(&T) -> bool,
    mut on_click: impl FnMut(&T, &mut Intent),
    mut on_menu: impl FnMut(&mut egui::Ui, &T, &mut Intent),
    intent: &mut Intent,
) {
    let q = filter.trim().to_lowercase();
    side_row(ui, SIDE_CAT_H, title, 12.5, SIDE_LABEL_C, false, Some(icon));
    let hits: Vec<(String, &T)> = items
        .iter()
        .map(|it| (label_of(it), it))
        .filter(|(label, _)| label.to_lowercase().contains(&q))
        .collect();
    for (label, it) in hits {
        let on = on(it);
        let resp =
            side_row(ui, ROW_H, &label, 12.0, SIDE_INK, on, if on { Some(icon_dot) } else { None });
        resp.context_menu(|ui| on_menu(ui, it, intent));
        if resp.clicked() {
            on_click(it, intent);
        }
    }
}

// ---- 侧栏图标：全部是 18..38 那一格里的线描，颜色 `SIDE_ICON_C`，笔画 1.5pt ----

/// 「工作区」= 一台显示器。参考图 69..86 那格量的：屏幕 20×11，下面一截支架加一道底座。
pub(crate) fn icon_monitor(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    p.rect_stroke(Rect::from_min_max(pos2(c.x - 10.0, c.y - 8.0), pos2(c.x + 10.0, c.y + 3.0)), 2.0, s, egui::StrokeKind::Middle);
    p.line_segment([pos2(c.x, c.y + 3.0), pos2(c.x, c.y + 6.5)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 6.5), pos2(c.x + 5.0, c.y + 6.5)], s);
}

/// 「分支」= 标准 git-branch 形：一根主干、一道斜出去的分叉、三个端点各一枚小圈。
pub(crate) fn icon_branch(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    p.line_segment([pos2(c.x - 5.0, c.y - 5.5), pos2(c.x - 5.0, c.y + 5.5)], s);
    p.line_segment([pos2(c.x - 5.0, c.y + 1.0), pos2(c.x + 5.0, c.y - 5.0)], s);
    for at in [pos2(c.x - 5.0, c.y - 7.5), pos2(c.x - 5.0, c.y + 7.5), pos2(c.x + 5.0, c.y - 7.5)] {
        p.circle(at, 2.5, SIDE_ICON_C, s);
    }
}

/// 「标签」= 一枚斜放的价格牌：菱形 + 一个穿孔。
pub(crate) fn icon_tag(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    let (l, r, t, b) = (c.x - 9.0, c.x + 9.0, c.y - 8.0, c.y + 8.0);
    p.line_segment([pos2(l, c.y), pos2(c.x, t)], s);
    p.line_segment([pos2(c.x, t), pos2(r, c.y)], s);
    p.line_segment([pos2(r, c.y), pos2(c.x, b)], s);
    p.line_segment([pos2(c.x, b), pos2(l, c.y)], s);
    p.circle(pos2(c.x - 3.5, c.y - 3.5), 1.5, SIDE_ICON_C, Stroke::NONE);
}

/// 「远端」= 一朵云。三枚圆叠在一条底线上，圆填侧栏底色好把内部的弧线盖掉。
pub(crate) fn icon_cloud(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    for (dx, dy, r) in [(-4.5, 1.0, 4.0), (4.5, 1.5, 3.5), (0.0, -2.5, 5.0)] {
        p.circle(pos2(c.x + dx, c.y + dy), r, SIDEBAR_BG, s);
    }
    p.line_segment([pos2(c.x - 8.5, c.y + 5.0), pos2(c.x + 8.5, c.y + 5.0)], s);
}

/// 「已贮藏」= 一个抽屉里搁着三样东西（参考图那枚就是方框加三个小圆）。
pub(crate) fn icon_stash(p: &egui::Painter, c: Pos2) {
    let s = Stroke::new(1.5, SIDE_ICON_C);
    p.rect_stroke(Rect::from_min_max(pos2(c.x - 9.0, c.y - 6.5), pos2(c.x + 9.0, c.y + 6.5)), 2.0, s, egui::StrokeKind::Middle);
    for at in [pos2(c.x - 4.0, c.y - 1.5), pos2(c.x + 4.0, c.y - 1.5), pos2(c.x, c.y + 3.0)] {
        p.circle(at, 2.0, SIDE_ICON_C, Stroke::NONE);
    }
}

/// 当前分支前面那颗点：参考图圆心 x≈29.5、直径 7、蓝 `(36,93,184)`。
pub(crate) fn icon_dot(p: &egui::Painter, c: Pos2) {
    p.circle(pos2(c.x - SIDE_ICON_X + SIDE_DOT_X, c.y), SIDE_DOT_R, SIDE_DOT_C, Stroke::NONE);
}

/// 一组文件 + 组标题复选框（参考图第 5 条：勾 = 这一组都在暂存区，取消勾就是整组 unstage）。
pub(crate) fn section(
    ui: &mut egui::Ui,
    title: &str,
    files: &[&FileStatus],
    staged: bool,
    current: &Option<String>,
    act: &mut Option<Action>,
    pick: &mut Option<Target>,
) {
    if files.is_empty() {
        return;
    }
    let on = staged;
    // 分类行：方框钉在面板左缘 +8、标签再往后 7pt（参考图 x 149 / 169.5，面板左缘 141）。
    // 不用 `ui.checkbox`：egui 那枚跟着字号长（11.5 号撑出 18pt 的框），缩进也由前一格说了算，
    // 三列对不齐；而且它的 `changed()` 会把「整组勾掉」和「用户点了别的」混成一个信号。
    let (hrect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    let head_hit = check_box(ui, hrect.min.x + CHK_HEAD_X, hrect.center().y, on);
    put_left(
        ui,
        Rect::from_min_max(pos2(hrect.min.x + CHK_HEAD_X + CHK_S + 7.0, hrect.min.y), pos2(hrect.max.x - 84.0, hrect.max.y)),
        |ui| {
            ui.add(Label::new(RichText::new(format!("{title}（{}）", files.len())).size(11.5).strong()).selectable(false).sense(Sense::hover()));
        },
    );
    let mut discard = false;
    if !staged {
        // 「放弃整组」是我们自己加的：参考图这一格是「⋯」菜单（还没画），先钉在行尾。
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(pos2(hrect.max.x - 80.0, hrect.min.y), pos2(hrect.max.x, hrect.max.y)))
                .layout(egui::Layout::right_to_left(Align::Center)),
            |ui| {
                discard = ui.small_button("放弃整组").clicked();
            },
        );
    }
    fix_pitch(ui, hrect);
    if discard {
        *act = Some(Action::Discard(files.iter().map(|f| f.path.clone()).collect()));
    } else if head_hit {
        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        *act = Some(if staged { Action::Unstage(paths) } else { Action::Stage(paths) });
    }
    for f in files {
        let (path, is_staged, untracked) = (f.path.clone(), f.staged, f.worktree_status == ChangeStatus::Untracked);
        let st = status_of(*f).clone();
        let (resp, toggled) = file_row(ui, &f.path, &st, current.as_deref() == Some(f.path.as_str()), "", Some(is_staged));
        if toggled {
            let one = vec![path.clone()];
            *act = Some(if is_staged { Action::Unstage(one) } else { Action::Stage(one) });
        }
        if resp.clicked() {
            *pick = Some(Target::Working { staged: is_staged, path: Some(path.clone()), untracked });
        }
        resp.context_menu(|ui| file_menu(ui, &path, is_staged, untracked, act));
    }
}
