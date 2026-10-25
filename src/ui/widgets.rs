//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use eframe::egui::{self, Align, Align2, Color32, FontId, Label, Pos2, Rect, RichText, Sense, Stroke, TextStyle, Vec2, pos2, vec2};
use gitx::git;
use gitx::models::{BlameLine, ChangeStatus, FileLogEntry, FileStatus, Remote};

use crate::ui::theme::*;
use crate::ui::state::*;
use crate::ui::data::*;
use crate::ui::graph::*;
use crate::ui::diffmodel::{DiffLine, Kind};

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
// 下面这几条文案**逐字抄自**本机 SourceTree 4.2.19
// `Contents/Resources/zh_CN.lproj/Localizable.strings`（UTF-16LE，要 `iconv -f UTF-16 -t UTF-8`）。
// 键名注在后面，下次核对直接按键搜；自己改一个"更顺口的"就是破坏复刻契约第③条。
pub(crate) const ALL_BRANCHES: &str = "所有分支"; // LogBranchesAll
pub(crate) const CURRENT_BRANCH: &str = "当前分支"; // LogBranchesCurrent
pub(crate) const SHOW_REMOTE: &str = "显示远程分支"; // LogShowRemotes
pub(crate) const HIDE_REMOTE: &str = "隐藏远程分支"; // LogHideRemotes
pub(crate) const DATE_ORDER: &str = "按日期排序"; // LogDateOrder
pub(crate) const TOPO_ORDER: &str = "层级顺序"; // LogAncestorOrder
pub(crate) const PENDING_SORTED_BY_PATH: &str = "待定的文件, 已依照路径排序"; // PendingFiles + SortedByPath
/// 日志右侧那格 SHA 跳转框的提示。官方原文**没有冒号**，别自己补。
pub(crate) const JUMP_TO_SHA: &str = "转到Commit的SHA-1"; // JumpToRevisionLabelGit

/// 范围那一档在按钮上写什么：两态（所有分支/当前分支）用官方文案，第三条「具体分支」
/// 是我们自己加的（SourceTree 那一档只切两态），所以按钮上直接是分支名。
pub(crate) fn scope_label(scope: &git::Scope) -> String {
    match scope {
        git::Scope::All => ALL_BRANCHES.to_owned(),
        git::Scope::Current => CURRENT_BRANCH.to_owned(),
        git::Scope::Rev(r) => r.clone(),
    }
}

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
        // 虚拟行**不选中也是灰底**（§3.1 把它和 hunk 头归到同一枚 `BG_INSET`），
        // 它表示的是「这一行不是提交」，不是「鼠标在这儿」。
        p.rect_filled(rect, 0.0, BG_INSET);
    }
    // 日志行**不画 hover 底色**（沿用旧决定）：这一列的行本来就靠选中色区分，鼠标扫过去整条带子跟着闪，
    // 反而让用户以为「亮起来的那行才是我点到」。§2 把 hover 记在 LOG 那一行里，
    // 那一档跟着 LOG 带落地（顺带把 `BG_HOVER` 画上）。
    if let Some(g) = graph {
        graph_cell(&p, rect, g);
    } else if let Some(lane) = cells.ring {
        ring_cell(&p, rect, lane, cells.stem);
    }
    // 虚拟行整行加粗（`*`、「Uncommitted changes」、「今天 14:46」三处都是粗的）。
    // 真字重来自 Inter SemiBold（fonts.rs 嵌入的那份）：subject 走命名字族。中文那半
    // 没有真粗体（Noto 只带 Regular，第二份 8MB 不值），§3.4 明写了这条限制——
    // 所以这里**不再用更深一档的字色补层次**（DESIGN.md §6：`INK_STRONG` → 加粗）。
    let strong = active || cells.ring.is_some();
    // §3.4 的层级：提交信息（正文）走 `TEXT_PRIMARY`，提交号 / 作者 / 日期走「次要」那一档。
    // 旧的写法是四列同色、「不靠颜色分主次」——那是 SourceTree 复刻期的规则，
    // DESIGN.md 原则②「信息靠层级」把它换掉了。选中行整行白字，不分档。
    let meta = if active { Color32::WHITE } else { TEXT_SECONDARY };
    let font = ui.style().text_styles[&TextStyle::Body].clone();
    let th = ui.text_style_height(&TextStyle::Body);
    let right = rect.max.x;
    // 四列全部**靠左**，且提交号从图列右边起（参考图量的：图表列 242、提交 284.5、描述 361.5、
    // 作者 825、日期 1017——每一格的数据都从自己列的左缘开始，没有右对齐的）。
    // 之前 sha 与日期/作者走 `RIGHT_CENTER`，还把提交列画在图列**里面**（`rect.min.x`），
    // 圆点正好压在提交号上。
    p.text(band(rect, rect.min.x + COL_GRAPH + COL_INSET, COL_SHA - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.sha, font.clone(), meta);
    p.text(band(rect, right - COL_DATE + COL_INSET, COL_DATE - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.date, font.clone(), meta);
    p.text(band(rect, right - COL_DATE - COL_AUTHOR + COL_INSET, COL_AUTHOR - COL_INSET, th).left_center(), Align2::LEFT_CENTER, &cells.author, font, meta);
    let x0 = rect.min.x + COL_GRAPH + COL_SHA + COL_INSET;
    // 药丸占掉描述列的开头，文字从药丸后面起笔（参考图量的：药丸 357.5..476、文字起笔 481）。
    let x = if cells.badges.is_empty() { x0 } else { pill_row(&p, rect, &cells.badges, x0) + PILL_AFTER };
    let w = (right - COL_DATE - COL_AUTHOR - x - 20.0).max(60.0);
    let mut rich = RichText::new(cells.subject.as_str()).size(12.5);
    if strong {
        rich = rich.family(crate::ui::fonts::semibold_family());
    }
    // §6 迁移表那条「`INK_STRONG` → 加粗」：虚拟行不再取更深的一档字色，
    // 它和正文同色，靠 `semibold_family()` 那面真字重拉开（中文那半没有粗体，
    // 落回 Noto Regular——§3.4 明写了这条限制）。
    rich = rich.color(if active { Color32::WHITE } else { TEXT_PRIMARY });
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
    let r = Rect::from_min_max(
        pos2(card.min.x + 8.0, y),
        pos2(card.max.x - 8.0, y + MODAL_ROW_H),
    );
    let resp = ui.allocate_rect(r, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, BG_HOVER);
    }
    modal_text(ui, card, y, label, TEXT_PRIMARY, 12.0);
    resp.clicked()
}

/// 卡上一行开关（偏好卡与提交卡用的都是它）：整行定高 `MODAL_ROW_H`，文字就是开关的名字。
/// 走 `ui.checkbox` 而不是自己画方框 + 命中矩形：勾选状态、键盘焦点、命中范围由一个
/// 控件说了算，判据只需盯 `changed()` 这一处。
pub(crate) fn pref_switch(ui: &mut egui::Ui, card: Rect, y: f32, label: &str, on: &mut bool) {
    let r = Rect::from_min_max(
        pos2(card.min.x + MODAL_PAD, y),
        pos2(card.max.x - MODAL_PAD, y + MODAL_ROW_H),
    );
    check_row(ui, r, |ui| ui.checkbox(on, RichText::new(label).size(11.5)));
}

/// 勾选方框那一档尺寸。全局 `interact_size.y` 是 `ROW_H`（26），egui 的 checkbox 拿它当
/// 方框见方，于是卡上每一枚勾都是一大块灰色圆角——2026-10-24 目视那一轮抓到的就是它。
/// 每一枚勾在**自己那一格**里把它压回 `CHK_CARD`，日志/文件表那些行不受影响。
fn check_row<T>(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui) -> T) -> T {
    put_left(ui, rect, |ui| {
        let sp = ui.spacing_mut();
        sp.interact_size.y = CHK_CARD;
        sp.icon_width = CHK_CARD;
        sp.icon_width_inner = CHK_CARD - 6.0;
        sp.icon_spacing = 6.0;
        add(ui)
    })
}

/// 卡里一格输入框。边框自己画：egui 那套 `TextEdit` 底框按 `interact_size.y` 撑高，
/// 焦点那条描边还会溢出文字矩形——目视抓到的「蓝边顶到卡片右缘、两格读起来像一格」
/// 就是它。所以关掉它的 `frame`，只留文字与光标：底色在加控件**之前**铺（不能盖住字），
/// 描边在**之后**画（焦点要拿到 `Response` 才知道）。返回那个 `Response`，调用方拿它
/// 请求焦点（卡刚打开时让光标落在摘要那一格）。
pub(crate) fn card_field(
    ui: &mut egui::Ui,
    r: Rect,
    hint: &str,
    text: &mut String,
    rows: usize,
) -> egui::Response {
    let inner = r.shrink(1.0);
    ui.painter().rect_filled(inner, FIELD_R, Color32::WHITE);
    let te = if rows > 1 {
        egui::TextEdit::multiline(text)
    } else {
        egui::TextEdit::singleline(text)
    };
    let resp = ui.put(
        inner,
        te.hint_text(hint)
            .desired_width(inner.width() - 14.0)
            .desired_rows(rows.max(1))
            .align(if rows > 1 { Align2::LEFT_TOP } else { Align2::LEFT_CENTER })
            .margin(egui::Margin::symmetric(7, if rows > 1 { 5 } else { 3 }))
            .frame(egui::Frame::new().fill(Color32::TRANSPARENT)),
    );
    if resp.has_focus() {
        ui.painter().rect_stroke(
            r.expand(1.5),
            FIELD_R + 2.0,
            Stroke::new(2.5, ACCENT_LINE),
            egui::StrokeKind::Inside,
        );
    }
    let edge = if resp.has_focus() { ACCENT } else { BORDER_STRONG };
    ui.painter().rect_stroke(
        r,
        FIELD_R,
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    resp
}

/// 卡脚那一枚按钮。**主**那枚实底蓝、白字，**次**那枚白底灰边——之前两枚都走
/// `hunk_button` 那一档浅灰描边，鼠标扫过去分不清谁是「办这件事」、谁是「算了」。
/// 返回 `(点中了没, 实际宽度)`：宽度交回去，左边那枚才知道该从哪儿起。
/// 文字落在格子正中——判据（`locate_card` 整串相等 + 只看卡脚那一条带）认的就是它。
pub(crate) fn card_button(
    ui: &mut egui::Ui,
    text: &str,
    right: f32,
    cy: f32,
    primary: bool,
) -> (bool, f32) {
    let font = FontId::proportional(11.5);
    let fg = if primary { Color32::WHITE } else { TEXT_PRIMARY };
    let g = ui.painter().layout_no_wrap(text.to_owned(), font.clone(), fg);
    let w = (g.size().x + 20.0).max(BTN_MIN_W);
    let r = Rect::from_min_max(pos2(right - w, cy - BTN_H / 2.0), pos2(right, cy + BTN_H / 2.0));
    let resp = ui.allocate_rect(r, Sense::click());
    let p = ui.painter();
    let fill = if primary {
        if resp.is_pointer_button_down_on() {
            ACCENT_DOWN
        } else if resp.hovered() {
            ACCENT_HOVER
        } else {
            ACCENT
        }
    } else if resp.hovered() {
        BG_HOVER
    } else {
        Color32::WHITE
    };
    p.rect_filled(r, BTN_R, fill);
    if !primary {
        p.rect_stroke(r, BTN_R, Stroke::new(1.0, BORDER_STRONG), egui::StrokeKind::Inside);
    }
    p.galley(r.center() - g.size() / 2.0, g, fg);
    (resp.clicked(), w)
}

/// 卡里那一排页签：整条住在灰槽里，选中那一段浮成白、描边、字变深。
/// `selectable_label` 的底色铺满整格又不收口，三条并排看着像三行而不是「换页」，所以自己画。
/// 返回被选中的那一档。
pub(crate) fn seg_tabs<T: PartialEq + Clone>(
    ui: &mut egui::Ui,
    r: Rect,
    items: &[(&str, T)],
    current: &mut T,
) {
    let w = r.width() / items.len() as f32;
    ui.painter()
        .rect_filled(r.shrink(1.0), 7.0, SEG_BG);
    for (i, (label, value)) in items.iter().enumerate() {
        let seg = Rect::from_min_max(
            pos2(r.min.x + w * i as f32 + 2.0, r.min.y + 2.0),
            pos2(r.min.x + w * (i + 1) as f32 - 2.0, r.max.y - 2.0),
        );
        let resp = ui.allocate_rect(seg, Sense::click());
        if *current == *value {
            ui.painter().rect_filled(seg, 6.0, Color32::WHITE);
            ui.painter().rect_stroke(
                seg,
                6.0,
                Stroke::new(1.0, BORDER_STRONG),
                egui::StrokeKind::Inside,
            );
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, 6.0, Color32::from_rgb(0xea, 0xec, 0xf0));
        }
        let g = ui.painter().layout_no_wrap(
            (*label).to_owned(),
            FontId::proportional(11.5),
            if *current == *value { TEXT_PRIMARY } else { TEXT_SECONDARY },
        );
        ui.painter().galley(
            seg.center() - g.size() / 2.0,
            g,
            if *current == *value { TEXT_PRIMARY } else { TEXT_SECONDARY },
        );
        if resp.clicked() {
            *current = value.clone();
        }
    }
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
        Form::RenameBranch { from } => (!raw.is_empty())
            .then(|| Action::RenameBranch { from: from.clone(), to: raw.clone() }),
        Form::StashSave { include_untracked } => Some(Action::StashSave {
            message: if raw.is_empty() { "没有说明的贮藏".to_owned() } else { raw.clone() },
            include_untracked: *include_untracked,
        }),
        Form::Confirm { then } => then.take(),
        Form::AddRepo { tab, dest } => match tab {
            AddTab::Open => (!raw.is_empty()).then(|| Action::AddExisting { path: raw.clone() }),
            // 目标目录留空 = 主按钮按着不让你点，而不是替你猜一个 `~/xxx`。
            AddTab::Clone => {
                let dest = dest.trim().to_owned();
                (!raw.is_empty() && !dest.is_empty()).then(|| Action::Clone { url: raw.clone(), dest })
            }
            AddTab::New => (!raw.is_empty()).then(|| Action::Init { path: raw.clone() }),
        },
        Form::Commit { desc, amend, push_after } => {
            let msg = crate::ui::data::commit_text(&raw, desc);
            // 关卡看的是**摘要**，不是拼出来的整串：`commit_text` 会把空的摘要和描述拼成
            // 「\n\n描述」，那玩意儿非空、git 也真会收下，落出来的却是一条没有标题的提交。
            (!raw.is_empty()).then(|| if *push_after {
                Action::CommitAndPush { msg, amend: *amend }
            } else if *amend {
                Action::Amend(msg)
            } else {
                Action::Commit(msg)
            })
        }
        // 偏好卡没有「主动作」：开关当场就落了，「完成」只是把卡关掉（见 `primary_closes`）。
        Form::Settings { .. } => None,
        Form::Merge { .. } => None,
        // 推送卡：表里一条都没勾、又没勾「推送所有标签」→ 这一按没有可办的事，
        // 所以**不产出动作**（主按钮按下去卡还留着，和空名字的建分支卡同一条路）。
        // 远端那格空着同样按不动：`git push` 少了远端会退到 `origin`，那是替你猜。
        Form::Push { remote, rows, force, tags } => {
            let picked: Vec<PushRef> = rows
                .iter()
                .filter(|r| r.picked)
                .map(|r| PushRef {
                    local: r.local.clone(),
                    remote_name: r.remote_name.clone(),
                    track: r.track,
                })
                .collect();
            (!remote.trim().is_empty() && !(picked.is_empty() && !*tags)).then(|| Action::PushRefs {
                remote: remote.trim().to_owned(),
                refs: picked,
                force: *force,
                tags: *tags,
            })
        }
        // 重置卡：卡上那排页签已经替用户答过「丢什么」，主按钮不做关卡（关卡是那张卡本身）。
        Form::Reset { oid, mode } => Some(Action::Reset { oid: oid.clone(), mode: *mode }),
        // 拉取卡：远端和远程分支两格都得有值才谈得上跑 `git pull`。本地那格留空 =
        // 「拉进当前分支」，交给 git 自己按上游算，所以它不是关卡。
        Form::Pull { remote, branch, into, opts } => {
            let (r, b) = (remote.trim(), branch.trim());
            (!r.is_empty() && !b.is_empty()).then(|| Action::PullFrom {
                remote: r.to_owned(),
                branch: b.to_owned(),
                into: (!into.trim().is_empty()).then(|| into.trim().to_owned()),
                opts: *opts,
            })
        }
    }
}

/// 按下主按钮之后这张卡要不要自己关掉。
///
/// 单独一个函数而不是在卡片里再判一次形式：Enter 和鼠标是两条进来路，
/// 「关掉」这件事必须由**一处**决定，否则一半输入路径能杀掉、另一半不能（判据见
/// `the_settings_card_closes_from_both_enter_and_the_button`）。
pub(crate) fn primary_closes(m: &Modal) -> bool {
    matches!(m.form, Form::Settings { .. })
}

/// 一张卡摆多大。**高度跟着内容走**（2026-10-24）：这三张卡以前各自钉死在本机那三张 nib 的
/// 窗口尺寸（855×441 / 650×271 / 614×299），可里面的控件只有两三行，剩下的全是一片空白。
/// 现在只有「行高 / 标题带 / 卡脚」这三档是常量，其余按行数算；`rows()` 那套账适用于动作卡，
/// 推送卡按表里那几行算，偏好卡按**页里最多那一条**算（换页不让卡跳）。
pub(crate) fn card_rect(m: &Modal, screen: Rect) -> Rect {
    let clamp = |w: f32, h: f32| vec2(w.min(screen.width() - 40.0), h.min(screen.height() - 40.0));
    let fits = |body: f32| MODAL_TITLE_H + body + MODAL_FOOT_H;
    let size = match m.form {
        Form::Settings { .. } => clamp(PREF_W, fits(settings_body_h())),
        Form::Push { ref rows, .. } => clamp(PUSH_W, fits(push_body_h(rows.len()))),
        Form::Pull { .. } => clamp(PULL_W, fits(PULL_BODY_H)),
        _ => clamp(MODAL_W, fits(MODAL_ROW_H * m.rows() as f32)),
    };
    Rect::from_center_size(screen.center(), size)
}

/// 卡内容区里某一格的可画矩形：`top` 是内容区左上角，四个数全按 nib 量来的偏移算。
fn cell(top: Pos2, x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_max(pos2(top.x + x, top.y + y), pos2(top.x + x + w, top.y + y + h))
}

/// 内容区里靠左摆一格控件（和 `put_left` 同一条理由：`Ui::put` 会把短文字居中）。
fn cell_left<T>(ui: &mut egui::Ui, top: Pos2, x: f32, y: f32, w: f32, h: f32, add: impl FnOnce(&mut egui::Ui) -> T) -> T {
    put_left(ui, cell(top, x, y, w, h), |ui| add(ui))
}

/// 一枚弹出按钮，钉在内容区里量来的那一格。返回被选中的那一项，没选中就是 `None`。
/// 用 `popup_button` 那套 `MenuButton`（`.small()` 保住行高），只是外面要给它一个绝对矩形：
/// 卡片这一层没有布局流可跟。
fn cell_popup(ui: &mut egui::Ui, top: Pos2, x: f32, y: f32, w: f32, h: f32, label: &str, items: &[String]) -> Option<String> {
    let mut picked = None;
    cell_left(ui, top, x, y, w, h, |ui| {
        picked = popup_button(ui, label, items);
    });
    picked
}

/// 推送卡。字段与顺序来自 `GitPushSheet`（那两张 nib 的测量记录见 docs/PROCESS.md），
/// 但**摆位按内容排**：外框跟着表里那几行长，卡高跟着外框走（见 `card_rect`）。
/// 这一层只**改表单**，落到 git 的那一条由 `primary_action` 变出来——Enter 和鼠标共用那一个出口。
///
/// 表格里的行是打开那一刻从快照钉进来的（见 `Form::Push`），所以这里只改勾与名字。
pub(crate) fn push_card(
    ui: &mut egui::Ui,
    top: Pos2,
    remotes: &[Remote],
    remote: &mut String,
    rows: &mut Vec<PushRow>,
    force: &mut bool,
    tags: &mut bool,
    allow_force: bool,
) {
    let names: Vec<String> = remotes.iter().map(|r| r.name.clone()).collect();
    modal_label(ui, top, PUSH_LABEL_X, PUSH_ROW1_Y + 4.0, "推送到仓库：");
    if let Some(picked) =
        cell_popup(ui, top, PUSH_REMOTE_X, PUSH_ROW1_Y, PUSH_REMOTE_W, FIELD_H, remote.as_str(), &names)
    {
        *remote = picked;
    }
    // 那一格显示的是所选远端的 URL（nib 里它是个不可编辑的说明框）。
    let url = remotes
        .iter()
        .find(|r| &r.name == remote)
        .map(|r| r.push_url.clone().unwrap_or_else(|| r.url.clone()))
        .unwrap_or_default();
    modal_note(ui, top, PUSH_URL_X, PUSH_ROW1_Y + 4.0, PUSH_URL_W, &url);

    // 表格那一格的标题在框**外面**（左），「全选」在同一个标题行的右端——它原来孤零零浮在
    // 一只大半空着的框底下，看着像上一行的尾巴。
    modal_label(ui, top, PUSH_LABEL_X, PUSH_HEAD_LABEL_Y + 4.0, "要推送的分支");
    // 「全选」把表里每一条都勾上——它只动**这一张卡**上的勾，不碰仓库。
    if card_button(
        ui,
        "全选",
        top.x + PUSH_TABLE_RIGHT,
        top.y + PUSH_HEAD_LABEL_Y + 13.0,
        false,
    )
    .0
    {
        for row in rows.iter_mut() {
            row.picked = true;
        }
    }
    // 外框：顶边 = 表头顶边，底边 = 最后一行的下沿，**只裹住真有的那几行**。
    let n = rows.len();
    let box_rect = Rect::from_min_max(
        pos2(top.x + PUSH_TABLE_X, top.y + PUSH_TABLE_TOP),
        pos2(top.x + PUSH_TABLE_RIGHT, top.y + push_table_bottom(n)),
    );
    // `Painter` 外面套的是 Arc，clone 一份就不占着 `ui` 的借用（下面要横跨好几个控件画线）。
    let p = ui.painter().clone();
    p.rect_filled(box_rect, 6.0, Color32::WHITE);
    p.rect_filled(
        Rect::from_min_max(box_rect.min, pos2(box_rect.max.x, box_rect.min.y + PUSH_HEAD_H)),
        6.0,
        TABLE_HEAD_BG,
    );
    p.line_segment(
        [
            pos2(box_rect.min.x, box_rect.min.y + PUSH_HEAD_H),
            pos2(box_rect.max.x, box_rect.min.y + PUSH_HEAD_H),
        ],
        Stroke::new(1.0, BORDER_SUBTLE),
    );
    p.rect_stroke(box_rect, 6.0, Stroke::new(1.0, BORDER_STRONG), egui::StrokeKind::Inside);

    // 表头四列。列宽见 theme.rs。
    let mut x = PUSH_TABLE_X;
    for (title, w) in [
        ("推送？", PUSH_COL_PICK),
        ("本地分支", PUSH_COL_LOCAL),
        ("远程分支", PUSH_COL_REMOTE),
        ("跟踪？", PUSH_COL_TRACK),
    ] {
        modal_label(ui, top, x + 4.0, PUSH_TABLE_TOP + 4.0, title);
        x += w;
    }
    // 表格那几行。两列勾**在列里居中**：判据（`push_row_box`）按列正中点鼠标，
    // 方框贴在列左缘的话那一按就落到遮罩上。
    let col_local_x = PUSH_TABLE_X + PUSH_COL_PICK;
    let col_remote_x = col_local_x + PUSH_COL_LOCAL;
    let col_track_x = col_remote_x + PUSH_COL_REMOTE;
    for (i, row) in rows.iter_mut().enumerate() {
        let y = PUSH_TABLE_TOP + PUSH_HEAD_H + PUSH_ROW_H * i as f32;
        let band = Rect::from_min_max(
            pos2(top.x + PUSH_TABLE_X, top.y + y),
            pos2(top.x + PUSH_TABLE_RIGHT, top.y + y + PUSH_ROW_H),
        );
        if i + 1 < n {
            p.line_segment(
                [
                    pos2(band.min.x + 4.0, band.max.y),
                    pos2(band.max.x - 4.0, band.max.y),
                ],
                Stroke::new(1.0, Color32::from_rgb(0xf0, 0xf1, 0xf4)),
            );
        }
        let box_x = |left: f32, w: f32| left + (w - CHK_CARD) / 2.0;
        check_row(ui, Rect::from_min_max(pos2(top.x + box_x(PUSH_TABLE_X, PUSH_COL_PICK), band.min.y), pos2(top.x + box_x(PUSH_TABLE_X, PUSH_COL_PICK) + CHK_CARD, band.max.y)), |ui| {
            ui.checkbox(&mut row.picked, "")
        });
        modal_label(ui, top, col_local_x + 4.0, y + 2.0, &row.local);
        card_field(
            ui,
            Rect::from_min_max(
                pos2(top.x + col_remote_x + 3.0, band.min.y + 1.0),
                pos2(top.x + col_remote_x + PUSH_COL_REMOTE - 3.0, band.max.y - 1.0),
            ),
            "",
            &mut row.remote_name,
            1,
        );
        check_row(ui, Rect::from_min_max(pos2(top.x + box_x(col_track_x, PUSH_COL_TRACK), band.min.y), pos2(top.x + box_x(col_track_x, PUSH_COL_TRACK) + CHK_CARD, band.max.y)), |ui| {
            ui.checkbox(&mut row.track, "")
        });
    }
    // 框下面那两枚勾。
    let y = push_checks_y(n);
    modal_switch(ui, top, PUSH_LABEL_X, y, "推送所有标签", tags);
    // 「允许强制推送」是偏好里的那道闸：关着的时候这一枚**摆都不摆**——
    // 摆一枚点了没用的勾等于没有闸（和偏好卡「三条都真接得通才摆上来」同一条规矩）。
    if allow_force {
        modal_switch(ui, top, PUSH_LABEL_X + 130.0, y, "强制推送", force);
    }
}

/// 表格外框的下沿（内容区坐标）。
fn push_table_bottom(n: usize) -> f32 {
    PUSH_TABLE_TOP + PUSH_HEAD_H + PUSH_ROW_H * n as f32
}

/// 拉取卡。字段与文案来自 `GitPullSheet`，摆位按内容排（同推送卡，见 `card_rect`）。
/// 返回「刷新」被点了没。
pub(crate) fn pull_card(
    ui: &mut egui::Ui,
    top: Pos2,
    remotes: &[Remote],
    remote: &mut String,
    branches: &[String],
    branch: &mut String,
    locals: &[String],
    into: &mut String,
    opts: &mut git::PullOpts,
) -> bool {
    let names: Vec<String> = remotes.iter().map(|r| r.name.clone()).collect();
    modal_label(ui, top, PULL_LABEL_X, PULL_REMOTE_Y + 4.0, "从仓库拉取：");
    if let Some(picked) =
        cell_popup(ui, top, PULL_FIELD_X, PULL_REMOTE_Y, PULL_REMOTE_W, FIELD_H, remote.as_str(), &names)
    {
        *remote = picked;
    }
    let url = remotes
        .iter()
        .find(|r| &r.name == remote)
        .map(|r| r.url.clone())
        .unwrap_or_default();
    modal_note(ui, top, PULL_FIELD_X, PULL_URL_Y, PULL_REMOTE_W, &url);
    modal_label(ui, top, PULL_LABEL_X, PULL_BRANCH_Y + 4.0, "要拉取的远程分支：");
    // 那一列的选项来自**当前那份快照**的远程跟踪引用（按选中的远端剥前缀），
    // 「刷新」跑完 fetch、新快照落地，它跟着变——所以选项不是卡打开那一刻钉死的那一批。
    let empty = "选择一个分支";
    let current = if branch.is_empty() { empty } else { branch.as_str() };
    if let Some(picked) =
        cell_popup(ui, top, PULL_FIELD_X, PULL_BRANCH_Y, PULL_BRANCH_W, FIELD_H, current, branches)
    {
        *branch = picked;
    }
    // 「刷新」和它刷的那一列同一行、钉在同一行的右端（原来它浮在正文右上角，
    // 离哪一格都不像）。它跑的是 `git fetch <远端>`，只动远端那一列，不改工作区。
    let refresh = card_button(
        ui,
        "刷新",
        top.x + PULL_W - MODAL_PAD,
        top.y + PULL_BRANCH_Y + FIELD_H / 2.0,
        false,
    )
    .0;
    modal_label(ui, top, PULL_LABEL_X, PULL_INTO_Y + 4.0, "拉取到本地分支：");
    let empty2 = "选择本地分支";
    let current2 = if into.is_empty() { empty2 } else { into.as_str() };
    if let Some(picked) =
        cell_popup(ui, top, PULL_FIELD_X, PULL_INTO_Y, PULL_BRANCH_W, FIELD_H, current2, locals)
    {
        *into = picked;
    }
    // 「选项」那只框：标题在框外，框只裹住真摆出来的那几行。
    modal_label(ui, top, PULL_LABEL_X, PULL_OPT_LABEL_Y + 4.0, "选项");
    let box_rect = Rect::from_min_max(
        pos2(top.x + PULL_LABEL_X, top.y + PULL_BOX_TOP),
        pos2(top.x + PULL_W - MODAL_PAD, top.y + PULL_BOX_BOTTOM),
    );
    ui.painter().rect_filled(box_rect, 6.0, Color32::from_rgb(0xfb, 0xfc, 0xfd));
    ui.painter().rect_stroke(
        box_rect,
        6.0,
        Stroke::new(1.0, BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
    modal_switch(ui, top, PULL_OPT_X, PULL_OPT_LOG_Y, "包括被合并提交的信息内容", &mut opts.log);
    modal_switch(ui, top, PULL_OPT_X, PULL_OPT_NOFF_Y, "无论是否可以快进更新都创建新的提交", &mut opts.no_ff);
    modal_switch(
        ui,
        top,
        PULL_OPT_X,
        PULL_OPT_REBASE_Y,
        "用变基代替合并 (警告：请确保您还没有推送您的变更)",
        &mut opts.rebase,
    );
    refresh
}

/// 内容区里一格灰色说明文字（远端 URL 那一格，nib 里是个不可编辑的说明框）。
fn modal_note(ui: &mut egui::Ui, top: Pos2, x: f32, y: f32, w: f32, text: &str) {
    cell_left(ui, top, x, y, w, 18.0, |ui| {
        ui.add(Label::new(RichText::new(text).size(11.0).color(TEXT_SECONDARY)).truncate().selectable(false));
    });
}

/// 内容区里一格说明文字（不可点）。走 `put_left` + `Label`，和卡上其余文字同一套截断规则。
fn modal_label(ui: &mut egui::Ui, top: Pos2, x: f32, y: f32, text: &str) {
    cell_left(ui, top, x, y, 320.0, 18.0, |ui| {
        ui.add(Label::new(RichText::new(text).size(11.5).color(TEXT_PRIMARY)).selectable(false));
    });
}

/// 内容区里一枚开关（推送卡那两枚、拉取卡那三枚用的都是它）。
fn modal_switch(ui: &mut egui::Ui, top: Pos2, x: f32, y: f32, label: &str, on: &mut bool) {
    check_row(
        ui,
        cell(top, x, y, 480.0, CHK_CARD),
        |ui| ui.checkbox(on, RichText::new(label).size(11.5)),
    );
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
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, LOG_HEAD_H), Sense::hover());
    let p = ui.painter();
    let font = &ui.style().text_styles[&TextStyle::Small];
    let right = rect.max.x;
    // 表头文字走 §3.4 的「次要」档（预览稿 `.lh` 同一条），列的排法仍跟着数据走：参考图那四个词（提交 / 描述 / 作者 / 日期）离自己列的左边线
    // 都是 5pt，跟数据一模一样；只有「图表」是在列里**居中**的（159..185，列 141..198）。
    let g = band(rect, rect.min.x, COL_GRAPH, 13.0);
    p.text(g.center_top(), Align2::CENTER_TOP, "图表", font.clone(), TEXT_SECONDARY);
    for (x, w, label) in [
        (rect.min.x + COL_GRAPH + COL_INSET, COL_SHA - COL_INSET, "提交"),
        (rect.min.x + COL_GRAPH + COL_SHA + COL_INSET, 200.0, "描述"),
        (right - COL_DATE - COL_AUTHOR + COL_INSET, COL_AUTHOR - COL_INSET, "作者"),
        (right - COL_DATE + COL_INSET, COL_DATE - COL_INSET, "日期"),
    ] {
        p.text(band(rect, x, w, 13.0).left_top(), Align2::LEFT_TOP, label, font.clone(), TEXT_SECONDARY);
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
        p.line_segment([pos2(x, y0), pos2(x, y0 + RULE_H)], Stroke::new(1.0, BORDER_SUBTLE));
    }
    // 表头上下各一条横线，把这一条从筛选条与日志里隔开（参考图 y=78、y=103）。
    p.line_segment(
        [pos2(rect.min.x, rect.min.y + 0.5), pos2(right, rect.min.y + 0.5)],
        Stroke::new(1.0, BORDER_SUBTLE),
    );
    p.line_segment(
        [pos2(rect.min.x, rect.max.y - 0.5), pos2(right, rect.max.y - 0.5)],
        Stroke::new(1.0, BORDER_SUBTLE),
    );
}

// ------------------------------------------------------------------------ 小组件
/// 顶栏动作组的一枚：28×28 命中、圆角 6、图标 14、hover `BG_HOVER`（§4.1 图标按钮）。
/// **中文不进顶栏**（§2：那一行只有 tab、分支、按钮）——标签住在 tooltip 里，
/// 等 §2 那句「溢出收进 `»` 菜单」落地时它还会住在菜单项里。
pub(crate) fn icon_button(ui: &mut egui::Ui, i: usize) -> bool {
    let (icon, _label, hint) = TOOLS[i];
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(ICON_BTN), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, ICON_BTN_R, BG_HOVER);
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, icon, FontId::proportional(ICON_PX), TEXT_SECONDARY);
    resp.on_hover_text(hint).clicked()
}

/// 一枚仓库 tab：**胶囊开关**（§2「高 26、圆角 999，选中 = 主色浅底深字」；预览稿 `.tab`）。
/// 宽度贴着名字现量（`layout_no_wrap`），左右各 `TAB_PILL_PAD`。
/// 圆角 999 落到 egui：`CornerRadius` 是 u8，取半高 `TAB_PILL_R` 画出来就是同一颗胶囊。
/// 不用 `selectable_label`：它的底色铺满整格且不带圆角，看着不像胶囊。
pub(crate) fn repo_tab(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    // 选中那一枚走真字重（预览稿 `.tab.on{font-weight:600}`）；§3.4 说 600 只有拉丁/数字吃得进
    // 真字重，仓库名正是拉丁。
    let font = if active {
        FontId { size: TAB_FONT, family: crate::ui::fonts::semibold_family(), ..FontId::proportional(TAB_FONT) }
    } else {
        FontId::proportional(TAB_FONT)
    };
    let ink = if active { ACCENT_INK } else { TEXT_SECONDARY };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, ink);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * TAB_PILL_PAD, TAB_PILL_H), Sense::click());
    let p = ui.painter();
    let fill = if active { ACCENT_SOFT } else if resp.hovered() { BG_HOVER } else { Color32::TRANSPARENT };
    p.rect(rect, egui::CornerRadius::same(TAB_PILL_R), fill, Stroke::NONE, egui::StrokeKind::Inside);
    p.galley(rect.min + vec2(TAB_PILL_PAD, (rect.height() - galley.size().y) / 2.0), galley, ink);
    resp.clicked()
}

/// 「＋ 添加仓库」那一枚**幽灵** tab（预览稿 `.tab.add`）：不铺底，只描一圈边、字走辅助档。
/// 它是空列表时**唯一**的入口，所以画得再轻也不能省。
/// spec 写的是 `1px dashed`：egui 0.37 的 `Stroke` 给不了圆角矩形的虚线（`PathPaint` 没有
/// dash 字段），先落 1px `BORDER_STRONG` 实线，虚线这一档自定让步登记在 PROCESS。
pub(crate) fn ghost_tab(ui: &mut egui::Ui, label: &str) -> bool {
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), TEXT_MUTED);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * TAB_PILL_PAD, TAB_PILL_H), Sense::click());
    let p = ui.painter();
    p.rect(rect, egui::CornerRadius::same(TAB_PILL_R), Color32::TRANSPARENT, Stroke::new(1.0, BORDER_STRONG), egui::StrokeKind::Inside);
    p.galley(rect.min + vec2(TAB_PILL_PAD, (rect.height() - galley.size().y) / 2.0), galley, TEXT_MUTED);
    resp.clicked()
}

/// 侧栏的**子项**行（§2「子项行高 26，选中 = 浅蓝药丸（圆角 6）」）：整行可点，文字钉死在
/// `SIDE_TEXT_X` 那一列，选中铺一枚 `ACCENT_SOFT` 药丸，`current` 那一条在文字左边的空槽里
/// 补一颗蓝点、文字走 `medium_family()`（§2「当前分支 = 蓝点 + 中黑字」）。
///
/// **返回 Response 而不是 bool**：分支、标签、贮藏那几行要挂右键菜单，而 `context_menu`
/// 只能问 Response。调用方自己问 `.clicked()`。
///
/// 不走 `ui.label` 的自动布局是因为布局系统给不了「文字左缘 = 面板 x=24」这个约束：
/// 滚动区、内边距、间距每一层都会挪它，而 §2.1 那一栏的数全是网格上的绝对位置
/// （8 内边距 + 8 缩进）。截断仍要 `put_left`：分支名可以长到放不下。
pub(crate) fn side_row(ui: &mut egui::Ui, text: &str, selected: bool, current: bool) -> egui::Response {
    side_row_at(ui, text, selected, current, SIDE_TEXT_X)
}

/// 侧栏的**第三级**行：远端点开之后那些分支。和 `side_row` 共用同一套画法（`side_row_at`），
/// 只把文字往右再让一档 `SIDE_SUB_X`。单独一个入口而不是给 `side_row` 加参数：
/// 其余调用点全都传同一个 24，多一个参数等于让每一处都重复那一个数。
pub(crate) fn side_sub_row(ui: &mut egui::Ui, text: &str) -> egui::Response {
    side_row_at(ui, text, false, false, SIDE_TEXT_X + SIDE_SUB_X)
}

/// `side_row` / `side_sub_row` 的公共体。`text_x` 是**文字**那一列的位置，行矩形、药丸、
/// 蓝点仍然按整套侧栏网格铺（缩进只往右挪文字，不挪命中区——那一行横竖都是整行可点）。
fn side_row_at(ui: &mut egui::Ui, text: &str, selected: bool, current: bool, text_x: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), SIDE_ROW_H), Sense::click());
    // 行矩形的左边就是内容框的左边；减掉内缩才换回那套面板坐标。
    let base = rect.min.x - SIDE_INSET as f32;
    let p = ui.painter();
    if selected {
        p.add(egui::Shape::rect_filled(
            Rect::from_min_max(
                pos2(base + SIDE_PILL_L, rect.center().y - SIDE_ROW_H / 2.0),
                pos2(base + SIDE_PILL_R, rect.center().y + SIDE_ROW_H / 2.0),
            ),
            SIDE_PILL_RAD as f32,
            ACCENT_SOFT,
        ));
    }
    if current {
        p.circle(pos2(base + SIDE_DOT_X, rect.center().y), SIDE_DOT_R, ACCENT, Stroke::NONE);
    }
    let x = base + text_x;
    put_left(ui, Rect::from_min_max(pos2(x, rect.min.y), pos2(base + SIDEBAR_W - 4.0, rect.max.y)), |ui| {
        // 文字的 `Label` 别开 `selectable_labels`，不然它压在整行的点击矩形上面，
        // 按下被文字选择抢走（见 `log_row` 里那段注释）。
        let mut rich = RichText::new(text).size(SIDE_ITEM_FONT).color(TEXT_PRIMARY);
        if current {
            rich = rich.family(crate::ui::fonts::medium_family());
        }
        ui.add(Label::new(rich).truncate().selectable(false));
    });
    resp
}

/// 侧栏的**分组头**（§2「11px 灰字 + 右对齐计数，无图标」）：占 `SIDE_GROUP_H` 一行，
/// 标题起在 `SIDE_GROUP_X`（比子项靠左半档），计数右对齐贴在 `SIDE_COUNT_X`。
/// 计数那一档的颜色 §2 没说，取 `TEXT_MUTED`（比组头字再淡一档，§3.1 的辅助档）——
/// 这是自己拍的，登记在 `DESIGN.md` §7。
/// **不做折叠**：§2.1 那句「点击折叠」归 S6-3b 和仓库分组一起接，这一批只换皮肤与网格。
pub(crate) fn side_group_row(ui: &mut egui::Ui, title: &str, count: usize) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), SIDE_GROUP_H), Sense::hover());
    let base = rect.min.x - SIDE_INSET as f32;
    let p = ui.painter();
    let cy = rect.center().y;
    p.text(pos2(base + SIDE_GROUP_X, cy), Align2::LEFT_CENTER, title, FontId::proportional(SIDE_GROUP_FONT), TEXT_SECONDARY);
    p.text(pos2(base + SIDE_COUNT_X, cy), Align2::RIGHT_CENTER, count.to_string(), FontId::proportional(SIDE_COUNT_FONT), TEXT_MUTED);
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
    /// 侧栏「远端」里要点开/收起的**那一条的名字**。放在 `Intent` 而不是就地改
    /// `self.remote_open`：这一段跑在面板闭包里，`self` 正被借走。
    pub(crate) toggle_remote: Option<String>,
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
    matches!(
        a,
        Action::DeleteBranch(_)
            | Action::DeleteTag(_)
            | Action::StashPop(_)
            | Action::StashDrop(_)
            | Action::RestoreFile { .. }
            // 「用我的/他们的解决」会用那一版覆盖工作区里正在编辑的冲突文件；
            // 「标记为未解决」会把冲突标记重新盖回去。两件都能丢手工改动。
            | Action::ResolveOurs(_)
            | Action::ResolveTheirs(_)
            | Action::MarkUnresolved(_)
            // 继续/终止都会落一次提交或回滚整段操作，SourceTree 同样先问一句。
            | Action::ContinueOp { .. }
            | Action::AbortOp { .. }
            // amend 换掉的是已经落地的上一条提交；推送过的仓库这一下会让本地与远端分叉。
            | Action::Amend(_)
    )
}

/// 两种操作轨迹在横幅和确认卡上的叫法。SourceTree 的中文文案：合并叫「合并」，
/// 遴选叫「遴选」，变基叫「变基」。
pub(crate) fn op_noun(state: &str) -> &'static str {
    match state {
        "merge" => "合并",
        "cherry-pick" => "遴选",
        "rebase" => "变基",
        _ => "操作",
    }
}

/// 横幅标题：「未解决的合并：」「未解决的变基:」——后一个冒号 SourceTree 用的是半角，
/// 照抄。没有进行中的操作时返回 `None`，调用方据此决定画不画横幅。
pub(crate) fn op_title(state: &str) -> Option<&'static str> {
    match state {
        "merge" => Some("未解决的合并："),
        "cherry-pick" => Some("未解决的遴选："),
        "rebase" => Some("未解决的变基:"),
        _ => None,
    }
}

/// 「继续」那一枚按钮上的字。合并就是「继续」，另外两种带上操作名（SourceTree 同款）。
pub(crate) fn continue_label(state: &str) -> String {
    match state {
        "merge" => "继续".to_owned(),
        other => format!("继续{}", op_noun(other)),
    }
}

/// 「终止」那一枚。变基在 SourceTree 里叫「放弃变基」，另外两种叫「终止…」。
pub(crate) fn abort_label(state: &str) -> String {
    match state {
        "merge" => "终止".to_owned(),
        "rebase" => "放弃变基".to_owned(),
        other => format!("终止{}", op_noun(other)),
    }
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
        Action::Amend(_) => (
            "修正上一条提交",
            "上一条提交会被新的信息和暂存内容替换。没推送过就没有影响；推送过的分支要强推才能更新远端。"
                .to_owned(),
        ),
        Action::RestoreFile { oid, path } => (
            "恢复到旧版本",
            format!(
                "{} 会被 {oid} 那一版覆盖，工作区和暂存区里它现在的改动都会没。",
                path,
                oid = &oid[..oid.len().min(8)]
            ),
        ),
        Action::ResolveOurs(path) => (
            "确认使用「我的版本」解决冲突",
            format!("{path} 会被当前分支那一版覆盖，工作区里它现在的手工改动都会没。"),
        ),
        Action::ResolveTheirs(path) => (
            "确认使用「他们的版本」解决冲突",
            format!("{path} 会被传入那一版覆盖，工作区里它现在的手工改动都会没。"),
        ),
        Action::MarkUnresolved(path) => (
            "确认标记为未解决",
            format!("{path} 会被重新写上冲突标记，工作区里它现在的改动会被覆盖。"),
        ),
        Action::ContinueOp { state } => (
            "继续操作？",
            match state.as_str() {
                "rebase" => "您确定要继续此变基操作吗？请确保您已经解决了所有待解决的冲突。".to_owned(),
                other => format!("确认所有冲突都已解决，然后完成这次{}。", op_noun(other)),
            },
        ),
        Action::AbortOp { state } => (
            "中止操作？",
            match state.as_str() {
                "rebase" => "您确定要中止这次变基吗？".to_owned(),
                other => format!("这次{}会整个撤销，已经解掉的那部分改动也会没。", op_noun(other)),
            },
        ),
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

/// 重置卡三档的文案：`(页签上那一格, 卡里那句「这一档丢什么」)`。
///
/// 一句 `match`、不是两张表：页签和下面那行必须说的是同一件事，分开写就成了「改一半、
/// 剩一半」——那正是本项目判据抓过的那族假绿。返回的是 `&'static str`，所以卡上不会有
/// 一条由变量拼出来的警告，判据钉得住每一句完整的话。
pub(crate) fn reset_words(mode: git::ResetMode) -> (&'static str, &'static str) {
    use git::ResetMode as M;
    match mode {
        M::Soft => ("都保留（软）", "分支指针挪走，暂存区和工作区里的改动一条都不动。"),
        M::Mixed => ("取消暂存（混合）", "改动都留在工作区，但暂存区清空，要重新暂存一遍。"),
        M::Hard => ("丢弃改动（硬）", "工作区和暂存区都被那条提交覆盖，未提交的改动当场没了、找不回来。"),
    }
}

/// 三档**共同**的那一句：挪分支指针意味着什么，和选哪档无关，所以它不跟着页签变。
/// 「还在对象库里、reflog 找得回来」这句要紧——用户听到「历史没了」的第一反应是恐慌，
/// 而三条提交其实一条都没被删。
pub(crate) const RESET_SHARES: &str = "那之后的提交会离开这条分支，但对象还在，用 reflog 找得回来。";

/// 日志一行的右键菜单。`subject` 只用来给「打标签」那张卡当说明——卡上的说明必须是
/// **打开那一刻**那条提交的话，之后日志翻页不能让它换内容。
pub(crate) fn commit_menu(ui: &mut egui::Ui, oid: &str, subject: &str, intent: &mut Intent) {
    let full = oid.to_owned();
    // 文案里写明「分离 HEAD」：git 的 `checkout <oid>` 就是这样，而只写「检出这个提交」的话，
    // 用户点完发现上面的提交不见了，会以为是软件吞了历史（状态栏那边另有「回到 main」那一格）。
    menu_item(ui, "检出这条提交（分离 HEAD）", || Action::Checkout(full.clone()), intent);
    menu_card(ui, "从这里新建分支", || Modal::new_branch_at(Some(full.clone())), intent);
    menu_card(ui, "在这条提交上打标签", || Modal::new_tag(full.clone(), subject.to_owned()), intent);
    menu_item(ui, "合并到当前分支", || Action::Merge(full.clone()), intent);
    // 重置走**卡**而不是确认框：三档的差别是「你的改动还在不在」，一句话的确认框装不下，
    // 而这一句正是用户按下去之前唯一想知道的事。名字里写清「当前分支」——挪的是站着的那条。
    menu_card(
        ui,
        "把当前分支重置到这条提交…",
        || Modal::reset(full.clone(), subject.to_owned()),
        intent,
    );
    // 归档只读对象库、不动工作区，所以不过确认卡；但它要**问一个存盘位置**，
    // 于是这一行交回去的只是 oid（见 `Action::Archive`：面板在下一帧之外才跑）。
    menu_item(ui, "归档成 zip", || Action::Archive(full.clone()), intent);
    if ui.button("复制完整提交号").clicked() {
        ui.ctx().copy_text(full.clone());
        ui.close();
    }
}

/// 侧栏的一个分组：`side_group_row` 那一行 11px 标题 + 右对齐计数，下面挂筛选过的子项。
/// `selected` 为真的铺药丸，`current` 为真的补蓝点 + 中黑字——§2 把这两件事**分开写**
/// （「选中 = 浅蓝药丸」/「当前分支 = 蓝点 + 中黑字」），所以是两个闭包不是一个：
/// 分支那组的 `current` 是 `is_current`，而它没有"被选中"这一说（点它就是切过去）；
/// 贮藏那组反过来，`selected` 跟着 `Sel::Stash(i)` 走，永远没有蓝点。
/// 计数用**筛选前**的规模：筛选是找东西，计数是这一组有多大。
///
/// 两个回调都不许捕获 `self`（它们跑在 `Panel::show` 的闭包里，那时 `self` 正被借走），
/// 要发生什么一律往交回去的那个 `Intent` 里放。`on_menu` 只在**这一行的右键菜单真的被打开**
/// 那一帧才被调用（egui 在里面判）。
///
/// `children` 是**第三级**：那一行的下面要不要再摆几行、摆什么。它每一帧都被问一次，
/// 由自己决定画不画（远端那组看的是「这一条展开没有」）。分成两个闭包而不是把子项塞进
/// `items`：计数报的是这一组有几条、筛选筛的也是这一组的条目名，摊平进去两件都会跟着变。
pub(crate) fn side_category<T>(
    ui: &mut egui::Ui,
    title: &str,
    filter: &str,
    items: &[T],
    mut label_of: impl FnMut(&T) -> String,
    selected: impl Fn(&T) -> bool,
    current: impl Fn(&T) -> bool,
    mut on_click: impl FnMut(&T, &mut Intent),
    mut on_menu: impl FnMut(&mut egui::Ui, &T, &mut Intent),
    mut on_children: impl FnMut(&mut egui::Ui, &T, &mut Intent),
    intent: &mut Intent,
) {
    let q = filter.trim().to_lowercase();
    let hits: Vec<(String, &T)> = items
        .iter()
        .map(|it| (label_of(it), it))
        .filter(|(label, _)| label.to_lowercase().contains(&q))
        .collect();
    // 计数报的是**这一组有几条**（筛选前），不是筛完剩几条：筛选是找东西，计数是这组的规模。
    side_group_row(ui, title, items.len());
    for (label, it) in hits {
        let resp = side_row(ui, &label, selected(it), current(it));
        resp.context_menu(|ui| on_menu(ui, it, intent));
        if resp.clicked() {
            on_click(it, intent);
        }
        on_children(ui, it, intent);
    }
}

// ---- 侧栏图标：§2 把分组头改成「11px 灰字 + 右对齐计数，**无图标**」，这一整批线描
// （显示器 / 分支 / 标签 / 云 / 抽屉 / 圆点）跟着 `side_row` 的 `icon` 参数一起删掉了。

/// 一组文件 + 组标题复选框（参考图第 5 条：勾 = 这一组都在暂存区，取消勾就是整组 unstage）。
pub(crate) fn section(
    ui: &mut egui::Ui,
    title: &str,
    files: &[&FileStatus],
    staged: bool,
    current: &Option<String>,
    act: &mut Option<Action>,
    pick: &mut Option<Target>,
    hist: &mut Option<String>,
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
        resp.context_menu(|ui| file_menu(ui, &path, is_staged, untracked, act, hist));
    }
}

/// 冲突文件那一组。**没有组复选框、没有「放弃整组」**：冲突不是「在/不在暂存区」那种
/// 两态，得一个一个选边或标记已解决。行本身也不带方框（`file_row` 传 `check: None`），
/// 否则用户会以为勾上就等于解决了。
pub(crate) fn conflict_section(
    ui: &mut egui::Ui,
    files: &[&FileStatus],
    current: &Option<String>,
    act: &mut Option<Action>,
    pick: &mut Option<Target>,
    hist: &mut Option<String>,
) {
    if files.is_empty() {
        return;
    }
    let (hrect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());
    put_left(
        ui,
        Rect::from_min_max(
            pos2(hrect.min.x + CHK_HEAD_X + CHK_S + 7.0, hrect.min.y),
            pos2(hrect.max.x, hrect.max.y),
        ),
        |ui| {
            ui.add(
                Label::new(
                    RichText::new(format!("冲突的文件（{}）", files.len()))
                        .size(11.5)
                        .strong()
                        .color(WARN),
                )
                .selectable(false)
                .sense(Sense::hover()),
            );
        },
    );
    fix_pitch(ui, hrect);
    for f in files {
        let path = f.path.clone();
        let (resp, _) = file_row(
            ui,
            &f.path,
            &ChangeStatus::Unmerged,
            current.as_deref() == Some(f.path.as_str()),
            "",
            None,
        );
        if resp.clicked() {
            // 冲突文件的差异要看**工作区那一份**：`git diff --cached` 对未合并路径不好读，
            // 而带冲突标记的正文在 `git diff -- path` 里。
            *pick = Some(Target::Working {
                staged: false,
                path: Some(path.clone()),
                untracked: false,
            });
        }
        resp.context_menu(|ui| conflict_menu(ui, &path, act, hist));
    }
}

/// 进行中的合并/遴选/变基横幅。只在 `repo_state != "none"` 时画，正常仓库的几何一点不动。
/// `continue` 与 `abort` 都走 `land` 那条路（由 `needs_confirm` 拦一道）。
pub(crate) fn op_banner(ui: &mut egui::Ui, state: &str, conflicts: usize, act: &mut Option<Action>) {
    let Some(title) = op_title(state) else { return };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), CONFLICT_BANNER_H), Sense::hover());
    ui.painter().rect_filled(rect.shrink2(vec2(0.0, 2.0)), 4.0, WARN_BG);
    let text = if conflicts > 0 {
        format!("{title}冲突：{conflicts}")
    } else {
        title.to_owned()
    };
    put_left(
        ui,
        Rect::from_min_max(pos2(rect.min.x + 10.0, rect.min.y + 4.0), pos2(rect.max.x - 150.0, rect.max.y - 4.0)),
        |ui| {
            ui.add(Label::new(RichText::new(text).size(11.5).color(WARN)).truncate().selectable(false));
        },
    );
    let mut next: Option<Action> = None;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(pos2(rect.max.x - 150.0, rect.min.y + 4.0), pos2(rect.max.x - 6.0, rect.max.y - 4.0)))
            .layout(egui::Layout::right_to_left(Align::Center)),
        |ui| {
            if ui.small_button(abort_label(state)).clicked() {
                next = Some(Action::AbortOp { state: state.to_owned() });
            }
            if ui.small_button(continue_label(state)).clicked() {
                next = Some(Action::ContinueOp { state: state.to_owned() });
            }
        },
    );
    fix_pitch(ui, rect);
    if next.is_some() {
        *act = next;
    }
}

/// 历史面板里的一句空态/加载提示。贴在给定区域左上角，不居中——列表空着的时候
/// 一条居中的字会让人以为整个面板坏了。
pub(crate) fn history_note(ui: &mut egui::Ui, area: Rect, text: &str) {
    put_left(
        ui,
        Rect::from_min_max(pos2(area.min.x + 4.0, area.min.y + 6.0), pos2(area.max.x, area.min.y + 26.0)),
        |ui| {
            ui.add(Label::new(RichText::new(text).size(11.0).color(TEXT_SECONDARY)).truncate().selectable(false));
        },
    );
}

/// 历史面板右侧的一行补丁。和差异区画法一致，但不带 hunk 按钮：
/// 这里是**已落库的提交**，没有区块可暂存。
pub(crate) fn history_diff_row(ui: &mut egui::Ui, width: f32, line: &DiffLine) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, DIFF_ROW_H), Sense::hover());
    let p = ui.painter_at(rect);
    let bg = match line.kind {
        Kind::Add => ADD_BG,
        Kind::Del => DEL_BG,
        Kind::Hunk => BG_INSET,
        // 历史面板每行都是单文件补丁（`layout(..., false)`），不会出现文件头。
        Kind::File | Kind::Ctx => Color32::TRANSPARENT,
    };
    if bg != Color32::TRANSPARENT {
        p.rect_filled(rect, 0.0, bg);
    }
    let font = ui.style().text_styles[&TextStyle::Monospace].clone();
    let th = ui.text_style_height(&TextStyle::Monospace);
    let fg = ui.visuals().text_color();
    for (i, n) in [(0, line.old), (1, line.new)] {
        let text = n.map(|v| v.to_string()).unwrap_or_default();
        let r = band(rect, rect.min.x + NO_W * i as f32, NO_W, th);
        p.text(r.right_center(), Align2::RIGHT_CENTER, text, font.clone(), TEXT_MUTED);
    }
    let x = rect.min.x + NO_W * 2.0 + 6.0;
    let rich = match line.kind {
        Kind::Hunk => RichText::new(&line.text).font(font.clone()).color(TEXT_MUTED),
        // 和差异区同一条例（§3.1）：增/删行连字一起染色。
        Kind::Add => RichText::new(&line.text).font(font.clone()).color(ADD_FG),
        Kind::Del => RichText::new(&line.text).font(font.clone()).color(DEL_FG),
        _ => RichText::new(&line.text).font(font).color(fg),
    };
    put_left(ui, band(rect, x, (rect.max.x - x).max(40.0), th), |ui| {
        ui.add(Label::new(rich).truncate());
    });
}

/// 历史面板右侧的全文行：左边一行号，右边原文。行号从 1 数。
pub(crate) fn history_text_row(ui: &mut egui::Ui, width: f32, line_no: usize, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, DIFF_ROW_H), Sense::hover());
    let p = ui.painter_at(rect);
    let font = ui.style().text_styles[&TextStyle::Monospace].clone();
    let th = ui.text_style_height(&TextStyle::Monospace);
    let fg = ui.visuals().text_color();
    p.text(
        pos2(rect.min.x + 36.0, rect.center().y),
        Align2::RIGHT_CENTER,
        line_no.to_string(),
        font.clone(),
        TEXT_MUTED,
    );
    let x = rect.min.x + 44.0;
    put_left(ui, band(rect, x, (rect.max.x - x).max(40.0), th), |ui| {
        ui.add(Label::new(RichText::new(text).font(font).color(fg)).truncate());
    });
}

/// 文件历史列表的一行：色块 + 提交说明 + 日期，第二行是提交号/作者/改动类型。
/// 行高钉在 [`HIST_ROW_H`]，和日志一样由 `show_rows` 定周期，不交给布局系统。
pub(crate) fn history_row(
    ui: &mut egui::Ui,
    width: f32,
    entry: &FileLogEntry,
    date: &str,
    active: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, HIST_ROW_H), Sense::click());
    let p = ui.painter_at(rect);
    if active {
        p.rect_filled(rect, 0.0, SELECTED);
    } else if resp.hovered() {
        p.rect_filled(rect, 0.0, BG_HOVER);
    }
    let (fill, glyph) = badge_of(&entry.change);
    let badge = Rect::from_min_size(pos2(rect.min.x + HIST_PAD, rect.min.y + 7.0), Vec2::splat(BADGE_S));
    p.rect_filled(badge, BADGE_R, fill);
    p.text(badge.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(11.0), Color32::WHITE);
    let ink = if active { Color32::WHITE } else { TEXT_PRIMARY };
    let meta = if active { Color32::WHITE } else { TEXT_SECONDARY };
    let left = rect.min.x + HIST_PAD + BADGE_S + 7.0;
    put_left(
        ui,
        Rect::from_min_max(pos2(left, rect.min.y + 3.0), pos2(rect.max.x - 76.0, rect.min.y + 22.0)),
        |ui| {
            ui.add(
                Label::new(RichText::new(&entry.commit.subject).size(11.5).color(ink))
                    .truncate()
                    .selectable(false)
                    .sense(Sense::hover()),
            )
        },
    );
    p.text(
        pos2(rect.max.x - HIST_PAD, rect.min.y + 13.0),
        Align2::RIGHT_CENTER,
        date,
        FontId::proportional(10.5),
        meta,
    );
    let detail = format!(
        "{} · {} · {}",
        entry.commit.short_id,
        entry.commit.author,
        change_label(&entry.change)
    );
    put_left(
        ui,
        Rect::from_min_max(pos2(left, rect.min.y + 22.0), pos2(rect.max.x - HIST_PAD, rect.min.y + 41.0)),
        |ui| {
            ui.add(Label::new(RichText::new(detail).size(10.5).color(meta)).truncate().selectable(false).sense(Sense::hover()))
        },
    );
    fix_pitch(ui, rect);
    resp
}

/// `git blame` 的一行。列宽固定：行号 46、提交号 70、作者 84、日期 72，其余留给内容。
/// 作者与内容都走 `Label::truncate`，不靠 `painter.text` 硬排——不然长名字会压到下一列上。
pub(crate) fn blame_row(ui: &mut egui::Ui, width: f32, line: &BlameLine) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, HIST_BLAME_ROW_H), Sense::hover());
    if resp.hovered() {
        ui.painter_at(rect).rect_filled(rect, 0.0, BG_HOVER);
    }
    let p = ui.painter_at(rect);
    let cy = rect.center().y;
    let x = rect.min.x + HIST_PAD;
    p.text(
        pos2(x + 36.0, cy),
        Align2::RIGHT_CENTER,
        line.line_no.to_string(),
        FontId::monospace(10.0),
        TEXT_MUTED,
    );
    p.text(
        pos2(x + 44.0, cy),
        Align2::LEFT_CENTER,
        line.short_id.chars().take(8).collect::<String>(),
        FontId::monospace(10.0),
        TEXT_SECONDARY,
    );
    let author_x = x + 108.0;
    put_left(
        ui,
        Rect::from_min_max(pos2(author_x, rect.min.y), pos2(author_x + 84.0, rect.max.y)),
        |ui| {
            ui.add(
                Label::new(RichText::new(&line.author).size(10.5).color(TEXT_SECONDARY))
                    .truncate()
                    .selectable(false)
                    .sense(Sense::hover()),
            )
        },
    );
    p.text(
        pos2(author_x + 88.0, cy),
        Align2::LEFT_CENTER,
        line.date.chars().take(10).collect::<String>(),
        FontId::monospace(10.0),
        TEXT_SECONDARY,
    );
    let content_x = x + 200.0;
    put_left(
        ui,
        Rect::from_min_max(pos2(content_x, rect.min.y), pos2(rect.max.x - 4.0, rect.max.y)),
        |ui| {
            ui.add(
                Label::new(RichText::new(&line.content).monospace().size(10.5).color(TEXT_PRIMARY))
                    .truncate()
                    .selectable(false)
                    .sense(Sense::hover()),
            )
        },
    );
    fix_pitch(ui, rect);
}
