//! 底部 toast：一句提示，8 秒后自己淡出，也可以点 × 立刻关掉。
//! SourceTree 的反馈走弹窗与横幅，没有这个部件——这是自家补充，替掉 tab 条右端
//! 那个常驻小格子（240pt 定宽截断、还和仓库 tab 抢一行）。做法同 deckcraft
//! `toast.rs`：`Area` 锚在状态栏上方，超时靠 `request_repaint_after` 唤醒下一帧结算。

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Margin, Sense, Stroke, Vec2, vec2};

use super::theme::{TEXT_PRIMARY, TEXT_MUTED, STATUS_H};

/// 一条 toast 停留的秒数。开发期提示（「还没接：搜索面板」）读两遍就够了，
/// 常驻反而让人对右下角那格字熟视无睹——原来那格的问题就在这。
const SHOW_SECS: f64 = 8.0;
const TOAST_W: f32 = 520.0;
/// 卡片最多长多高。toast 锚在底部、往上长，所以这个高度直接决定它盖掉上面多少内容：
/// 一条带「git 原文」的失败提示有 12 行，实测卡片上沿到 y 330，而停在冲突里的操作
/// 横幅（「继续回滚 / 终止回滚」两枚按钮在 319–334）正压在底下——点不动，等它 8 秒
/// 自己消失又不是办法。超出的部分改成卡内滚动：一个字不少，但不再霸占内容区。
/// 96 = 6 行 × 16pt 行高（12pt 字号），够读完中文那句 + `error:` 那一行。
const TOAST_MAX_H: f32 = 96.0;
/// 文字那一栏的换行宽度：卡片最大宽 520，减去左右各 12 的内边距、× 那 20、× 前面
/// 那点空隙（实测 7，取 8）。量内容高度时要按同一个宽度折行，否则量出来的行数
/// 和真的画出来的对不上。
const TOAST_TEXT_W: f32 = TOAST_W - 12.0 - 12.0 - 20.0 - 8.0;

/// 卡片描边比 1px 分隔线深半档：toast 悬在白底内容区上，`BORDER_SUBTLE` 会糊掉，
/// 所以取「控件描边」那一档 `BORDER_STRONG`（旧那枚 `#d0d3d9` 和它本是同一个灰）。
/// §5.5 还要一枚细阴影——那是描边可以退回 `BORDER_SUBTLE` 的前提，跟着 toast 那一带来。
const TOAST_EDGE: Color32 = super::theme::BORDER_STRONG;

/// 把 `note` 画成 toast 并负责它的生命周期。`at` 是它第一次出现的时刻
/// （egui 时间，秒）——放在 state 里而不是 toast 自己攒，是因为无头判据要能
/// 造出「已经过了 8 秒」的帧（`raw_input` 里直接给时间）。
pub(crate) fn show(ctx: &egui::Context, note: &mut Option<String>, at: &mut Option<f64>) {
    let Some(text) = note.clone() else {
        *at = None;
        return;
    };
    let now = ctx.input(|i| i.time);
    let start = at.get_or_insert(now);
    if now - *start > SHOW_SECS {
        *note = None;
        *at = None;
        return;
    }
    let mut close = false;
    // 按 `TOAST_TEXT_W` 折一遍，量这条提示真实要多高；再和上限取小，就是卡片给文字那一栏的高度。
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text,
        0.0,
        egui::text::TextFormat {
            font_id: FontId::proportional(12.0),
            color: TEXT_PRIMARY,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: TOAST_TEXT_W,
        max_rows: usize::MAX,
        break_anywhere: true,
        ..Default::default()
    };
    job.break_on_newline = true;
    let text_h = ctx
        .fonts_mut(|f| f.layout_job(job.clone()))
        .size()
        .y
        .min(TOAST_MAX_H);
    egui::Area::new(egui::Id::unique("gitx-toast"))
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -(STATUS_H + 8.0)))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1.0, TOAST_EDGE))
                .corner_radius(CornerRadius::same(6))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 2],
                    blur: 8,
                    spread: 0,
                    color: Color32::from_black_alpha(28),
                })
                .inner_margin(Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_max_width(TOAST_W);
                    ui.horizontal(|ui| {
                        egui::ScrollArea::vertical()
                            .max_height(text_h)
                            .min_scrolled_height(text_h)
                            .auto_shrink([true, true])
                            .show(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(&text).size(12.0).color(TEXT_PRIMARY)).wrap());
                            });
                        // 整张卡都能点关（原来那格就是点击消失），× 给鼠标一个显式目标。
                        // 用的是乘号「×」（U+00D7），排版符号，不是 emoji——UI 禁 emoji 是契约第②条。
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(4), super::theme::BG_HOVER);
                        }
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "×", FontId::proportional(14.0), TEXT_MUTED);
                        close |= resp.clicked();
                    });
                });
        });
    if close {
        *note = None;
        *at = None;
    } else {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 超时的数值只在这一个地方出现两次（判据与实现共用常量），这里钉死它：
    /// 改成更短的值会让开发期提示还没读完就消失。
    #[test]
    fn toast_stays_eight_seconds() {
        assert_eq!(SHOW_SECS, 8.0);
        assert_eq!(TOAST_W, 520.0);
    }
}
