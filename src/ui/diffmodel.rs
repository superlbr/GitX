//! （拆分自 main.rs；见 PROCESS.md「拆分 main.rs」。）



// ------------------------------------------------------------------------ 差异排版
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Hunk,
    Add,
    Del,
    Ctx,
}

#[derive(Clone)]
pub(crate) struct DiffLine {
    pub(crate) kind: Kind,
    pub(crate) old: Option<u32>,
    pub(crate) new: Option<u32>,
    pub(crate) text: String,
    /// 这一块是整份补丁里的第几块（**从 1 数、跨文件不重置**，和 `text` 里那句「块 N」同一个数）。
    /// 按钮往 core 传的就是它——`git::hunk_patch` 用同一套数法，两边不用换算。
    /// `None` 只留给「还没进任何一块」那种行；文件头那几行现在整个不进列表，所以帧里画不出 `None`。
    pub(crate) hunk: Option<usize>,
}

/// 把补丁文本排成带行号的行。**在工作线程上跑**，帧里只管画视口那几十行。
///
/// `META` 里那些文件头一行都不留：SourceTree 的差异区第一行就是 hunk 那条带
/// （`sourcetree-4.2.19-repo-window.png`：文件名灰带 (236,236,236) 到 y=368pt 结束，
/// y=368..396 已经是 hunk 带 (240,244,241)，中间没有任何一行）。
/// **skip 不能改成"不筛"**：`+++ b/a.txt` 的首字节是 `+`、`--- ` 是 `-`，
/// 不筛就会被下面的 `match` 当成一行新增/删除画出来，还带上行号。
pub(crate) fn layout(patch: &str) -> Vec<DiffLine> {
    let mut out = Vec::with_capacity(patch.lines().count());
    let (mut old, mut new) = (0u32, 0u32);
    let mut hunk = 0usize;
    for line in patch.lines() {
        if META.iter().any(|m| line.starts_with(m)) {
            continue;
        }
        if line.starts_with("@@") {
            hunk += 1;
            let (a, ac, c, cc) = hunk_header(line);
            old = a;
            new = c;
            // 翻译只在这一处做：帧里画 hunk 行就是画 `line.text`。文案照参考图那句
            // 「块 1： 行 569-575」（`…-2026-10-20.png` y 409..438 那一行，x 749..852）：
            // **只报一个区间**，取旧侧那一段。新侧的行号本来就在右边那一列逐行写着；
            // 删/加不等长时（`ac != cc`）参考图没有样本可抄，就多带一句「（新 C-D）」，
            // 免得把另一侧的行号范围整个藏掉。
            let (o0, o1) = (a, a + ac.saturating_sub(1));
            let (n0, n1) = (c, c + cc.saturating_sub(1));
            let head = if (o1 - o0) == (n1 - n0) {
                format!("块 {hunk}： 行 {o0}-{o1}")
            } else {
                format!("块 {hunk}： 行 {o0}-{o1}（新 {n0}-{n1}）")
            };
            out.push(DiffLine { kind: Kind::Hunk, old: None, new: None, text: head, hunk: Some(hunk) });
            continue;
        }
        // 行号用「先取值再自增」：hunk 头说 -8,6 时第一条上下文行就是第 8 行，不是第 9 行。
        let (kind, o, n) = match line.as_bytes().first() {
            Some(b'+') => {
                let n = new;
                new += 1;
                (Kind::Add, None, Some(n))
            }
            Some(b'-') => {
                let o = old;
                old += 1;
                (Kind::Del, Some(o), None)
            }
            _ => {
                let (o, n) = (old, new);
                old += 1;
                new += 1;
                (Kind::Ctx, Some(o), Some(n))
            }
        };
        out.push(DiffLine { kind, old: o, new: n, text: line.to_owned(), hunk: Some(hunk) });
    }
    out
}

/// git 自己塞在补丁里的**文件头**，一行都不画（见 `layout`）。
/// 留着这一份是为了「筛掉」，不是「换颜色画出来」。
pub(crate) const META: [&str; 12] = [
    "diff --git ",
    "index ",
    "--- ",
    "+++ ",
    "new file ",
    "deleted file ",
    "old mode ",
    "new mode ",
    "similarity ",
    "rename from ",
    "rename to ",
    "Binary files ",
];

/// `@@ -8,6 +10,7 @@ 说明` → (8, 6, 10, 7)；省略 `,n` 时按 1 算。
pub(crate) fn hunk_header(line: &str) -> (u32, u32, u32, u32) {
    let mut it = line.trim_start_matches('@').trim().split_whitespace();
    let parse = |s: Option<&str>| -> (u32, u32) {
        let s = s.unwrap_or("0").trim_start_matches(|c| c == '-' || c == '+');
        match s.split_once(',') {
            Some((a, b)) => (a.parse().unwrap_or(0), b.parse().unwrap_or(1)),
            None => (s.parse().unwrap_or(0), 1),
        }
    };
    let (a, ac) = parse(it.next());
    let (c, cc) = parse(it.next());
    (a, ac, c, cc)
}
