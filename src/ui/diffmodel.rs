//! （拆分自 main.rs；见 docs/PROCESS.md「拆分 main.rs」。）

use gitx::tr;
use std::sync::Arc;

// ------------------------------------------------------------------------ 差异排版
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Kind {
    /// 多文件补丁里每个文件那一行带（路径 + 折叠开关）。单文件差异不画（见 `layout`）。
    File,
    Hunk,
    Add,
    Del,
    Ctx,
}

/// 一行的正文：**整份补丁共用一块文本，一行只记它那段字节区间**。
///
/// 为什么不是一行一个 `String`：30 002 行的差异 = 30 002 次小块分配，而**账写在次数上、
/// 不在字节数上**——`vmmap` 里那 30 000 次分配明写在「活动分配数」那一栏上，1.4 MB 的正文
/// 在进程账上多出的远不止 1.4 MB（A/B 数与「对不上的那一截」都记在 docs/PROCESS.md 内存那一批）。
/// 换成这一份之后正文不再逐行分配（块头/文件头各另占一小块，每块/每文件才一次），
/// 常驻从 3.49 MB 降到 2.06 MB，剩下那一坨还是**连续**的一块。
///
/// 块头那句「块 1： 行 569-575」和文件头那条路径不是补丁里的原文，它们自己占一小块
/// ——每一块/每个文件才一次，不参与上面那本账。
#[derive(Clone)]
pub(crate) struct DiffText {
    arena: Arc<str>,
    start: u32,
    end: u32,
}

impl DiffText {
    /// 补丁正文里 `[start, end)` 那一段（调用方给的是**去掉行尾换行**之后的区间）。
    fn span(arena: &Arc<str>, start: usize, end: usize) -> Self {
        Self { arena: arena.clone(), start: start as u32, end: end as u32 }
    }

    /// 现写的文案（块头、文件头）。
    fn owned(text: String) -> Self {
        let arena: Arc<str> = text.into();
        Self { start: 0, end: arena.len() as u32, arena }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.arena[self.start as usize..self.end as usize]
    }

    /// 判据用（`#[cfg(test)]`：产品代码一行都不读它，release 档里它就是死代码，
    /// 而零警告门禁不放死代码过去）：这两行的正文是不是真在同一块文本里。
    /// 哪天有人把 `layout` 改回逐行 `to_owned`，这条就量得出来。
    #[cfg(test)]
    pub(crate) fn shares_arena(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.arena, &other.arena)
    }

    /// 判据用（同上，只在测试里存在）：正文所在那块文本的**总长**。正文行 = 整份补丁那么长（大家同一块），
    /// 现写的头带 = 它自己那一小段。光看 `shares_arena` 分不出「一块补丁」和
    /// 「一块行号表」，长度把这一档钉死。
    #[cfg(test)]
    pub(crate) fn arena_len(&self) -> usize {
        self.arena.len()
    }
}

impl std::ops::Deref for DiffText {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for DiffText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Debug for DiffText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl PartialEq<str> for DiffText {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for DiffText {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// 夹具和判据手写的正文走这一条（真数据全从 `layout` 来，不需要）。
impl From<&str> for DiffText {
    fn from(text: &str) -> Self {
        Self::owned(text.to_owned())
    }
}

#[derive(Clone)]
pub(crate) struct DiffLine {
    pub(crate) kind: Kind,
    pub(crate) old: Option<u32>,
    pub(crate) new: Option<u32>,
    pub(crate) text: DiffText,
    /// 这一行属于补丁里的第几个文件（**从 1 数**）。折叠一个文件就是把它名下所有非 `File`
    /// 行从可见行里筛掉，所以每一行都得记着户口。
    pub(crate) file: usize,
    /// 这一块是整份补丁里的第几块（**从 1 数、跨文件不重置**，和 `text` 里那句「块 N」同一个数）。
    /// 按钮往 core 传的就是它——`git::hunk_patch` 用同一套数法，两边不用换算。
    /// `None` 留给文件头那一行（`Kind::File`）。
    pub(crate) hunk: Option<usize>,
}

/// 把补丁文本排成带行号的行。**在工作线程上跑**，帧里只管画视口那几十行。
///
/// `show_files` = 要不要在每个文件前落一行 `Kind::File` 头带。整份补丁（全部未暂存改动、
/// 一条提交的全部改动）要——不然几个文件的首尾是连着的，用户看不出哪儿换了文件；
/// 单文件差异不要——顶上那条标题已经写着路径，再来一行就是重复。
///
/// `META` 里其余文件头一行都不留，**skip 不能改成"不筛"**：`+++ b/a.txt` 的首字节是 `+`、
/// `--- ` 是 `-`，不筛就会被下面的 `match` 当成一行新增/删除画出来，还带上行号。
///
/// 收 `impl Into<Arc<str>>` 而不是 `&str`：从 `git diff` 回来的那一整份补丁**就是**这些行的
/// 正文，移进 `Arc` 之后一行都不必再复制（见 `DiffText`）。判据手写的 `&str` 补丁照样能传。
pub(crate) fn layout(patch: impl Into<Arc<str>>, show_files: bool) -> Vec<DiffLine> {
    let patch: Arc<str> = patch.into();
    let mut out = Vec::with_capacity(patch.lines().count());
    let (mut old, mut new) = (0u32, 0u32);
    let mut hunk = 0usize;
    let mut file = 0usize;
    // 游标按**字节**走：`line` 由 `patch.lines()` 给（它只砍掉 `\n` 或 `\r\n`，
    // 见 `a_crlf_patch_lays_out_the_same_lines_as_an_lf_one`），所以正文区间就是
    // `[起点, 起点 + len)`，再把这一行的终止符跨过去。
    let mut pos = 0usize;
    for line in patch.lines() {
        let start = pos;
        let end = start + line.len();
        debug_assert_eq!(patch.get(start..end), Some(line), "游标和 `lines()` 对不上了");
        pos = end
            + if patch[end..].starts_with("\r\n") {
                2
            } else if patch[end..].starts_with('\n') {
                1
            } else {
                0
            };
        if let Some(rest) = line.strip_prefix("diff --git ") {
            file += 1;
            if show_files {
                out.push(DiffLine {
                    kind: Kind::File,
                    old: None,
                    new: None,
                    text: DiffText::owned(diff_path(rest)),
                    file,
                    hunk: None,
                });
            }
            continue;
        }
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
                tr!(
                    format!("块 {hunk}： 行 {o0}-{o1}"),
                    format!("Hunk {hunk}: lines {o0}-{o1}")
                )
            } else {
                tr!(
                    format!("块 {hunk}： 行 {o0}-{o1}（新 {n0}-{n1}）"),
                    format!("Hunk {hunk}: lines {o0}-{o1} (new {n0}-{n1})")
                )
            };
            out.push(DiffLine {
                kind: Kind::Hunk,
                old: None,
                new: None,
                text: DiffText::owned(head),
                file,
                hunk: Some(hunk),
            });
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
        out.push(DiffLine { kind, old: o, new: n, text: DiffText::span(&patch, start, end), file, hunk: Some(hunk) });
    }
    out
}

/// `diff --git a/x b/x` 的 `rest` → 相对路径 `x`（改名时取新名）。
///
/// 路径含空格/特殊字符时 git 会把两边都包进引号，这时按 `" "` 从右边切一段再脱引号；
/// 否则取最后一个 ` b/` 之后那段——`a/b/foo b/b/foo` 这种路径里本来就带 `b/`，
/// 从左边切会把 `b/` 当成前缀切掉一次，所以必须认最后一个。
fn diff_path(rest: &str) -> String {
    let token = if rest.starts_with('"') {
        rest.rsplit_once("\" \"")
            .map(|(_, b)| b.trim_matches('"'))
            .unwrap_or(rest)
    } else if let Some(i) = rest.rfind(" b/") {
        &rest[i + 1..]
    } else {
        rest.split_whitespace().last().unwrap_or(rest)
    };
    let token = unquote(token);
    token.strip_prefix("b/").unwrap_or(&token).to_owned()
}

/// 去掉 git 给路径加的引号与转义（`\"`、`\\`；八进制转义少见，这里按两字符原样留着，
/// 宁可显示一串 `\346` 也不把路径切坏）。
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        let mut out = String::with_capacity(s.len());
        let mut esc = false;
        for c in s[1..s.len() - 1].chars() {
            if esc {
                out.push(c);
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else {
                out.push(c);
            }
        }
        out
    } else {
        s.to_owned()
    }
}

/// 一行在等宽字体里大约占多少个字格。差异区的**横向**可滚宽度靠它算：
/// ASCII 算 1 格、制表符算 4 格（egui 不认制表位，按四个空格保守估）、其余算 2 格
/// （CJK 与 emoji 在等宽字体里约等于两个 ASCII 宽）。估宽不估窄——窄了长行的尾巴
/// 就永远滚不到，那正是用户看到的「只有三个点省略」。
pub(crate) fn columns(text: &str) -> usize {
    text.chars()
        .map(|c| if c == '\t' { 4 } else if c.is_ascii() { 1 } else { 2 })
        .sum()
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
