use std::collections::HashMap;
use std::process::Command;

use crate::error::{GitxError, Result};
use crate::models::*;

/// Without this, git escapes every non-ASCII path into `"\344\270\255..."`,
/// which is what makes SourceTree unusable for Chinese file names.
const CONFIG_ARGS: [&str; 2] = ["-c", "core.quotepath=false"];

/// Run a git command in the given repository and return stdout.
fn run_git(repo: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(CONFIG_ARGS)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(GitxError::Git(explain_git_error(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    Ok(decode(&output.stdout))
}

/// Like `run_git`, but with extra environment variables (e.g. GIT_SEQUENCE_EDITOR).
fn run_git_env(repo: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(CONFIG_ARGS)
        .args(args)
        .envs(envs.iter().map(|(k, v)| (*k, *v)))
        .output()?;
    if !output.status.success() {
        return Err(GitxError::Git(explain_git_error(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    Ok(decode(&output.stdout))
}

/// git prints paths and messages in the repository's own bytes, which for a
/// legacy Chinese Windows repo are GBK — invalid UTF-8. Failing on that would
/// make the whole repo unreadable, so bad bytes become U+FFFD instead.
fn decode(out: &[u8]) -> String {
    String::from_utf8_lossy(out).into_owned()
}

/// git's own wording is English and unhelpful to most domestic users, but the
/// original text is the only thing searchable when something is truly odd, so
/// every message keeps it after the explanation.
fn explain_git_error(stderr: &str) -> String {
    let trimmed = stderr.trim();
    let lower = trimmed.to_lowercase();
    let hint = [
        ("not a git repository", "这里不是一个 git 仓库（没有 .git），请先在侧栏添加正确的目录。"),
        ("has no upstream branch", "当前分支没有关联远程分支，首次推送请用「推送」里的设置上游（git push -u origin 分支名）。"),
        ("no such branch", "找不到这个分支，名字可能写错了或已被删除。"),
        ("pathspec", "没有匹配到这些路径，可能已经被移动、删除或拼写不一致。"),
        ("non-fast-forward", "远程有你本地没有的提交，请先 Fetch 并 Pull / 变基之后再推送，强推会丢掉对方的提交。"),
        ("rejected", "远程拒绝了这次推送，通常是远端有新提交或权限不足。"),
        ("permission denied (publickey)", "SSH 公钥没被对方接受：请确认密钥已加到平台的 SSH Keys，且用的是 ssh_config 里那把（可在「设置 → SSH」试连）。"),
        ("could not read username", "远程要求账号密码。HTTP 令牌方式请在 remote 地址里带上用户名，或改用 SSH。"),
        ("connection refused", "连不上远端服务器：端口不通、代理没走或地址写错了。"),
        ("timed out", "连接超时：国内直连 GitHub 常见，建议在「克隆/远程」里改用加速节点或配好代理。"),
        ("could not resolve host", "域名解析失败：网络不通或需要代理。"),
        ("authentication failed", "认证失败：令牌过期、拼错或权限不足，请到平台的设置里重新生成访问令牌。"),
        ("repository not found", "远程仓库不存在：地址写错、仓库已改名或被删除，也可能是私有仓库而你的令牌/公钥没有读取权限。"),
        ("requested url returned error", "远程拒绝了这次访问：私有仓库需要有效的访问令牌，或仓库地址（含组织名）写错了。"),
        ("destination path", "目标目录已存在且不是空目录：换一个目录，或把已有仓库直接添加到侧栏。"),
        ("rpc failed", "传输中断：国内直连 GitHub 常见，请改用加速节点或配好代理后重试。"),
        ("recv failure", "网络在传输途中断开：通常是代理或加速节点不稳定，重试或换一个节点。"),
        ("early eof", "传输提前结束：网络不稳定，重试一次通常就能通过。"),
        ("server certificate verification failed", "证书校验失败：常见于公司代理/网关替换证书，需要配置根证书或改用 SSH。"),
        ("diverged", "本地和远程都有新提交（分叉了）。请选择变基或合并后再同步。"),
        ("nothing to commit", "没有可提交的内容：先勾选要提交的改动文件。"),
        ("working tree not clean", "工作区还有未提交的改动，git 不肯继续。请先提交或贮藏。"),
        ("would be overwritten by merge", "本地改动会被覆盖：请先提交或贮藏这些改动再继续。"),
        ("merge conflict", "存在合并冲突，请先在「冲突」列表里逐个解决。"),
        ("author identity unknown", "还没配置 git 用户身份，请在「设置」或运行 git config user.name / user.email。"),
        ("empty message", "提交信息不能为空。"),
        ("unmerged files", "还有未解决的合并冲突文件，先解决再提交。"),
        ("lock file", "上一个 git 操作可能没干净退出（存在锁文件）。确认没有其它 git 进程在跑后删除该锁文件。"),
        ("dubious ownership", "目录属主和当前用户不一致，git 出于安全拒绝操作。请用对应账号运行，或把该目录加入 git 的安全目录。"),
        ("file exists", "目标已经存在，换个名字再试。"),
    ]
    .iter()
    .find(|(needle, _)| lower.contains(needle))
    .map(|(_, msg)| *msg);

    match hint {
        Some(msg) => format!("{msg}\n\n—— git 原文 ——\n{trimmed}"),
        None => trimmed.to_string(),
    }
}

/// Clone via `clone_url` into `dest` (relative `dest` resolved against `$HOME`),
/// then set the `origin` remote to `remote_url`. This keeps acceleration nodes
/// used only for the initial fetch — the persisted remote always points at the
/// real GitHub URL, so later `pull`/`push` never go through the node.
pub fn clone_repo(clone_url: &str, remote_url: &str, dest: &str) -> Result<String> {
    let dest = expand_dest(dest);
    let output = Command::new("git")
        .arg("clone")
        .arg(clone_url)
        .arg(&dest)
        .output()?;
    if !output.status.success() {
        return Err(GitxError::Git(explain_git_error(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    if clone_url != remote_url {
        run_git(&dest, &["remote", "set-url", "origin", remote_url])?;
    }
    Ok(dest)
}

/// 在 `path` 新建一个空仓库（`git init`）。目录还不存在时 git 会把中间层一起建出来，
/// 相对路径和克隆一样按主目录展开——「添加仓库」那张卡的「新建」那一档走的就是这条。
pub fn init_repo(path: &str) -> Result<String> {
    let dest = expand_dest(path);
    let output = Command::new("git")
        .arg("init")
        .arg(&dest)
        .output()?;
    if !output.status.success() {
        return Err(GitxError::Git(explain_git_error(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    Ok(dest)
}

/// 相对路径按主目录算（用户在那一格只打一个名字时，落点要能预期）。
fn expand_dest(dest: &str) -> String {
    if std::path::Path::new(dest).is_absolute() {
        dest.to_string()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{}/{}", home.trim_end_matches('/'), dest)
    }
}

/// True if the path is inside a git work tree.
pub fn is_repo(path: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The leading dotted-numeric part of a version token: `2.50.1.windows.1`,
/// `2.47.0.rc0` and `3.0.0-pre` all report their `major.minor.patch`. A
/// pre-release or platform suffix is not something to show or compare against,
/// and fewer than two segments is not a version at all.
fn numeric_prefix(token: &str) -> Option<String> {
    let core = token.split(['-', '+']).next().unwrap_or(token);
    let parts: Vec<&str> = core
        .split('.')
        .take_while(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if parts.len() < 2 {
        return None;
    }
    Some(parts.join("."))
}

/// Read the version out of `git --version`. Windows prints
/// `git version 2.50.1.windows.1` and macOS prints
/// `git version 2.39.3 (Apple Git-146)`; both keep their leading numeric dotted
/// prefix and drop the rest. Anything without one means git is not usable here,
/// which on a fresh Windows machine is the normal case rather than a crash.
pub fn parse_git_version(out: &str) -> Option<String> {
    let tail = out.trim().strip_prefix("git version")?.trim();
    numeric_prefix(tail.split_whitespace().next()?)
}

/// `git lfs version` prints `git-lfs/3.5.1 (GitHub; darwin arm64; go 1.21.3)`.
pub fn parse_lfs_version(out: &str) -> Option<String> {
    let tail = out.trim().strip_prefix("git-lfs/")?;
    numeric_prefix(tail.split([' ', '(']).next()?)
}

/// `git lfs ls-files` reports `<oid> <+-|> <path>`; the flag tells whether the
/// working copy matches the stored object, and the path may contain spaces, so
/// split on the two fixed tokens rather than whitespace-splitting the line.
pub fn parse_lfs_paths(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|line| {
            let oid_end = line.find(' ')?;
            let after_oid = &line[oid_end + 1..];
            let flag_end = after_oid.find(' ')?;
            if &after_oid[..flag_end] != "-" && &after_oid[..flag_end] != "+" {
                return None;
            }
            let path = after_oid[flag_end + 1..].trim();
            (!path.is_empty()).then(|| path.to_string())
        })
        .collect()
}

/// The installed git's version, or `None` when there is no git binary at all.
pub fn git_version() -> Option<String> {
    let out = Command::new("git").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_git_version(&String::from_utf8_lossy(&out.stdout))
}

/// Which git-lfs the machine has, or `None` when the subcommand is missing.
pub fn lfs_version() -> Option<String> {
    let out = Command::new("git")
        .arg("lfs")
        .arg("version")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_lfs_version(&String::from_utf8_lossy(&out.stdout))
}

/// What LFS looks like for one repository: installed or not, and which tracked
/// files it manages. A repo that stores LFS pointers *without* git-lfs
/// installed is the confusing case this exists for — the files open as text
/// blobs instead of content, and no error says why.
pub fn lfs_info(repo: &str) -> LfsInfo {
    let version = lfs_version();
    let files = match &version {
        Some(_) => run_git(repo, &["lfs", "ls-files"])
            .map(|out| parse_lfs_paths(&out))
            .unwrap_or_default(),
        None => Vec::new(),
    };
    LfsInfo {
        installed: version.is_some(),
        version: version.unwrap_or_default(),
        files,
    }
}

/// Walk up from `path` (a dropped file or folder) to the nearest directory that
/// contains a `.git` entry, so dragging a file from inside a repo lands on the
/// repo itself rather than nowhere. Existence of `.git` (a dir, or a file for
/// submodule/worktree checkouts) is the test — `add_repo` re-checks with
/// `is_repo` afterwards, so a stray `.git` costs one failed add at worst.
pub fn find_repo_root(path: &str) -> Option<String> {
    let start = std::path::Path::new(path);
    let mut cur = start.to_path_buf();
    if cur.is_file() {
        cur = cur.parent()?.to_path_buf();
    }
    cur.ancestors()
        .find(|anc| anc.join(".git").exists())
        .map(|anc| anc.to_string_lossy().into_owned())
}

pub fn head(repo: &str) -> Result<String> {
    Ok(run_git(repo, &["rev-parse", "HEAD"])?.trim().to_string())
}

/// Repositories found below one folder, breadth-first, at most `max_depth` levels
/// down. A directory holding `.git` is reported and *not* descended into, so
/// submodules and vendored checkouts never flood the list with repos nobody
/// browses directly. Hidden directories and symlinks are skipped (both are how a
/// walk turns into a loop), unreadable directories are ignored rather than
/// failing the whole scan, and the result is capped by `limit`.
pub fn scan_repos(root: &str, max_depth: usize, limit: usize) -> Result<Vec<String>> {
    let start = std::path::Path::new(root);
    if !start.is_dir() {
        return Err(GitxError::Other(format!("文件夹不存在: {}", root)));
    }
    let mut found: Vec<String> = Vec::new();
    let mut queue = std::collections::VecDeque::from([(start.to_path_buf(), 0usize)]);
    while let Some((dir, depth)) = queue.pop_front() {
        if dir.join(".git").exists() {
            if found.len() < limit {
                found.push(dir.to_string_lossy().into_owned());
            }
            continue;
        }
        if depth >= max_depth || found.len() >= limit {
            continue;
        }
        let Ok(children) = std::fs::read_dir(&dir) else {
            continue;
        };
        for child in children.flatten() {
            let name = child.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let Ok(ft) = child.file_type() else { continue };
            // Symlinks are how a walk loops back onto itself, and a git
            // dependency installed by npm keeps its own `.git` in
            // node_modules — neither is a repository the user means to open.
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            queue.push_back((child.path(), depth + 1));
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

pub fn current_branch(repo: &str) -> Result<String> {
    Ok(run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string())
}

/// Full message (subject + body) of one commit, for editing. Trailing blank
/// lines that `%B` emits are dropped so the editor opens with a clean value.
pub fn commit_message(repo: &str, oid: &str) -> Result<String> {
    Ok(run_git(repo, &["log", "-1", "--format=%B", oid])?
        .trim_end_matches(|c| c == '\n' || c == '\r')
        .to_string())
}

/// Remote branches that already contain `oid`. Non-empty means the commit is
/// published, so amending it rewrites history other people have fetched.
pub fn pushed_refs(repo: &str, oid: &str) -> Result<Vec<String>> {
    let out = run_git(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "--contains",
            oid,
            "refs/remotes",
        ],
    )?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.ends_with("/HEAD"))
        .map(String::from)
        .collect())
}

/// Commits reachable from HEAD that no remote-tracking ref contains yet — the
/// ones a push would send. One `rev-list` for the whole list is far cheaper
/// than asking per commit, which is what a badge across 200 rows needs.
///
/// A repo with no remote has nothing to be unpushed *relative to*, so it
/// reports none rather than badging its entire history.
pub fn unpushed_commits(repo: &str) -> Vec<String> {
    let has_remote = run_git(repo, &["for-each-ref", "--count=1", "refs/remotes"])
        .map(|out| !out.trim().is_empty())
        .unwrap_or(false);
    if !has_remote {
        return Vec::new();
    }
    run_git(repo, &["rev-list", "HEAD", "--not", "--remotes"])
        .map(|out| {
            out.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

// ----------------------------------------------------------------------------
// Status
// ----------------------------------------------------------------------------

pub fn status(repo: &str) -> Result<RepoStatus> {
    let out = run_git(repo, &["status", "--porcelain=v2", "--branch"])?;
    let mut st = parse_status(&out);
    // 同一把 `.git` 读完两处：这一次 fork 之外的信息全在文件系统里。
    let gd = git_dir(repo)?;
    st.stash_count = stash_count(&gd);
    st.repo_state = state_at(&gd);
    Ok(st)
}

/// Locate the git directory by reading the working tree, with no subprocess.
///
/// `<repo>/.git` is a directory in an ordinary checkout and a one-line
/// `gitdir: <path>` pointer in a linked worktree — and the pointer names the
/// *per-worktree* git dir, which is exactly where that worktree's own
/// `MERGE_HEAD` lives. Bare repositories have no `.git` at all, so the caller
/// falls back to asking git rather than reading the wrong place.
fn git_dir(repo: &str) -> Result<std::path::PathBuf> {
    let dot = std::path::Path::new(repo).join(".git");
    if dot.is_dir() {
        return Ok(dot);
    }
    if let Ok(raw) = std::fs::read_to_string(&dot) {
        if let Some(target) = raw.trim().strip_prefix("gitdir:") {
            let target = target.trim();
            if !target.is_empty() {
                // git 通常写绝对路径，但相对写法确实存在，且它是**相对工作树**的。
                let p = std::path::Path::new(target);
                return Ok(if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    std::path::Path::new(repo).join(p)
                });
            }
        }
    }
    Ok(std::path::PathBuf::from(
        run_git(repo, &["rev-parse", "--absolute-git-dir"])?
            .trim()
            .to_string(),
    ))
}

/// Detect an in-progress merge / cherry-pick / rebase via git-dir sentinel files.
fn repo_state(repo: &str) -> Result<String> {
    Ok(state_at(&git_dir(repo)?))
}

/// `state_at` 的入参必须是**绝对** git-dir 或相对进程 cwd 能走通的路径：
/// 拿 `--git-dir` 那种相对当前进程 cwd 的一截路径去 `exists()`，查的是壳进程所在的
/// 目录，不是被打开的那个仓库。`git_dir` 给的两种形态都落在仓库自己的路径下。
/// `state_at` 只是 `op_at` 的一层投影：sentinel 的读法全仓库只有一处。
fn state_at(gd: &std::path::Path) -> String {
    op_at(gd)
        .map(|op| op.kind)
        .unwrap_or_else(|| "none".into())
}

/// 贮藏条数，读 reflog 的行数，不再 fork `git stash list`。
///
/// git 在 `refs/stash` 被删掉时（`drop` 掉最后一条、`clear`）会连着删掉这个文件，
/// 所以「行数」和 `stash list` 的行数在实测的每个状态下都相等 —— 判据
/// `stash_count_matches_stash_list` 把 drop/pop/apply/gc/packed-refs 全过了一遍。
/// 需要贮藏的说明和 oid 时（侧栏）仍然走 `stash_list`。
fn stash_count(gd: &std::path::Path) -> u32 {
    match std::fs::read_to_string(gd.join("logs/refs/stash")) {
        Ok(text) => text.lines().filter(|l| !l.is_empty()).count() as u32,
        Err(_) => 0,
    }
}

/// 本机在这家仓库里的提交身份：邮件地址集，小写、去重、保持出现的顺序。
/// 拆成纯函数是因为 `--get-regexp` 的真实输出（全局 + 本地混在一起、一个键多行）
/// 用嘴说不算数，得拿字面量钉住。
fn parse_identities(out: &str) -> Vec<String> {
    let mut emails = Vec::new();
    for line in out.lines() {
        // `--get-regexp` 一行是「<key> <value>」，值里可以带空格，所以只在第一个空白处
        // 切一刀；`user.email` 之外的那些键（`user.name`、带 @ 的 `user.signingkey`）
        // 一条都不能进来 —— 名字匹配会把别人的提交算到我头上。
        let Some((key, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if key != "user.email" {
            continue;
        }
        let e = value.trim().to_lowercase();
        if !e.is_empty() && !emails.contains(&e) {
            emails.push(e);
        }
    }
    emails
}

/// 问 git，不是读文件：config 的取值规则（include、条件段、覆盖顺序）我们复刻不起，
/// 复刻一份错的比多 fork 一次严重。一次 `--get-regexp '^user\.'` 就够，调用方负责缓存
/// （见 `db::identity_cache`），所以首屏稳态不为它 fork。
/// 拿不到（没配、或仓库读不了）返回空集而不是报错：首屏宁可少两行活动流。
pub fn identities(repo: &str) -> Vec<String> {
    // 没有一条 user.* 时 git 以 1 退出，`run_git` 把它当失败 —— 这里读作「没配身份」。
    run_git(repo, &["config", "--get-regexp", "^user\\."])
        .map(|out| parse_identities(&out))
        .unwrap_or_default()
}

/// git-dir 里那份「进行中的操作」，含变基的第几步。`state_at` 是它的一层投影，
/// 两处共用同一次读取 —— 两套机制各查一遍 sentinel 时，改坏一套另一套还绿。
fn op_at(gd: &std::path::Path) -> Option<Op> {
    // 顺序沿用改造前的 `state_at`：merge → cherry-pick → rebase。实测变基停在冲突上时
    // git-dir 根目录只有 `REBASE_HEAD`，没有 `CHERRY_PICK_HEAD` 也没有 `MERGE_HEAD`，
    // 所以这个顺序不会把「变基到第 3 步」报成「正在遴选」。
    if gd.join("MERGE_HEAD").exists() {
        Some(Op { kind: "merge".into(), step: None, total: None })
    } else if gd.join("CHERRY_PICK_HEAD").exists() {
        Some(Op { kind: "cherry-pick".into(), step: None, total: None })
    } else {
        // 两种后端各留一份目录：`-m/--i` 走 rebase-merge，`git am` 走 rebase-apply。
        for dir in ["rebase-merge", "rebase-apply"] {
            let d = gd.join(dir);
            if d.exists() {
                return Some(Op {
                    kind: "rebase".into(),
                    step: read_count(&d.join("msgnum")),
                    total: read_count(&d.join("end")),
                });
            }
        }
        None
    }
}

/// 文件里的一个计数。读不到或不是数字就回 `None` —— 界面上那是「进行中」，
/// 而不是「0 / 0」那根永远不动的进度条。
fn read_count(path: &std::path::Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// `log --since` 那条的取数上限。窗口的边界本来归 `--since` 管，这个数字只是保险丝：
/// 时间窗内提交多于 500 条的仓库，我的提交可能被挤出活动流（一条已登记的边界）。
const ACTIVITY_LOG_SCAN: u32 = 500;

/// 首屏「最近」面板要的那一份摘要。
///
/// `emails` 由调用方传进来（本机身份集，缓存见 db），所以这个函数**固定两次 fork**：
/// `status --porcelain=v2 --branch` 与一次带 `--since` 的 `log`。`since` 是绝对 RFC3339
/// 串而不是「7 天」，判据才能拿固定日期跑。
/// 时间窗交给 git 的 `--since` 而不是本地筛：本地筛要先有全量，一个万人仓库会被 `--max-count`
/// 截断在窗口外面。
pub fn recent_activity(repo: &str, emails: &[String], since: &str, max: u32) -> Result<Activity> {
    let st = status(repo)?;
    let gd = git_dir(repo)?;
    let out = run_git(
        repo,
        &[
            "log",
            &format!("--since={}", since),
            &format!("--max-count={}", ACTIVITY_LOG_SCAN),
            &format!("--pretty=format:{}", LOG_FMT),
        ],
    )?;
    // 先按身份筛、再截断：反过来（把 `--max-count` 设成 `max`）会在筛之前就把我的提交
    // 剪掉，忙仓库里活动流会整批空掉。
    let set: Vec<String> = emails.iter().map(|e| e.to_lowercase()).collect();
    let mine = parse_log(&out)
        .into_iter()
        .filter(|c| set.contains(&c.author_email.to_lowercase()))
        .take(max as usize)
        .collect();
    Ok(Activity {
        repo: repo.to_string(),
        branch: st.branch,
        ahead: st.ahead,
        behind: st.behind,
        dirty: st.files.len() as u32,
        stash: st.stash_count,
        op: op_at(&gd),
        mine,
    })
}

/// 并发聚合的机器：`workers` 个 worker 从原子下标领任务，每读完一个仓库立刻 `emit` 一份。
///
/// `emit` 会被多个线程同时调到，所以要 `Sync`；它只负责「到了就交」，什么时候到由
/// 仓库自己的快慢决定 —— 这正是首屏要的：面板一块一块长出来，不是等最慢的那个仓库一起出现。
/// `emit` 回 `false` 表示「没人要了」，那个 worker 把手上的活儿干完就走，不再领新任务。
/// 队列用原子下标发号，不用 `chunks` 切分：切成分片后一片卡在挂起的仓库上，
/// 后面整片都不会开始，而并发深度会掉到「片数」。
fn activity_fan_out(
    items: &[RepoQuery],
    since: &str,
    max: u32,
    workers: usize,
    emit: &(dyn Fn(usize, RepoOutcome) -> bool + Sync),
) {
    if items.is_empty() {
        return;
    }
    let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = items.len();
    let n = workers.clamp(1, count);
    std::thread::scope(|scope| {
        for _ in 0..n {
            let (next, items, emit) = (next.clone(), items, emit);
            scope.spawn(move || loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= count {
                    break;
                }
                let q = &items[i];
                let result = recent_activity(&q.repo, &q.emails, since, max);
                if !emit(i, RepoOutcome { repo: q.repo.clone(), result }) {
                    break;
                }
            });
        }
    });
}

/// 首屏一次问完所有仓库并**当场收齐**：适合只要最终结果、不关心中间过程的调用方。
/// 想在读的过程中就往界面上填，用 `recent_activity_stream` —— 这个函数会等最慢的那个仓库。
///
/// `workers` 是同时在飞的最大仓库数（不是线程池常驻大小）；0 与超过仓库数都收缩到
/// 实际需要的数量，空输入直接回空表，不 spawn 任何线程。
/// 返回顺序和 `items` 一致 —— 面板按「最近打开」排，队列自己不能反过来。
/// 单仓库失败只落在它那一行的 `Err` 上（错误隔离）。这里**不做超时**，而且要说清代价：
/// 一个挂起的 `git`（凭据提示、在休眠的网络盘上、被别的进程锁住 index）会占住它那个 worker，
/// `thread::scope` 要等所有 worker 收完，所以整个调用会跟着挂住 —— 慢仓库今天会拖住首屏，
/// 不只是它自己那一行。修它需要先给 `run_git` 做可取消执行（超时不是加个 `recv_timeout` 就完事，
/// 那样子进程还活着，是谎报「超时了」）。这是已登记的一步，不在这里偷做。
pub fn recent_activity_all(
    items: &[RepoQuery],
    since: &str,
    max: u32,
    workers: usize,
) -> Vec<RepoOutcome> {
    // 先按序号放进各自的格子，再摊平：worker 交出来的顺序是完成顺序，不是输入顺序。
    let slots = std::sync::Mutex::new(
        (0..items.len())
            .map(|_| Option::<RepoOutcome>::None)
            .collect::<Vec<_>>(),
    );
    activity_fan_out(items, since, max, workers, &(|i, outcome| {
        slots.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(outcome);
        true
    }));
    slots
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .map(|s| {
            s.unwrap_or_else(|| RepoOutcome {
                repo: String::new(),
                result: Err(GitxError::Other("该仓库的 worker 没有产出结果".into())),
            })
        })
        .collect()
}

/// 同上，但每读完一个仓库就往 `tx` 塞一份，不等最慢的那个。
/// `RepoOutcome` 里只有仓库路径，序号不给 —— 界面按路径找那一行该填哪儿，
/// 排序仍然是它自己的「最近打开」顺序，跟谁先回来无关。
pub fn recent_activity_stream(
    items: &[RepoQuery],
    since: &str,
    max: u32,
    workers: usize,
    tx: &std::sync::mpsc::Sender<RepoOutcome>,
) {
    // 接收端已经被丢掉 = 界面那侧不再要结果（窗口关了、或新一批刷新把旧的盖掉了）。
    // 这时候继续跑完剩下的仓库只是白添进程，所以谁先撞上发送失败，谁就退休；
    // 其余 worker 在各自的下一次发送上同样失败，一起收摊。
    activity_fan_out(items, since, max, workers, &(|_, outcome| {
        tx.send(outcome).is_ok()
    }));
}

fn map_status(c: &str) -> ChangeStatus {
    match c {
        "A" => ChangeStatus::Added,
        "M" => ChangeStatus::Modified,
        "D" => ChangeStatus::Deleted,
        "R" => ChangeStatus::Renamed,
        "C" => ChangeStatus::Copied,
        "U" => ChangeStatus::Unmerged,
        "?" => ChangeStatus::Untracked,
        _ => ChangeStatus::Unknown,
    }
}

fn map_status_char(c: char) -> ChangeStatus {
    match c {
        'A' => ChangeStatus::Added,
        'M' => ChangeStatus::Modified,
        'D' => ChangeStatus::Deleted,
        'R' => ChangeStatus::Renamed,
        'C' => ChangeStatus::Copied,
        'U' => ChangeStatus::Unmerged,
        'T' => ChangeStatus::Modified,
        _ => ChangeStatus::Unknown,
    }
}

fn parse_status(out: &str) -> RepoStatus {
    let mut branch = String::new();
    let mut upstream: Option<String> = None;
    let mut ahead = 0u32;
    let mut behind = 0u32;
    let mut files = Vec::new();

    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            branch = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            let up = rest.trim();
            if !up.is_empty() && up != "(gone)" {
                upstream = Some(up.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut it = rest.split_whitespace();
            if let Some(a) = it.next() {
                ahead = a.trim_start_matches('+').parse().unwrap_or(0);
            }
            if let Some(b) = it.next() {
                behind = b.trim_start_matches('-').parse().unwrap_or(0);
            }
        } else if let Some(rest) = line.strip_prefix("1 ") {
            let mut it = rest.splitn(9, ' ');
            let xy = it.next().unwrap_or("");
            for _ in 0..6 {
                it.next();
            }
            let path = it.next().unwrap_or("").to_string();
            let (x, y) = xy.split_at(1);
            files.push(FileStatus {
                path,
                old_path: None,
                index_status: map_status(x),
                worktree_status: map_status(y),
                staged: x != "." && x != "?",
            });
        } else if let Some(rest) = line.strip_prefix("2 ") {
            let mut it = rest.splitn(9, ' ');
            let xy = it.next().unwrap_or("");
            for _ in 0..7 {
                it.next();
            }
            let raw = it.next().unwrap_or("");
            let mut p = raw.splitn(2, '\t');
            let path = p.next().unwrap_or("").to_string();
            let old_path = p.next().map(|s| s.to_string());
            let (x, y) = xy.split_at(1);
            files.push(FileStatus {
                path,
                old_path,
                index_status: map_status(x),
                worktree_status: map_status(y),
                staged: x != "." && x != "?",
            });
        } else if let Some(rest) = line.strip_prefix("u ") {
            let mut it = rest.splitn(10, ' ');
            let xy = it.next().unwrap_or("");
            for _ in 0..8 {
                it.next();
            }
            let path = it.next().unwrap_or("").to_string();
            let (x, y) = xy.split_at(1);
            files.push(FileStatus {
                path,
                old_path: None,
                index_status: map_status(x),
                worktree_status: map_status(y),
                staged: true,
            });
        } else if let Some(rest) = line.strip_prefix("? ") {
            files.push(FileStatus {
                path: rest.to_string(),
                old_path: None,
                index_status: ChangeStatus::Unknown,
                worktree_status: ChangeStatus::Untracked,
                staged: false,
            });
        }
    }

    RepoStatus {
        branch,
        upstream,
        ahead,
        behind,
        files,
        stash_count: 0,
        repo_state: "none".into(),
    }
}

// ----------------------------------------------------------------------------
// Commits
// ----------------------------------------------------------------------------

/// Which commits the log walks — SourceTree's 「所有分支 ⇄ 当前分支」 switch, plus the
/// third state its popup gets here (one named branch).
///
/// An enum rather than `rev: Option<&str>` + `all: bool`: those two together would ask
/// git for `--all <rev>`, a scope no menu item can produce and no test could reach.
/// Building it out of the type is cheaper than defending it with a guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every ref in the repo: `git log --all`.
    All,
    /// The current branch: ask git for nothing and let it fall back to HEAD.
    Current,
    /// One revision's ancestors: the name goes to git as a positional argument.
    Rev(String),
}

pub fn commits(
    repo: &str,
    max: u32,
    author: Option<&str>,
    path: Option<&str>,
) -> Result<Vec<Commit>> {
    log_commits(repo, max, author, path, false, 0, &Scope::Current)
}

/// The same list in topological order — SourceTree's 「层级顺序」 switch: a branch's
/// commits stay in one group instead of interleaving with whatever else was committed
/// at that time, which is what makes the history graph read as branches rather than noise.
pub fn commits_topo(
    repo: &str,
    max: u32,
    author: Option<&str>,
    path: Option<&str>,
) -> Result<Vec<Commit>> {
    log_commits(repo, max, author, path, true, 0, &Scope::Current)
}

/// The next page of the log: `max` commits starting `skip` commits past the head.
/// `--skip` counts in the same order `git log` prints, so page 0 + page 1 read as one
/// continuous list — until someone commits, which shifts the boundary by one. That is
/// the same trade every paginated history makes; refetching the whole log to be safe
/// costs a full walk per page.
pub fn commits_page(repo: &str, skip: u32, max: u32, topo: bool, scope: &Scope) -> Result<Vec<Commit>> {
    log_commits(repo, max, None, None, topo, skip, scope)
}

/// The history **of one revision** — a branch picked out of the log bar's scope popup.
/// `git log <rev>` walks that rev's ancestors, so commits only other branches reached
/// drop out; that is the whole point of picking a branch there.
pub fn commits_on(repo: &str, rev: &str, max: u32, topo: bool) -> Result<Vec<Commit>> {
    log_commits(repo, max, None, None, topo, 0, &Scope::Rev(rev.to_owned()))
}

/// Builds the `git log` argv. Split from the parse so a test can prove the topology
/// switch adds *only* that one flag — a switch that also changed the filters would
/// silently change which commits the user sees.
/// `git log` 那一条记录的行格式，`log_args`、`file_log`、`recent_activity` 三处共用。
/// 分隔符用 `\x1f`（单元分隔符）：提交说明里不会出现它，而逗号、引号、tab 都会。
const LOG_FMT: &str = "%H%x1f%h%x1f%an%x1f%ae%x1f%aI%x1f%s%x1f%P";

fn log_args(
    max: u32,
    author: Option<&str>,
    path: Option<&str>,
    topo: bool,
    skip: u32,
    scope: &Scope,
) -> Vec<String> {
    let fmt = LOG_FMT;
    let mut args: Vec<String> = vec![
        "log".into(),
        format!("--max-count={}", max),
        format!("--pretty=format:{}", fmt),
    ];
    if skip > 0 {
        args.push(format!("--skip={}", skip));
    }
    if topo {
        args.push("--topo-order".into());
    }
    if let Some(a) = author {
        let t = a.trim();
        if !t.is_empty() {
            args.push(format!("--author={}", t));
        }
    }
    // The scope takes the single slot **before** the `--`: after it, git reads the same
    // string as a pathspec and the log silently comes back empty — which is true for
    // `--all` too, so both arms of the match sit ahead of the pathspec block.
    match scope {
        Scope::All => args.push("--all".into()),
        Scope::Current => {}
        Scope::Rev(r) => {
            let t = r.trim();
            if !t.is_empty() {
                args.push(t.to_string());
            }
        }
    }
    if let Some(p) = path {
        let q = p.trim();
        if !q.is_empty() {
            // A bare pathspec is a *prefix* match, so in a monorepo typing `admin`
            // returned every commit ("筛选没反应") while `main.ts` returned none.
            // The two globs add the substring match users expect: `**/*q*` catches
            // a fragment in the file name, `**/*q*/**` a fragment in a directory.
            args.push("--".into());
            args.push(q.to_string());
            args.push(format!(":(glob,icase)**/*{}*", q));
            args.push(format!(":(glob,icase)**/*{}*/**", q));
        }
    }
    args
}

fn parse_log(out: &str) -> Vec<Commit> {
    let mut commits = Vec::new();
    for line in out.split('\n') {
        if line.is_empty() {
            continue;
        }
        let p: Vec<&str> = line.split('\x1f').collect();
        if p.len() < 7 {
            continue;
        }
        let parents: Vec<String> = p[6].split_whitespace().map(|s| s.to_string()).collect();
        commits.push(Commit {
            id: p[0].to_string(),
            short_id: p[1].to_string(),
            author: p[2].to_string(),
            author_email: p[3].to_string(),
            date: p[4].to_string(),
            subject: p[5].to_string(),
            parents,
        });
    }
    commits
}

fn log_commits(
    repo: &str,
    max: u32,
    author: Option<&str>,
    path: Option<&str>,
    topo: bool,
    skip: u32,
    scope: &Scope,
) -> Result<Vec<Commit>> {
    let out = match run_git(repo, &str_slice(&log_args(max, author, path, topo, skip, scope))) {
        Ok(out) => out,
        // 刚 `git init` 出来的仓库有分支、还没有一条提交：`git log` 在这种仓库上退出 128，
        // 说的是 `does not have any commits yet`。那是**一份空的日志**，不是错误——
        // 当成错误的话，「新建仓库」开出来的那本永远读不出快照，界面卡在转圈上。
        Err(e) if format!("{e}").contains("does not have any commits yet") => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(parse_log(&out))
}

/// Tracked file paths, which is what the history page's path filter completes
/// against. Loaded on first focus of that input rather than on repo switch —
/// a monorepo's list is tens of thousands of lines and the webview has no need
/// for it until the user actually reaches for the filter.
pub fn repo_files(repo: &str, max: u32) -> Result<Vec<String>> {
    let out = run_git(repo, &["ls-files"])?;
    Ok(out.lines().take(max as usize).map(|s| s.to_string()).collect())
}

// ----------------------------------------------------------------------------
// Single-file history
// ----------------------------------------------------------------------------

/// History of one path, following it across renames (`--follow`).
/// `--name-status` adds what each commit did to this file, which plain
/// `git log -- <path>` does not tell us.
pub fn file_log(repo: &str, path: &str, max: u32) -> Result<Vec<FileLogEntry>> {
    let fmt = LOG_FMT;
    let fmt_arg = format!("--pretty=format:{}", fmt);
    let max_arg = format!("--max-count={}", max);
    let out = run_git(
        repo,
        &[
            "log",
            "--follow",
            &max_arg,
            &fmt_arg,
            "-M",
            "--name-status",
            "--",
            path,
        ],
    )?;
    let mut entries: Vec<FileLogEntry> = Vec::new();
    for line in out.split('\n') {
        if line.is_empty() {
            continue;
        }
        if line.contains('\x1f') {
            let p: Vec<&str> = line.split('\x1f').collect();
            if p.len() < 7 {
                continue;
            }
            entries.push(FileLogEntry {
                commit: Commit {
                    id: p[0].to_string(),
                    short_id: p[1].to_string(),
                    author: p[2].to_string(),
                    author_email: p[3].to_string(),
                    date: p[4].to_string(),
                    subject: p[5].to_string(),
                    parents: p[6].split_whitespace().map(|s| s.to_string()).collect(),
                },
                change: ChangeStatus::Unknown,
                old_path: None,
            });
        } else if let Some(last) = entries.last_mut() {
            // "M\tnew.txt" | "A\tnew.txt" | "R100\told.txt\tnew.txt"
            let mut parts = line.split('\t');
            let code = parts.next().unwrap_or("").chars().next();
            let first_name = parts.next().unwrap_or("");
            match code {
                Some('A') => last.change = ChangeStatus::Added,
                Some('D') => last.change = ChangeStatus::Deleted,
                Some('M') => last.change = ChangeStatus::Modified,
                Some('R') | Some('C') => {
                    last.change = ChangeStatus::Renamed;
                    last.old_path = Some(first_name.to_string());
                }
                _ => {}
            }
        }
    }
    Ok(entries)
}

/// Contents of `path` as of `oid`.
pub fn rev_file_text(repo: &str, oid: &str, path: &str) -> Result<String> {
    let spec = format!("{}:{}", oid, path);
    match run_git(repo, &["show", &spec]) {
        Ok(text) => {
            if text.len() > 2 * 1024 * 1024 {
                return Ok(format!("（该版本约 {} KB，未展示内容）", text.len() / 1024));
            }
            Ok(text)
        }
        // Binary blobs fail the UTF-8 conversion, and deleted paths fail in git;
        // either way report it in the pane rather than raising an error toast.
        Err(e) => Ok(format!("（无法展示该版本：{}）", e)),
    }
}

/// Put `path` back to how it looked at `oid` (git also stages the result).
pub fn restore_file(repo: &str, oid: &str, path: &str) -> Result<()> {
    run_git(repo, &["checkout", oid, "--", path])?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Commit operations (cherry-pick / reset)
// ----------------------------------------------------------------------------

pub fn cherry_pick(repo: &str, oid: &str) -> Result<()> {
    run_git(repo, &["cherry-pick", "--no-edit", oid])?;
    Ok(())
}

pub fn reset(repo: &str, oid: &str, mode: &str) -> Result<()> {
    let arg = match mode {
        "soft" => "--soft",
        "hard" => "--hard",
        _ => "--mixed",
    };
    run_git(repo, &["reset", arg, oid])?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Conflict resolution
// ----------------------------------------------------------------------------

pub fn resolve_ours(repo: &str, path: &str) -> Result<()> {
    run_git(repo, &["checkout", "--ours", "--", path])?;
    run_git(repo, &["add", "--", path])?;
    Ok(())
}

pub fn resolve_theirs(repo: &str, path: &str) -> Result<()> {
    run_git(repo, &["checkout", "--theirs", "--", path])?;
    run_git(repo, &["add", "--", path])?;
    Ok(())
}

/// Mark a hand-edited file as resolved: stage whatever is in the work tree as-is.
pub fn mark_resolved(repo: &str, path: &str) -> Result<()> {
    run_git(repo, &["add", "--", path])?;
    Ok(())
}

/// Put the conflict markers back: `checkout -m` rebuilds the merged file from the three
/// stages, so the work tree carries both sides again and the path leaves the index.
pub fn mark_unresolved(repo: &str, path: &str) -> Result<()> {
    run_git(repo, &["checkout", "-m", "--", path])?;
    Ok(())
}

/// Read the raw (possibly conflict-marked) contents of a file in the work tree.
pub fn conflict_content(repo: &str, path: &str) -> Result<String> {
    let root = run_git(repo, &["rev-parse", "--show-toplevel"])?
        .trim()
        .to_string();
    if root.is_empty() {
        return Err(GitxError::Other("无法确定仓库根目录".into()));
    }
    let full = std::path::Path::new(&root).join(path);
    std::fs::read_to_string(&full).map_err(GitxError::Io)
}

/// Write user-edited content for a conflicted file and stage it as resolved.
pub fn resolve_manual(repo: &str, path: &str, content: &str) -> Result<()> {
    let root = run_git(repo, &["rev-parse", "--show-toplevel"])?
        .trim()
        .to_string();
    if root.is_empty() {
        return Err(GitxError::Other("无法确定仓库根目录".into()));
    }
    let full = std::path::Path::new(&root).join(path);
    std::fs::write(&full, content)?;
    run_git(repo, &["add", "--", path])?;
    Ok(())
}

pub fn continue_op(repo: &str) -> Result<()> {
    // `--continue` 的三兄弟默认都要开 $EDITOR。GUI 里没有终端（从 Finder 启动的 app 连
    // TERM 都没有），只会得到 "Terminal is dumb, but EDITOR unset" —— CI 上同一报错。
    // git 早已把默认信息备在 MERGE_MSG 里，`core.editor=true` 让它直接采纳、不问编辑器。
    match repo_state(repo)?.as_str() {
        "merge" => {
            run_git(repo, &["-c", "core.editor=true", "merge", "--continue"])?;
        }
        "cherry-pick" => {
            run_git(repo, &["-c", "core.editor=true", "cherry-pick", "--continue"])?;
        }
        "rebase" => {
            run_git(repo, &["-c", "core.editor=true", "rebase", "--continue"])?;
        }
        _ => {}
    }
    Ok(())
}

pub fn abort_op(repo: &str) -> Result<()> {
    match repo_state(repo)?.as_str() {
        "merge" => {
            run_git(repo, &["merge", "--abort"])?;
        }
        "cherry-pick" => {
            run_git(repo, &["cherry-pick", "--abort"])?;
        }
        "rebase" => {
            run_git(repo, &["rebase", "--abort"])?;
        }
        _ => {}
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Blame (file annotation)
// ----------------------------------------------------------------------------

pub fn blame(repo: &str, path: &str, oid: Option<&str>) -> Result<Vec<BlameLine>> {
    use chrono::{Local, TimeZone};

    let mut args: Vec<String> = vec!["blame".into(), "--line-porcelain".into()];
    if let Some(o) = oid {
        args.push(o.to_string());
    }
    args.push("--".into());
    args.push(path.to_string());
    let out = run_git(repo, &str_slice(&args))?;

    let mut result = Vec::new();
    let mut sha = String::new();
    let mut author = String::new();
    let mut date = String::new();
    let mut line_no: u32 = 0;

    for raw in out.lines() {
        if raw.starts_with('\t') {
            // content line terminates the current block
            result.push(BlameLine {
                commit: sha.clone(),
                short_id: sha.chars().take(8).collect(),
                author: author.clone(),
                date: date.clone(),
                line_no,
                content: raw[1..].to_string(),
            });
            continue;
        }
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.len() == 4
            && parts[0].len() == 40
            && parts[0].chars().all(|c| c.is_ascii_hexdigit())
        {
            sha = parts[0].to_string();
            line_no = parts[2].parse().unwrap_or(0);
            author.clear();
            date.clear();
        } else if let Some(a) = raw.strip_prefix("author ") {
            author = a.to_string();
        } else if let Some(t) = raw.strip_prefix("author-time ") {
            if let Ok(secs) = t.trim().parse::<i64>() {
                date = Local
                    .timestamp_opt(secs, 0)
                    .single()
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default();
            }
        }
    }
    Ok(result)
}

// ----------------------------------------------------------------------------
// Diff
// ----------------------------------------------------------------------------

fn parse_diff_output(raw: &str, num: &str) -> Vec<DiffEntry> {
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    for line in num.lines() {
        let mut it = line.splitn(3, '\t');
        let a = it.next().unwrap_or("0");
        let d = it.next().unwrap_or("0");
        let p = it.next().unwrap_or("").to_string();
        if p.is_empty() {
            continue;
        }
        let add = if a == "-" { 0 } else { a.parse().unwrap_or(0) };
        let del = if d == "-" { 0 } else { d.parse().unwrap_or(0) };
        counts.insert(p, (add, del));
    }

    let mut out = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let (meta, paths) = match line.split_once('\t') {
            Some(x) => x,
            None => continue,
        };
        let fields: Vec<&str> = meta.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        let status = fields[4];
        let status_char = status.chars().next().unwrap_or('?');
        let paths: Vec<&str> = paths.split('\t').collect();
        let (path, old_path) = if paths.len() >= 2 {
            (paths[1].to_string(), Some(paths[0].to_string()))
        } else {
            (paths.first().copied().unwrap_or("").to_string(), None)
        };
        let (add, del) = counts.get(&path).copied().unwrap_or((0, 0));
        out.push(DiffEntry {
            path: path.clone(),
            old_path,
            status: map_status_char(status_char),
            additions: add,
            deletions: del,
        });
    }
    out
}

pub fn working_diff(repo: &str, staged: bool) -> Result<Vec<DiffEntry>> {
    let base: &[&str] = if staged { &["--cached"] } else { &[] };
    let raw = run_git(repo, &concat_args(&["diff"], base, &["--raw", "-M"]))?;
    let num = run_git(repo, &concat_args(&["diff"], base, &["--numstat"]))?;
    Ok(parse_diff_output(&raw, &num))
}

pub fn range_diff(repo: &str, a: &str, b: &str) -> Result<Vec<DiffEntry>> {
    let raw = run_git(repo, &["diff", "--raw", "-M", a, b])?;
    let num = run_git(repo, &["diff", "--numstat", a, b])?;
    Ok(parse_diff_output(&raw, &num))
}

pub fn commit_diff(repo: &str, oid: &str) -> Result<Vec<DiffEntry>> {
    let raw = run_git(repo, &["show", "--raw", "-M", oid])?;
    let num = run_git(repo, &["show", "--numstat", oid])?;
    Ok(parse_diff_output(&raw, &num))
}

pub fn working_patch(repo: &str, staged: bool, path: Option<&str>) -> Result<String> {
    let mut args: Vec<String> = vec!["diff".into()];
    if staged {
        args.push("--cached".into());
    }
    args.push("--no-color".into());
    if let Some(p) = path {
        args.push("--".into());
        args.push(p.into());
    }
    run_git(repo, &str_slice(&args))
}

/// `git diff` shows nothing for an untracked file, so synthesize an "added file"
/// patch from its contents — otherwise selecting a new file looks like a no-op.
pub fn untracked_patch(repo: &str, path: &str) -> Result<String> {
    let full = std::path::Path::new(repo).join(path);
    let meta = std::fs::metadata(&full)?;
    let header = format!("diff --git a/{p} b/{p}\n", p = path);
    if meta.len() > 2 * 1024 * 1024 {
        return Ok(format!(
            "{}（新文件较大，约 {} KB，未展示内容）",
            header,
            meta.len() / 1024
        ));
    }
    let bytes = std::fs::read(&full)?;
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return Ok(format!("{}（二进制新文件，{} 字节）", header, bytes.len()));
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = if text.is_empty() {
        vec![]
    } else {
        text.trim_end_matches('\n').split('\n').collect()
    };
    let mut out = String::new();
    out.push_str(&format!("diff --git a/{p} b/{p}\n", p = path));
    out.push_str("new file mode 100644\n");
    out.push_str(&format!("--- /dev/null\n+++ b/{p}\n", p = path));
    out.push_str(&format!("@@ -0,0 +1,{} @@\n", lines.len().max(1)));
    for l in lines {
        out.push('+');
        out.push_str(l);
        out.push('\n');
    }
    Ok(out)
}

pub fn range_patch(repo: &str, a: &str, b: &str, path: Option<&str>) -> Result<String> {
    let mut args: Vec<String> = vec![
        "diff".into(),
        "--no-color".into(),
        "-M".into(),
        a.into(),
        b.into(),
    ];
    if let Some(p) = path {
        args.push("--".into());
        args.push(p.into());
    }
    run_git(repo, &str_slice(&args))
}

pub fn commit_patch(repo: &str, oid: &str, path: Option<&str>) -> Result<String> {
    // `--format=` 把 `git show` 自带的日志头（`commit <oid>` / `Author:` / `Date:` +
    // 空行 + 缩进的提交说明）整个掐掉，只留补丁。SourceTree 的差异区没有这几行：
    // 参考图 `sourcetree-4.2.19-repo-window.png` 文件名那条灰带 (236,236,236) 到 y=368pt 结束，
    // y=368..396 立刻换成 hunk 那条带 (240,244,241) —— 中间一行文件头都没有。
    // 提交说明在日志那一行已经写过了，这里再抄一遍是重复。
    let mut args: Vec<String> = vec![
        "show".into(),
        "--no-color".into(),
        "-M".into(),
        "--format=".into(),
        oid.into(),
    ];
    if let Some(p) = path {
        args.push("--".into());
        args.push(p.into());
    }
    run_git(repo, &str_slice(&args))
}

fn concat_args<'a>(head: &'a [&'a str], base: &'a [&'a str], tail: &'a [&'a str]) -> Vec<&'a str> {
    let mut v: Vec<&str> = Vec::new();
    v.extend_from_slice(head);
    v.extend_from_slice(base);
    v.extend_from_slice(tail);
    v
}

fn str_slice(args: &[String]) -> Vec<&str> {
    args.iter().map(|s| s.as_str()).collect()
}

/// 从一份补丁里抠出第 `index`（**从 1 数**，和差异区那句「块 N」同一个数）个 hunk，
/// 连它自己那格文件的头（`diff --git` 到第一个 `@@` 之前的所有行）。
///
/// 只给「块 N」不给「这个文件的第几块」是有意的：`layout()` 数 hunk 也不按文件重置，
/// 两边同一套数法，UI 传下来的数字才不用再换算。多文件补丁里第 2 块可能落在第 2 个文件上，
/// 那时带出来的头就是第 2 个文件的头。
pub fn hunk_patch(patch: &str, index: usize) -> Option<String> {
    let mut head: Vec<&str> = Vec::new();
    let mut seen = 0usize;
    let mut body: Option<Vec<&str>> = None;
    // 「已经在某一块的正文里」——目标块之前的那些行属于**别的**块，既不能进头也不能漏下去，
    // 只能丢掉。少了这个标记，抠第 2 块会把第 1 块的 `+new` 当文件头带出去。
    let mut in_hunk = false;
    for line in patch.lines() {
        if line.starts_with("@@") {
            // 已经收到目标块，再遇 `@@` 就是下一块开始了 —— 必须在这儿收手，
            // 不然下一块的正文会拼到目标块尾巴上（判据 `…_with_its_own_header` 抓过一次）。
            if body.is_some() {
                break;
            }
            seen += 1;
            in_hunk = true;
            if seen == index {
                body = Some(vec![line]);
            }
            continue;
        }
        // 收到目标块以后再遇到 `diff --git`，说明那一块已经结束。
        if line.starts_with("diff --git ") {
            if body.is_some() {
                break;
            }
            head.clear();
            in_hunk = false;
            head.push(line);
            continue;
        }
        match &mut body {
            Some(v) => v.push(line),
            None => {
                if !in_hunk {
                    head.push(line);
                }
            }
        }
    }
    let body = body?;
    // 没有 `---`/`+++` 这一对，git 不认这是补丁；宁可返回 None 让调用方报错，
    // 也别把一份半截补丁喂给 `git apply`。
    if !head.iter().any(|l| l.starts_with("--- ")) {
        return None;
    }
    let mut out = String::new();
    for line in head.iter().chain(body.iter()) {
        out.push_str(line);
        out.push('\n');
    }
    Some(out)
}

/// 把补丁从标准输入喂给 git（`git apply -`）。不落临时文件：临时文件在并发或崩溃时会留残骸，
/// 而且 `git apply` 拿文件路径时把路径当相对当前目录，`-C repo` 那层就得再绕一遍。
fn run_git_stdin(repo: &str, args: &[&str], input: &str) -> Result<String> {
    use std::io::Write;
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(CONFIG_ARGS)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .expect("上面开了 Stdio::piped()")
        .write_all(input.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(GitxError::Git(explain_git_error(&String::from_utf8_lossy(
            &out.stderr,
        ))));
    }
    Ok(decode(&out.stdout))
}

/// 只把某一块改动放进暂存区（SourceTree 的「暂存区块」）。
/// `index` 是差异区那句「块 N」的 N。
pub fn stage_hunk(repo: &str, path: &str, index: usize) -> Result<()> {
    let patch = working_patch(repo, false, Some(path))?;
    let one = hunk_patch(&patch, index).ok_or_else(|| {
        GitxError::Other(format!("{path} 没有第 {index} 块改动，可能列表刚刷新过"))
    })?;
    run_git_stdin(repo, &["apply", "--cached", "--whitespace=nowarn", "-"], &one)?;
    Ok(())
}

/// 把暂存区里的某一块退回工作区（SourceTree 的「取消暂存区块」）。
/// 反着打的是**已暂存**那份补丁，所以工作区的其它改动不会被动到。
pub fn unstage_hunk(repo: &str, path: &str, index: usize) -> Result<()> {
    let patch = working_patch(repo, true, Some(path))?;
    let one = hunk_patch(&patch, index).ok_or_else(|| {
        GitxError::Other(format!("{path} 的暂存区里没有第 {index} 块改动，可能列表刚刷新过"))
    })?;
    run_git_stdin(
        repo,
        &["apply", "--cached", "-R", "--whitespace=nowarn", "-"],
        &one,
    )?;
    Ok(())
}

/// 丢掉工作区里的某一块（SourceTree 的「放弃区块」）。**这条会真删掉用户没提交的行**，
/// 和上面两条不是一个风险级别：上面两条只挪索引，文件内容一个字都不动。
/// 所以调用方必须自己先要一次确认（egui 壳里是「按两下」，见 `a_discard_needs_two_clicks`）。
pub fn discard_hunk(repo: &str, path: &str, index: usize) -> Result<()> {
    let patch = working_patch(repo, false, Some(path))?;
    let one = hunk_patch(&patch, index).ok_or_else(|| {
        GitxError::Other(format!("{path} 没有第 {index} 块改动，可能列表刚刷新过"))
    })?;
    // 不带 `--cached`：反打的落点就是工作区那个文件。
    run_git_stdin(repo, &["apply", "-R", "--whitespace=nowarn", "-"], &one)?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Working tree operations
// ----------------------------------------------------------------------------

pub fn stage(repo: &str, paths: &[String]) -> Result<()> {
    let mut args: Vec<String> = vec!["add".into(), "--".into()];
    args.extend(paths.iter().cloned());
    run_git(repo, &str_slice(&args))?;
    Ok(())
}

pub fn stage_all(repo: &str) -> Result<()> {
    run_git(repo, &["add", "-A"])?;
    Ok(())
}

pub fn unstage(repo: &str, paths: &[String]) -> Result<()> {
    let mut args: Vec<String> = vec!["reset".into(), "--".into()];
    args.extend(paths.iter().cloned());
    run_git(repo, &str_slice(&args))?;
    Ok(())
}

pub fn discard(repo: &str, paths: &[String]) -> Result<()> {
    let mut args: Vec<String> = vec!["checkout".into(), "--".into()];
    args.extend(paths.iter().cloned());
    run_git(repo, &str_slice(&args))?;
    Ok(())
}

/// Append `path` to the repository's `.gitignore` (relative to the work tree root).
/// No-op if the pattern is already present. Does not touch the index, so already
/// tracked files remain tracked (use `git rm --cached` separately if needed).
pub fn ignore_path(repo: &str, path: &str) -> Result<()> {
    let root = run_git(repo, &["rev-parse", "--show-toplevel"])?
        .trim()
        .to_string();
    if root.is_empty() {
        return Err(GitxError::Other("无法确定仓库根目录".into()));
    }
    let ignore_file = std::path::Path::new(&root).join(".gitignore");
    let mut content = std::fs::read_to_string(&ignore_file).unwrap_or_default();
    let line = path.trim().trim_start_matches('/').to_string();
    if content.lines().any(|l| l.trim() == line) {
        return Ok(());
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&line);
    content.push('\n');
    std::fs::write(&ignore_file, content)?;
    Ok(())
}

/// Read the repository's `.gitignore` (empty string if it does not exist).
pub fn read_gitignore(repo: &str) -> Result<String> {
    let root = run_git(repo, &["rev-parse", "--show-toplevel"])?
        .trim()
        .to_string();
    if root.is_empty() {
        return Err(GitxError::Other("无法确定仓库根目录".into()));
    }
    let ignore_file = std::path::Path::new(&root).join(".gitignore");
    Ok(std::fs::read_to_string(&ignore_file).unwrap_or_default())
}

/// Overwrite the repository's `.gitignore` with `content`.
pub fn write_gitignore(repo: &str, content: &str) -> Result<()> {
    let root = run_git(repo, &["rev-parse", "--show-toplevel"])?
        .trim()
        .to_string();
    if root.is_empty() {
        return Err(GitxError::Other("无法确定仓库根目录".into()));
    }
    let ignore_file = std::path::Path::new(&root).join(".gitignore");
    std::fs::write(&ignore_file, content)?;
    Ok(())
}

pub fn commit(
    repo: &str,
    message: &str,
    amend: bool,
    paths: &[String],
    sign: bool,
    no_verify: bool,
) -> Result<String> {
    let mut args: Vec<String> = vec!["commit".into()];
    if amend {
        args.push("--amend".into());
    }
    if sign {
        args.push("-S".into());
    }
    if no_verify {
        args.push("--no-verify".into());
    }
    args.push("-m".into());
    args.push(message.into());
    if !paths.is_empty() {
        args.push("--".into());
        args.extend(paths.iter().cloned());
    }
    run_git(repo, &str_slice(&args))?;
    head(repo)
}

// ----------------------------------------------------------------------------
// Branches
// ----------------------------------------------------------------------------

pub fn branches(repo: &str) -> Result<Vec<Branch>> {
    let out = run_git(
        repo,
        &[
            "branch",
            "-vv",
            "--format=%(refname:short)|%(HEAD)|%(upstream:short)|%(objectname)",
        ],
    )?;
    let mut v = Vec::new();
    for line in out.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        let name = parts[0].to_string();
        let is_current = parts.get(1).map(|h| *h == "*").unwrap_or(false);
        let upstream = parts
            .get(2)
            .filter(|u| !u.is_empty())
            .map(|s| s.to_string());
        let target = parts.get(3).unwrap_or(&"").to_string();
        let (ahead, behind) = match &upstream {
            Some(up) => get_ahead_behind(repo, &name, up)?,
            None => (0, 0),
        };
        v.push(Branch {
            name,
            is_current,
            upstream,
            ahead,
            behind,
            target,
        });
    }
    Ok(v)
}

/// Remote-tracking refs (`origin/main`). `branches()` deliberately leaves them out,
/// so this is a separate query — the branch page must not list something you cannot
/// check out. `origin/HEAD` is a pointer at the default branch rather than a
/// position, so it is dropped.
///
/// `%(refname:short)` is *not* usable here: for the symbolic `refs/remotes/origin/HEAD`
/// it strips the last component and prints `origin`, so the filter above would never
/// fire and every repository would grow a chip labelled with nothing but the remote's
/// name. `lstrip=2` cuts the `refs/remotes/` prefix and keeps the full tail.
pub fn remote_refs(repo: &str) -> Result<Vec<RemoteRef>> {
    let out = run_git(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:lstrip=2)|%(objectname)",
            "refs/remotes",
        ],
    )?;
    Ok(out
        .lines()
        .filter_map(|line| {
            let (name, target) = line.split_once('|')?;
            if name.is_empty() || name.ends_with("/HEAD") {
                return None;
            }
            Some(RemoteRef {
                name: name.to_string(),
                target: target.to_string(),
            })
        })
        .collect())
}

fn get_ahead_behind(repo: &str, branch: &str, upstream: &str) -> Result<(u32, u32)> {
    let ahead = run_git(
        repo,
        &["rev-list", "--count", &format!("{}..{}", upstream, branch)],
    )
    .map(|s| s.trim().parse().unwrap_or(0))
    .unwrap_or(0);
    let behind = run_git(
        repo,
        &["rev-list", "--count", &format!("{}..{}", branch, upstream)],
    )
    .map(|s| s.trim().parse().unwrap_or(0))
    .unwrap_or(0);
    Ok((ahead, behind))
}

pub fn create_branch(
    repo: &str,
    name: &str,
    start_point: Option<&str>,
    checkout: bool,
) -> Result<()> {
    let mut args: Vec<String> = vec!["branch".into(), name.into()];
    if let Some(sp) = start_point {
        args.push(sp.into());
    }
    run_git(repo, &str_slice(&args))?;
    if checkout {
        checkout_branch(repo, name)?;
    }
    Ok(())
}

pub fn checkout_branch(repo: &str, target: &str) -> Result<()> {
    run_git(repo, &["checkout", target])?;
    Ok(())
}

pub fn delete_branch(repo: &str, name: &str, force: bool) -> Result<()> {
    let args: &[&str] = if force {
        &["branch", "-D", name]
    } else {
        &["branch", "-d", name]
    };
    run_git(repo, args)?;
    Ok(())
}

pub fn rename_branch(repo: &str, old: &str, new: &str) -> Result<()> {
    run_git(repo, &["branch", "-m", old, new])?;
    Ok(())
}

pub fn merge_branch(repo: &str, name: &str) -> Result<()> {
    run_git(repo, &["merge", name])?;
    Ok(())
}

pub fn rebase_branch(repo: &str, onto: &str) -> Result<()> {
    run_git(repo, &["rebase", onto])?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Interactive rebase (pick / squash / fixup / drop, with reordering)
// ----------------------------------------------------------------------------

/// Apply an interactive rebase plan: `plan` is an ordered list of
/// `(commit_oid, action)` where action is one of pick/p/squash/s/fixup/f/drop/d.
/// `base` is the commit the rebase starts from (itself excluded from the plan).
pub fn interactive_rebase(repo: &str, base: &str, plan: &[(String, String)]) -> Result<()> {
    // Ensure base exists.
    run_git(repo, &["rev-parse", "--verify", "--quiet", base])?;

    let valid = ["pick", "p", "squash", "s", "fixup", "f", "drop", "d"];
    let mut todo = String::new();
    for (oid, action) in plan {
        let a = action.as_str();
        if !valid.contains(&a) {
            return Err(GitxError::Other(format!("不支持的变基操作: {}", a)));
        }
        let subject = run_git(repo, &["log", "--format=%s", "-n", "1", oid])
            .unwrap_or_default()
            .trim()
            .to_string();
        todo.push_str(&format!("{} {} {}\n", a, oid, subject));
    }
    if todo.trim().is_empty() {
        return Err(GitxError::Other("变基计划为空".into()));
    }

    // Write the todo list to a temp file and replay it via GIT_SEQUENCE_EDITOR.
    let dir = std::env::temp_dir();
    let path = dir.join(format!("gitx-rebase-{}.todo", std::process::id()));
    std::fs::write(&path, todo)?;
    let path_str = path.to_string_lossy().to_string();
    let seq_editor = format!("sh -c 'cp {} \"$1\"'", path_str);

    let res = run_git_env(
        repo,
        &["rebase", "-i", base],
        &[("GIT_SEQUENCE_EDITOR", &seq_editor)],
    );
    let _ = std::fs::remove_file(&path);
    res?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Tags
// ----------------------------------------------------------------------------

/// 标签列表。`target` **一律是被指向的那个提交号**，不是标签对象号：
/// annotated tag 的 `%(objectname)` 是 tag 对象，拿它去和 `Commit.id` 相等判断永远不成立，
/// 于是「打了标签的那一行」在日志里少一枚药丸——而这是唯一能看出它和轻量标签不同的地方。
/// `%(*objectname)` 是 git 的 peel 字段：轻量标签那里是空的，所以要回退。
pub fn tags(repo: &str) -> Result<Vec<Tag>> {
    let out = run_git(
        repo,
        &[
            "tag",
            "-l",
            "--format=%(refname:short)|%(objecttype)|%(objectname)|%(*objectname)",
        ],
    )?;
    let mut v = Vec::new();
    for line in out.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        let object = parts.get(2).unwrap_or(&"");
        let peeled = parts.get(3).unwrap_or(&"");
        v.push(Tag {
            name: parts[0].to_string(),
            target: if peeled.is_empty() {
                object.to_string()
            } else {
                peeled.to_string()
            },
            annotated: parts.get(1).map(|t| *t == "tag").unwrap_or(false),
        });
    }
    Ok(v)
}

pub fn create_tag(repo: &str, name: &str, target: &str, message: Option<&str>) -> Result<()> {
    match message {
        Some(m) => {
            run_git(repo, &["tag", "-a", name, "-m", m, target])?;
        }
        None => {
            run_git(repo, &["tag", name, target])?;
        }
    }
    Ok(())
}

pub fn delete_tag(repo: &str, name: &str) -> Result<()> {
    run_git(repo, &["tag", "-d", name])?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Remotes
// ----------------------------------------------------------------------------

pub fn remotes(repo: &str) -> Result<Vec<Remote>> {
    let out = run_git(repo, &["remote"])?;
    let mut v = Vec::new();
    for name in out.lines() {
        if name.is_empty() {
            continue;
        }
        let url = run_git(repo, &["remote", "get-url", name]).unwrap_or_default();
        let push = run_git(repo, &["remote", "get-url", "--push", name])
            .ok()
            .filter(|s| !s.trim().is_empty());
        v.push(Remote {
            name: name.to_string(),
            url: url.trim().to_string(),
            push_url: push.map(|s| s.trim().to_string()),
        });
    }
    Ok(v)
}

/// Apply a node template to one URL. `{url}` is the placeholder; a template
/// without it is a prefix, which is how most nodes document themselves.
pub fn apply_node_template(tpl: &str, url: &str) -> String {
    let t = tpl.trim();
    if t.contains("{url}") {
        t.replace("{url}", url)
    } else {
        let sep = if t.ends_with('/') { "" } else { "/" };
        format!("{}{}{}", t, sep, url)
    }
}

/// `-c` overrides that route ONE command through an acceleration node.
///
/// The matched base is the whole remote URL rather than just `https://host/`, so
/// the node sees exactly the string a clone would give it
/// (`https://node/https://github.com/a/b.git`), and nothing else on that host is
/// silently redirected behind the user's back. Nothing lands in the repo's
/// config: `origin` keeps the real URL, so `push` — which a read-only node cannot
/// serve — and a later plain `fetch` still go where the user pointed the repo.
/// A template is untrusted text from a settings field, so characters that would
/// break git's `key=value` parse are refused instead of escaped.
pub fn node_rewrite_args(node_tpl: &str, remote_url: &str) -> Result<Vec<String>> {
    if node_tpl.trim().is_empty() {
        return Err(GitxError::Other("请先填写一个加速节点地址。".into()));
    }
    let url = remote_url.trim();
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(GitxError::Other(format!(
            "加速节点只能改写 https 形式的远程，「{}」走不了节点。SSH 地址请配置代理，或在「设置 → SSH」里试连。",
            url
        )));
    }
    let section = apply_node_template(node_tpl, url);
    let unusable = section.is_empty()
        || section.contains(['=', '"', '\''])
        || section.chars().any(char::is_whitespace);
    if unusable {
        return Err(GitxError::Other(format!(
            "加速节点「{}」不是 git 能接受的地址。请填写完整 https 地址，例如 https://node.example/{{url}}。",
            node_tpl.trim()
        )));
    }
    Ok(vec![
        "-c".to_string(),
        format!("url.{}.insteadOf={}", section, url),
    ])
}

/// URL a remote actually points at — the thing a node rewrites.
/// Public because the shell's「远程服务器」button needs it to build a browsable page.
pub fn remote_url(repo: &str, remote: &str) -> Result<String> {
    let url = run_git(repo, &["remote", "get-url", remote])?;
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(GitxError::Other(format!(
            "这个仓库没有名为「{}」的远程。",
            remote
        )));
    }
    Ok(url)
}

/// Fetch, optionally through an acceleration node for this single call.
pub fn fetch(repo: &str, remote: Option<&str>, node: Option<&str>) -> Result<()> {
    let rewrite = match node {
        Some(tpl) => node_rewrite_args(tpl, &remote_url(repo, remote.unwrap_or("origin"))?)?,
        None => Vec::new(),
    };
    fetch_rewritten(repo, remote, &rewrite)
}

/// The git half of [`fetch`], with the rewrite already turned into `-c` args.
fn fetch_rewritten(repo: &str, remote: Option<&str>, rewrite: &[String]) -> Result<()> {
    let mut args: Vec<&str> = rewrite.iter().map(String::as_str).collect();
    args.push("fetch");
    match remote {
        Some(r) => args.push(r),
        None => args.push("--all"),
    }
    run_git(repo, &args)?;
    Ok(())
}

/// 拉取卡「选项」里那三句真的改 git 参数的。
///
/// 第四句「立即提交合并的改动」（git 的 `--commit`）**没进这个结构**：它就是 git 的默认行为，
/// 摆一枚怎么按都看不出区别的勾，比不摆更糟——和偏好卡「三条都真接得通才摆得上来」同一条规矩。
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct PullOpts {
    /// 「用变基代替合并」→ `--rebase`。
    pub rebase: bool,
    /// 「无论是否可以快进更新都创建新的提交」→ `--no-ff`。
    pub no_ff: bool,
    /// 「包括被合并提交的信息内容」→ `--log`：合并提交的信息里带上被并进来的那些标题。
    pub log: bool,
}

/// Pull, optionally through an acceleration node. Only the network half of a
/// pull is redirected; the merge itself is local and unchanged.
pub fn pull(
    repo: &str,
    remote: Option<&str>,
    branch: Option<&str>,
    opts: &PullOpts,
    node: Option<&str>,
) -> Result<()> {
    let rewrite = match node {
        Some(tpl) => node_rewrite_args(tpl, &remote_url(repo, remote.unwrap_or("origin"))?)?,
        None => Vec::new(),
    };
    let mut args: Vec<&str> = rewrite.iter().map(String::as_str).collect();
    args.push("pull");
    if opts.rebase {
        args.push("--rebase");
    } else {
        // 不勾变基就是**要合并**：明说。git 2.34+ 遇到分岔又不给策略时直接拒跑
        // （"need to specify how to reconcile"），那时策略就落到了仓库配置头上，
        // 而卡上那枚勾才是用户此刻的选择。
        args.push("--no-rebase");
    }
    if opts.no_ff {
        args.push("--no-ff");
    }
    if opts.log {
        args.push("--log");
    }
    if let Some(r) = remote {
        args.push(r);
    }
    if let Some(b) = branch {
        args.push(b);
    }
    run_git(repo, &args)?;
    Ok(())
}

pub fn push(
    repo: &str,
    remote: Option<&str>,
    branch: Option<&str>,
    force: bool,
    set_upstream: bool,
) -> Result<()> {
    let mut args: Vec<String> = vec!["push".into()];
    if force {
        args.push("--force".into());
    }
    if set_upstream {
        // `-u` needs both halves named: asking git to record an upstream while
        // letting either default is how the tracking branch silently never gets
        // set, and the user retries the push a second time to no visible effect.
        if remote.is_none() || branch.is_none() {
            return Err(GitxError::Other(
                "设置上游分支必须同时指定远程与分支名。".into(),
            ));
        }
        args.push("--set-upstream".into());
    }
    if let Some(r) = remote {
        args.push(r.into());
    }
    if let Some(b) = branch {
        args.push(b.into());
    }
    run_git(repo, &str_slice(&args))?;
    Ok(())
}

/// 推送卡表格里那一行的落地：`git push [-u] <远端> <本地>:<远端那一侧>`。
///
/// 为什么不复用 [`push`]：那一条的最后一个实参是**裸分支名**，走的是 git 自己的
/// `push.default`；这一条要按卡上「远程分支」那一格说的算。两者唯一的公共部分
/// 是「起一个 push 进程」，把 refspec 塞成布尔参数会让调用方读不出这次到底带没带
/// refspec——而带 refspec 的 push 会**完全无视** `push.default`，这是那条参数的语义，
/// 不是我们的实现细节。
///
/// 远端那一侧留空 = 这一行填错了，**当场拒**：拼出 `main:` 那种半截 refspec，git 的报错
/// 是「src refspec main does not match any」，说的是左边、坏的是右边，用户看得一头雾水。
pub fn push_ref(
    repo: &str,
    remote: &str,
    local: &str,
    remote_name: &str,
    force: bool,
    set_upstream: bool,
) -> Result<()> {
    if remote.is_empty() || local.is_empty() || remote_name.is_empty() {
        return Err(GitxError::Other(
            "推送必须同时指定远端、本地分支和远端分支名。".into(),
        ));
    }
    let spec = format!("{local}:{remote_name}");
    let mut args: Vec<String> = vec!["push".into()];
    if force {
        args.push("--force".into());
    }
    if set_upstream {
        args.push("--set-upstream".into());
    }
    args.push(remote.into());
    args.push(spec);
    run_git(repo, &str_slice(&args))?;
    Ok(())
}

/// 某一个远端上有哪些分支（`origin/main` 去掉前缀 = `main`），拉取卡「要拉取的远程分支」
/// 那一格要的就是这个。
///
/// 单独一个函数而不是把 [`remote_refs`] 拿过来滤：那一条返回的是**带前缀的全名**
/// （日志上的药丸要的就是那个样子，见它自己的注释），而 `git pull <远端> <分支>` 收的是
/// 去掉远端名前缀的那一半——差的那一截正是这张卡会填错的地方，所以在源头就剥掉。
/// `HEAD` 那条指针不是位置，剥完会剩一个空串，跟着滤掉（同 [`remote_refs`] 的理由）。
pub fn remote_branches(repo: &str, remote: &str) -> Result<Vec<String>> {
    let out = run_git(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:lstrip=3)",
            &format!("refs/remotes/{remote}/"),
        ],
    )?;
    Ok(out
        .lines()
        // `lstrip=3` 把 `refs/remotes/origin/` 整段剥掉，那条默认分支指针落下来正好是
        // 光一个 `HEAD`（不是 `origin/HEAD`）——第一版只滤 `ends_with("/HEAD")`，
        // 于是那一格里混进一条选了就报错的空选项，是这条判据抓的。
        .filter(|l| !l.is_empty() && *l != "HEAD" && !l.ends_with("/HEAD"))
        .map(|l| l.to_string())
        .collect())
}

/// 推送卡上「推送所有标签」那一句：`git push <remote> --tags`。
///
/// 单独一个函数而不是给 `push` 再加一枚布尔：`--tags` 推的是**另一批东西**（标签对象，
/// 不是分支），和分支那几行没有共享的 refspec；混进同一个函数就要在里面判「这一枚布尔
/// 在的时候其余参数都不算」，那是把两条命令塞进一个函数。
pub fn push_all_tags(repo: &str, remote: &str) -> Result<()> {
    run_git(repo, &["push", remote, "--tags"])?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Stash
// ----------------------------------------------------------------------------

pub fn stash_list(repo: &str) -> Result<Vec<Stash>> {
    let out = run_git(repo, &["stash", "list", "--format=%H|%gd|%gs|%aI"])?;
    let mut v = Vec::new();
    for line in out.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() < 4 {
            continue;
        }
        let idx = parts[1]
            .trim_start_matches("stash@{")
            .trim_end_matches('}')
            .parse::<u32>()
            .unwrap_or(0);
        v.push(Stash {
            index: idx,
            oid: parts[0].to_string(),
            message: stash_message(parts[2]),
            date: parts[3].to_string(),
        });
    }
    Ok(v)
}

/// `%gs` 自带 git 那层包装：给了 `-m` 是 `On <分支>: <说明>`，没给是
/// `WIP on <分支>: <短 oid> <提交标题>`。侧栏只该显示说明本身。
/// 分支名里不可能有 `:`（git 的 ref 规则），所以第一个 `": "` 就是包装与正文的分界。
fn stash_message(raw: &str) -> String {
    for pre in ["WIP on ", "On "] {
        if let Some(rest) = raw.strip_prefix(pre) {
            if let Some((_, msg)) = rest.split_once(": ") {
                return msg.to_owned();
            }
        }
    }
    raw.to_owned()
}

/// 一条贮藏改了哪些文件。走 `stash show` 而不是 `show <oid>`：贮藏是个合并提交，
/// `git show` 对它默认按合并提交出 `--cc`，两侧都不同的文件才露头；`stash show`
/// 比的是它的第一父提交，也就是贮藏时的工作区。`-u` 把第三父提交里的未跟踪文件
/// 一并算进来（用户存的是「全部改动」，不是「已跟踪的那一半」）。
pub fn stash_diff(repo: &str, oid: &str) -> Result<Vec<DiffEntry>> {
    let raw = run_git(repo, &["stash", "show", "--raw", "-M", "-u", oid])?;
    let num = run_git(repo, &["stash", "show", "--numstat", "-u", oid])?;
    Ok(parse_diff_output(&raw, &num))
}

/// 一条贮藏的补丁。不用 `git stash show -p`：它**不接受路径**（`Too many revisions
/// specified`），而右侧那一格点单个文件要的正是路径过滤。拆成两段自己拼：
/// - 已跟踪那半边 = `stash@{n}` 相对**第一父提交**的差异；
/// - 未跟踪那半边住在贮藏的**第三父提交**里（`stash show -u` 内部也是这么算的）。
///   没带 `-u` 存出来的贮藏没有第三父提交，`^3` 会让 git 报 `bad revision`——
///   那份缺失不是错误，取不到就当没有。
pub fn stash_patch(repo: &str, oid: &str, path: Option<&str>) -> Result<String> {
    let mut tracked: Vec<String> =
        vec!["diff".into(), "--no-color".into(), "-M".into(), format!("{oid}^1"), oid.into()];
    if let Some(p) = path {
        tracked.push("--".into());
        tracked.push(p.into());
    }
    let mut out = run_git(repo, &str_slice(&tracked))?;
    let mut untracked: Vec<String> =
        vec!["show".into(), "--format=".into(), "--no-color".into(), format!("{oid}^3")];
    if let Some(p) = path {
        untracked.push("--".into());
        untracked.push(p.into());
    }
    if let Ok(extra) = run_git(repo, &str_slice(&untracked)) {
        out.push_str(&extra);
    }
    Ok(out)
}

pub fn stash_push(repo: &str, message: &str, include_untracked: bool) -> Result<()> {
    let mut args: Vec<String> = vec!["stash".into(), "push".into(), "-m".into(), message.into()];
    if include_untracked {
        args.push("-u".into());
    }
    run_git(repo, &str_slice(&args))?;
    Ok(())
}

pub fn stash_apply(repo: &str, index: u32) -> Result<()> {
    run_git(repo, &["stash", "apply", &format!("stash@{{{}}}", index)])?;
    Ok(())
}

pub fn stash_pop(repo: &str, index: u32) -> Result<()> {
    run_git(repo, &["stash", "pop", &format!("stash@{{{}}}", index)])?;
    Ok(())
}

pub fn stash_drop(repo: &str, index: u32) -> Result<()> {
    run_git(repo, &["stash", "drop", &format!("stash@{{{}}}", index)])?;
    Ok(())
}

// ----------------------------------------------------------------------------
// AI: Pull-request description context
// ----------------------------------------------------------------------------

/// Compute (base_ref, commit_log, full_diff) for the current branch relative to
/// a suitable base (its upstream if set, otherwise main/master), to feed into an
/// AI prompt that drafts a pull-request description.
pub fn draft_pr_context(repo: &str) -> Result<(String, String, String)> {
    let branches = branches(repo)?;
    let current = branches
        .iter()
        .find(|b| b.is_current)
        .ok_or_else(|| GitxError::Other("未找到当前分支".into()))?;

    let base = if let Some(up) = &current.upstream {
        up.clone()
    } else {
        let candidates = ["origin/main", "origin/master", "main", "master"];
        candidates
            .iter()
            .find(|c| run_git(repo, &["rev-parse", "--verify", "--quiet", c]).is_ok())
            .map(|c| c.to_string())
            .ok_or_else(|| {
                GitxError::Other(
                    "无法确定基准分支：当前分支无 upstream，且未发现 main/master".into(),
                )
            })?
    };

    let range = format!("{}..HEAD", base);
    let log = run_git(repo, &["log", &range, "--format=%h %s"])?;
    if log.trim().is_empty() {
        return Err(GitxError::Other(format!(
            "当前分支相对基准 {} 没有新的提交",
            base
        )));
    }
    let diff = run_git(repo, &["diff", &format!("{}...HEAD", base)])?;
    Ok((base, log, diff))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as Proc;

    fn git(repo: &str, args: &[&str]) {
        let out = Proc::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git run");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Create a temp git repo with an initial commit.
    fn init_repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap().to_string();
        git(&p, &["init", "-q"]);
        git(&p, &["config", "user.email", "t@t.com"]);
        git(&p, &["config", "user.name", "tester"]);
        fs::write(dir.path().join("a.txt"), "hello\n").unwrap();
        git(&p, &["add", "."]);
        git(&p, &["commit", "-q", "-m", "init"]);
        (dir, p)
    }

    /// `a.txt` in a real merge conflict: base `hello`, `side` says `theirs`, current says `mine`.
    fn merge_conflict() -> (tempfile::TempDir, String) {
        let (dir, p) = init_repo();
        let base = run_git(&p, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim()
            .to_string();
        git(&p, &["checkout", "-q", "-b", "side"]);
        fs::write(format!("{}/a.txt", p), "theirs\n").unwrap();
        git(&p, &["commit", "-q", "-am", "theirs"]);
        git(&p, &["checkout", "-q", &base]);
        fs::write(format!("{}/a.txt", p), "mine\n").unwrap();
        git(&p, &["commit", "-q", "-am", "mine"]);
        let out = Proc::new("git")
            .arg("-C")
            .arg(&p)
            .args(["merge", "side"])
            .output()
            .unwrap();
        assert!(!out.status.success(), "the merge was supposed to conflict");
        (dir, p)
    }

    /// `git lfs version` / `git lfs ls-files` as they actually print.
    #[test]
    fn parses_lfs_output() {
        for &(text, want) in &[
            ("git-lfs/3.5.1 (GitHub; darwin arm64; go 1.21.3)\n", "3.5.1"),
            ("git-lfs/2.13.3 (Windows; amd64; go 1.20.6)\n", "2.13.3"),
            ("git-lfs/3.0.0-pre (GitHub)\n", "3.0.0"),
        ] {
            assert_eq!(parse_lfs_version(text).as_deref(), Some(want), "{:?}", text);
        }
        assert_eq!(parse_lfs_version("git: 'lfs' is not a git command"), None);
        assert_eq!(parse_lfs_version("git-lfs/"), None);

        let out = "4289fadb20139ffb5981f6ee57ec2e4e3a572840 - a.bin\n\
                   0f3f5b4c1d2e3a5b6c7d8e9f0a1b2c3d4e5f6a7b + assets/big file 1.png\n";
        assert_eq!(
            parse_lfs_paths(out),
            ["a.bin", "assets/big file 1.png"],
            "spaces in the path must survive"
        );
        // A line without the `<oid> <flag>` shape is noise, not a file.
        assert!(parse_lfs_paths("").is_empty());
        assert!(parse_lfs_paths("git: 'lfs' is not a git command.").is_empty());
        assert!(parse_lfs_paths("only-two tokens").is_empty());
    }

    #[test]
    fn lfs_info_never_fails_and_agrees_with_itself() {
        let (_d, p) = init_repo();
        let info = lfs_info(&p);
        assert_eq!(info.installed, !info.version.is_empty());
        // git-lfs is not necessarily on this machine, and this fixture repo
        // tracks nothing either way — the call must simply not fail.
        if !info.installed {
            assert!(info.files.is_empty());
        }
    }

    #[test]
    fn parses_git_version_out_of_the_real_outputs() {
        for &(text, want) in &[
            ("git version 2.50.1.windows.1\n", "2.50.1"),
            ("git version 2.39.3 (Apple Git-146)\n", "2.39.3"),
            ("git version 2.45.0\n", "2.45.0"),
            ("git version 2.47.0.rc0\n", "2.47.0"),
        ] {
            assert_eq!(parse_git_version(text).as_deref(), Some(want), "{:?}", text);
        }
        // Nothing usable to report: wrong text, no binary, or a bare major.
        for bad in [
            "",
            "not found",
            "git",
            "git version\n",
            "git version 2\n",
            "version 2.40.0",
        ] {
            assert_eq!(
                parse_git_version(bad),
                None,
                "read a version out of {:?}",
                bad
            );
        }
    }

    #[test]
    fn reports_the_git_that_is_actually_installed() {
        // The rest of these fixtures shell out to git, so on any machine that
        // runs them the version is real rather than None.
        let v = git_version().expect("git --version");
        assert!(v.split('.').count() >= 2, "{}", v);
    }

    #[test]
    fn detects_repo() {
        let (_d, p) = init_repo();
        assert!(is_repo(&p));
        assert!(!is_repo("/tmp"));
    }

    #[test]
    fn walks_up_from_dropped_path_to_the_repo_root() {
        let (d, p) = init_repo();
        let nested = d.path().join("src/deep");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("x.rs"), "fn main() {}\n").unwrap();
        // A dropped subfolder, a dropped file, and the repo dir itself all
        // resolve to the same root.
        assert_eq!(
            find_repo_root(nested.to_str().unwrap()).as_deref(),
            Some(p.as_str())
        );
        assert_eq!(
            find_repo_root(nested.join("x.rs").to_str().unwrap()).as_deref(),
            Some(p.as_str())
        );
        assert_eq!(find_repo_root(&p).as_deref(), Some(p.as_str()));
        // Nothing above looks like a repo → no answer.
        let plain = tempfile::tempdir().unwrap();
        let no_repo_above = plain.path().ancestors().all(|a| !a.join(".git").exists());
        if no_repo_above {
            assert_eq!(find_repo_root(plain.path().to_str().unwrap()), None);
        }
    }

    /// Folder tree for `scan_repos`: repos at three depths, plus the traps a
    /// naive walk would fall into. `.git` is only ever created, never used, so
    /// the fixture stays fast and needs no git binary.
    fn scan_fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let mk = |name: &str, git_as_file: bool| {
            let p = root.path().join(name);
            fs::create_dir_all(&p).unwrap();
            fs::write(p.join("a.txt"), "x\n").unwrap();
            let dot = p.join(".git");
            if git_as_file {
                fs::write(dot, "gitdir: /elsewhere\n").unwrap();
            } else {
                fs::create_dir_all(dot).unwrap();
            }
            p
        };
        mk("top", false);
        mk("group/app", false);
        mk("group/team/deep", true); // a worktree/submodule-style checkout
        fs::create_dir_all(root.path().join("plain/notes")).unwrap();
        mk(".hidden/repo", false); // inside a hidden directory
        mk("node_modules/dep", false); // an npm git dependency
        mk("top/inner", false); // a repo inside a repo
        root
    }

    fn rel(paths: &[String], root: &str) -> Vec<String> {
        paths
            .iter()
            .map(|p| {
                p.trim_start_matches(root)
                    .trim_start_matches('/')
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn scan_finds_repos_down_to_the_depth_limit() {
        let root = scan_fixture();
        let r = root.path().to_str().unwrap();
        assert_eq!(rel(&scan_repos(r, 1, 50).unwrap(), r), ["top"]);
        assert_eq!(rel(&scan_repos(r, 2, 50).unwrap(), r), ["group/app", "top"]);
        assert_eq!(
            rel(&scan_repos(r, 4, 50).unwrap(), r),
            ["group/app", "group/team/deep", "top"]
        );
    }

    #[test]
    fn scan_skips_hidden_dirs_node_modules_and_nested_repos() {
        let root = scan_fixture();
        let r = root.path().to_str().unwrap();
        let found = rel(&scan_repos(r, 8, 50).unwrap(), r);
        assert!(!found.iter().any(|p| p.contains(".hidden")), "{:?}", found);
        assert!(
            !found.iter().any(|p| p.contains("node_modules")),
            "{:?}",
            found
        );
        // `top` already matched, so `top/inner` is never reached or listed twice.
        assert!(!found.iter().any(|p| p.contains("inner")), "{:?}", found);
        assert!(!found.iter().any(|p| p.contains("plain")), "{:?}", found);
    }

    #[test]
    fn scan_stops_at_the_limit_and_accepts_the_root_itself() {
        let root = scan_fixture();
        let r = root.path().to_str().unwrap();
        assert_eq!(scan_repos(r, 8, 1).unwrap().len(), 1);
        assert_eq!(scan_repos(r, 8, 0).unwrap(), Vec::<String>::new());
        // Point it straight at a repo and that repo is the answer.
        let top = root.path().join("top");
        assert_eq!(scan_repos(top.to_str().unwrap(), 3, 50).unwrap().len(), 1);
    }

    #[test]
    fn scan_rejects_a_missing_folder() {
        let e = scan_repos("/definitely/not/here", 2, 10)
            .unwrap_err()
            .to_string();
        assert!(e.contains("文件夹不存在"), "{}", e);
    }

    #[cfg(unix)]
    #[test]
    fn scan_does_not_follow_symlinks() {
        let root = scan_fixture();
        let r = root.path().to_str().unwrap();
        // A link back to the parent would send a recursive walk around forever.
        std::os::unix::fs::symlink(root.path(), root.path().join("plain/self")).unwrap();
        let found = rel(&scan_repos(r, 8, 50).unwrap(), r);
        assert_eq!(found, ["group/app", "group/team/deep", "top"]);
    }

    #[test]
    fn status_reports_clean_then_modified() {
        let (_d, p) = init_repo();
        let st = status(&p).unwrap();
        assert!(st.branch == "master" || st.branch == "main");
        assert!(st.files.is_empty());

        fs::write(format!("{}/a.txt", p), "changed\n").unwrap();
        fs::write(format!("{}/b.txt", p), "new\n").unwrap();
        let st = status(&p).unwrap();
        assert_eq!(st.files.len(), 2);
        assert!(st
            .files
            .iter()
            .any(|f| f.path == "b.txt" && f.worktree_status == ChangeStatus::Untracked));
    }

    /// 记账替身的日志路径，进程级、只装一次、不回收。
    ///
    /// 最初写成「每个测试装一个、测完删掉临时目录」，并行跑时另一个测试正好 spawn 到
    /// 那个目录，`git` 凭空消失 —— 那是假失败，跟产品无关。装成进程级以后 PATH 不再回收，
    /// 替身只做「记一行、然后 exec 真 git」，别的测试经过它结果一模一样；日志每行带上
    /// 仓库路径，判据只数自己那一个仓库的行。
    static GIT_SHIM: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

    fn git_shim_log() -> &'static std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        GIT_SHIM.get_or_init(|| {
            let dir = std::env::temp_dir().join(format!("gitx-git-shim-{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let log = dir.join("calls.tsv");
            // 真 git 的绝对路径要在此刻 PATH 还没改时扫出来，写死进替身里。
            let real = std::env::split_paths(&std::env::var_os("PATH").unwrap())
                .map(|d| d.join("git"))
                .find(|p| p.is_file())
                .expect("找不到真 git");
            // 只记短的那几截：整条 `$*` 可能超过一次 append 的原子长度，并行写会互相截断，
            // 数出来的行数偏小 —— 判据就会因为巧合而变松。
            // 每次调用记两行（开始 B / 结束 E），并发判据靠这两行的**重叠深度**来证
            // 「真的同时在跑多个 git」；只看墙钟时间是不可重复的判据。
            // 所以这里不能用 `exec` —— 换掉进程就没机会写 E 行了。退出码要原样传回去，
            // 否则替身把 git 的失败吞成 0，`run_git` 的错误分支判据会跟着变绿。
            // `sleep` 只在 `GITX_SHIM_SLEEP` 被设时发生：不然全量套件的每一次 git fork 都要
            // 多等一档，几百次下来会白添几十秒。`GITX_SHIM_SLOW_REPO` 只拖这一个仓库的路径，
            // 流式判据要的就是「一个仓库还在飞、另一个已经交出来了」这个可观察的瞬间。
            let script = format!(
                "#!/bin/sh\nprintf 'B\\t%s\\t%s\\n' \"$2\" \"$5\" >> '{}'\nif [ -n \"$GITX_SHIM_SLEEP\" ]; then sleep 0.06; fi\nif [ -n \"$GITX_SHIM_SLOW_REPO\" ] && [ \"$2\" = \"$GITX_SHIM_SLOW_REPO\" ]; then sleep 0.3; fi\n'{}' \"$@\"\nrc=$?\nprintf 'E\\t%s\\t%s\\n' \"$2\" \"$5\" >> '{}'\nexit $rc\n",
                log.display(),
                real.display(),
                log.display()
            );
            let shim = dir.join("git");
            fs::write(&shim, script).unwrap();
            fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
            let mut path = std::ffi::OsString::from(dir.to_str().unwrap());
            path.push(":");
            path.push(std::env::var_os("PATH").unwrap());
            std::env::set_var("PATH", path);
            fs::write(&log, "").unwrap();
            log
        })
    }

    /// 此刻这个仓库还在飞几个 git（B 减 E）。流式判据用它来证「第一个结果到手的时候，
    /// 别的仓库确实还没读完」——这是一个瞬间状态，不是墙钟时间。
    fn shim_inflight(repo: &str) -> isize {
        fs::read_to_string(git_shim_log())
            .unwrap_or_default()
            .lines()
            .filter(|l| l.split('\t').nth(1) == Some(repo))
            .map(|l| match l.split('\t').next() {
                Some("B") => 1,
                Some("E") => -1,
                _ => 0,
            })
            .sum()
    }

    /// 让替身只拖慢这一个仓库（其余照常快跑），好制造「一个还在飞、一个已交出来」的瞬间。
    struct ShimSlowRepo {
        repo: String,
    }
    impl Drop for ShimSlowRepo {
        fn drop(&mut self) {
            if std::env::var_os("GITX_SHIM_SLOW_REPO").as_deref()
                == Some(std::ffi::OsStr::new(&self.repo))
            {
                std::env::remove_var("GITX_SHIM_SLOW_REPO");
            }
        }
    }

    impl ShimSlowRepo {
        fn start(repo: &str) -> ShimSlowRepo {
            std::env::set_var("GITX_SHIM_SLOW_REPO", repo);
            ShimSlowRepo { repo: repo.to_string() }
        }
    }

    /// `repo` 这个仓库到目前为止被 fork 了几次 git（只数开始行，一次调用一行）。
    fn shim_calls(repo: &str) -> usize {
        fs::read_to_string(git_shim_log())
            .unwrap_or_default()
            .lines()
            .filter(|l| {
                let mut f = l.split('\t');
                f.next() == Some("B") && f.next() == Some(repo)
            })
            .count()
    }

    /// 这批仓库**同时**在飞的最大 git 进程数：按日志出现顺序走一遍，B 加一、E 减一，
    /// 取峰值。深度 ≥2 就是「两条 B 之间没有 E」的证据，也就是真并发 ——
    /// 串行实现无论仓库多少个，深度永远是 1。
    fn shim_max_inflight(repos: &[&str]) -> usize {
        let text = fs::read_to_string(git_shim_log()).unwrap_or_default();
        let (mut depth, mut peak) = (0isize, 0usize);
        for line in text.lines() {
            let mut f = line.split('\t');
            let (tag, repo) = (f.next(), f.next());
            if repo.is_some_and(|r| repos.contains(&r)) {
                match tag {
                    Some("B") => {
                        depth += 1;
                        peak = peak.max(depth as usize);
                    }
                    Some("E") => depth -= 1,
                    _ => {}
                }
            }
        }
        peak
    }

    /// 首屏要给每个仓库出角标，稳态下只允许一次 fork：`status --porcelain=v2 --branch`
    /// 自己就带分支、上游、ahead/behind（`# branch.ab`）和脏文件数，剩下的
    /// 贮藏条数与进行中操作都能从 `.git` 里的文件读出来。多出来的一次就是 200 个仓库
    /// 多出来的 200 个进程。
    #[test]
    fn status_is_one_git_fork_per_repo() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/a.txt", p), "changed\n").unwrap();
        fs::write(format!("{}/s.txt", p), "stash me\n").unwrap();
        git(&p, &["add", "s.txt"]);
        // 只卷走 s.txt：不带 pathspec 的 `git stash` 会把 a.txt 的改动一起收进去，
        // 那样「脏文件数还得对」这条就没东西可对了。
        git(&p, &["stash", "push", "-q", "-m", "keep", "--", "s.txt"]);
        let before = shim_calls(&p);
        let st = status(&p).unwrap();
        assert_eq!(shim_calls(&p) - before, 1, "status 只该 fork 一次 git");
        // 省下来的两次 fork 不能把信息一起省掉 —— 这两条才是「文件读」的正确性判据。
        assert_eq!(st.stash_count, 1);
        assert_eq!(st.repo_state, "none");
        assert_eq!(st.files.len(), 1, "脏文件数还得对");
    }

    /// 读文件省下的那一次 fork，不能把定位 git-dir 读错。三种形态都对得上
    /// `rev-parse --absolute-git-dir`：普通仓库、linked worktree（`.git` 是指针，
    /// 而且指针指向的是**这个 worktree 自己**那份 git-dir，`MERGE_HEAD` 就在那儿）、
    /// 以及根本没有 `.git` 的裸仓库（走回退，仍然问 git）。
    #[test]
    fn git_dir_matches_git_for_every_layout() {
        let (d, p) = init_repo();
        assert_eq!(git_dir(&p).unwrap(), std::path::Path::new(&p).join(".git"));

        git(&p, &["branch", "side"]);
        let wt = d.path().join("work");
        git(&p, &["worktree", "add", "-q", wt.to_str().unwrap(), "side"]);
        let wtp = wt.to_str().unwrap();
        assert_eq!(
            git_dir(wtp).unwrap(),
            std::path::PathBuf::from(
                run_git(wtp, &["rev-parse", "--absolute-git-dir"])
                    .unwrap()
                    .trim()
            ),
            "worktree 的 .git 指针必须解到它自己那份 git-dir"
        );

        // 裸仓库没有 `.git`，回退到问 git —— 慢一次，但读错目录比慢一次严重。
        let bare = d.path().join("bare.git");
        git(&p, &["clone", "-q", "--bare", p.as_str(), bare.to_str().unwrap()]);
        let bp = bare.to_str().unwrap();
        assert_eq!(
            git_dir(bp).unwrap(),
            std::path::PathBuf::from(run_git(bp, &["rev-parse", "--absolute-git-dir"]).unwrap().trim())
        );
    }

    /// `.git` 指针**允许**写成相对路径（相对那个工作树），而 `git worktree add` 平时
    /// 写的是绝对路径 —— 不手工把它改成相对的，「相对就按仓库目录解析」那一支就是
    /// 没有主的死代码：改成按进程 cwd 解析，全模块 65 条一条都不会红。
    ///
    /// 断言不问「路径字符串相不相等」（相对的解出来带 `..`，同目录不同写法，比了没意义），
    /// 问它真正影响的那件事：这个 worktree 自己那份 git-dir 里的 `MERGE_HEAD` 读得到，
    /// 而主仓库的 sentinel 不受牵连。
    #[test]
    fn a_relative_gitdir_pointer_still_finds_its_own_merge_head() {
        let (d, p) = init_repo();
        git(&p, &["branch", "side"]);
        let wt = d.path().join("work");
        git(&p, &["worktree", "add", "-q", wt.to_str().unwrap(), "side"]);
        let wtp = wt.to_str().unwrap();
        fs::write(wt.join(".git"), "gitdir: ../.git/worktrees/work\n").unwrap();

        // 夹具的诚实性检查：真 git 也得认这个相对写法，否则我是在拿一个 git
        // 不会产出的形状测自己的解析。两边都 canonicalize —— 我解出来的路径带着 `..`，
        // 写法不同但指同一个目录，比字符串会比出一个假失败。
        assert_eq!(
            std::path::Path::new(
                run_git(wtp, &["rev-parse", "--absolute-git-dir"])
                    .unwrap()
                    .trim()
            )
            .canonicalize()
            .unwrap(),
            std::path::Path::new(&p)
                .join(".git/worktrees/work")
                .canonicalize()
                .unwrap()
        );

        // 让 worktree 里起一次真冲突：两边把 a.txt 改成不同的内容再 merge。
        fs::write(wt.join("a.txt"), "theirs\n").unwrap();
        git(wtp, &["commit", "-q", "-am", "theirs"]);
        fs::write(format!("{}/a.txt", p), "mine\n").unwrap();
        git(&p, &["commit", "-q", "-am", "mine"]);
        // merge 的是**主仓库那条分支**：worktree 自己就在 `side` 上，跟自己 merge 会
        // 直接 "Already up to date" 成功，夹具里根本不会留下 MERGE_HEAD。
        let base = run_git(&p, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim()
            .to_string();
        let out = Proc::new("git")
            .arg("-C")
            .arg(wtp)
            .args(["merge", &base])
            .output()
            .unwrap();
        assert!(!out.status.success(), "这次合并本该冲突");
        // git 在 merge 冲突时留下 MERGE_HEAD，正是首屏要报「合并进行中」的那个信号。

        assert_eq!(state_at(&git_dir(wtp).unwrap()), "merge", "worktree 自己的进行中操作");
        assert_eq!(
            state_at(&git_dir(&p).unwrap()),
            "none",
            "主仓库不该跟着报进行中 —— 报错了会指着一处没有冲突的地方让用户去解决"
        );
        assert_eq!(status(wtp).unwrap().repo_state, "merge");
    }

    /// `stash_count` 读的是 reflog 的行数，而界面显示的是 `stash list`。这两个数一旦
    /// 分家，角标就会在用户眼前说谎。把 git 真会留下的每种状态都走一遍：攒 3 条、
    /// drop 到只剩 1 条、pop、apply、gc、以及 `refs/stash` 被压进 packed-refs。
    #[test]
    fn stash_count_matches_stash_list() {
        let (_d, p) = init_repo();
        let check = |n: u32| {
            let st = status(&p).unwrap();
            assert_eq!(st.stash_count, n, "status 给的条数");
            assert_eq!(
                st.stash_count,
                stash_list(&p).unwrap().len() as u32,
                "和 stash list 必须一致"
            );
        };
        check(0);
        for i in 0..3 {
            fs::write(format!("{}/a.txt", p), format!("v{i}\n")).unwrap();
            git(&p, &["add", "a.txt"]);
            git(&p, &["stash", "-q", "-m", &format!("s{i}")]);
        }
        check(3);
        git(&p, &["stash", "drop", "-q", "stash@{1}"]);
        check(2);
        git(&p, &["stash", "pop", "-q"]);
        check(1);
        // pop 会把内容落回工作树，`apply` 紧接着就撞车 —— 先清干净，让 apply 走在
        // 一个本来干净的仓库上（这才是它平时遇到的情形）。
        git(&p, &["checkout", "-q", "--", "a.txt"]);
        git(&p, &["stash", "apply", "-q"]);
        check(1);
        git(&p, &["gc", "-q", "--prune=now"]);
        check(1);
        git(&p, &["pack-refs", "--all"]);
        check(1);
        // 最后清空：reflog 文件随 `refs/stash` 一起消失，条数要归 0 而不是停在 1。
        fs::write(format!("{}/a.txt", p), "v9\n").unwrap();
        git(&p, &["add", "a.txt"]);
        git(&p, &["stash", "-q", "-m", "s9"]);
        check(2);
        git(&p, &["stash", "clear"]);
        let st = status(&p).unwrap();
        assert_eq!(st.stash_count, 0, "clear 之后角标必须是 0");
        assert!(!std::path::Path::new(&p)
            .join(".git/logs/refs/stash")
            .exists());
    }

    /// 在指定日期落一枚提交（改 `a.txt` 保证每枚都有内容差异）。
    /// 时间窗的判据必须能造「窗口外」的提交，否则它测的是巧合不是规则。
    fn commit_at(repo: &str, date: &str, name: &str, email: &str, msg: &str) {
        let out = Proc::new("git")
            .arg("-C")
            .arg(repo)
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .env("GIT_AUTHOR_NAME", name)
            .env("GIT_AUTHOR_EMAIL", email)
            .env("GIT_COMMITTER_NAME", name)
            .env("GIT_COMMITTER_EMAIL", email)
            .args(["commit", "-q", "--allow-empty", "-m", msg])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "commit {:?} 失败了: {}",
            msg,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// 身份集的取数规则。`--get-regexp '^user\.'` 实测出来的形状是**全局 config 和本地
    /// config 混在一起**、一个键还能好几行（`git config --add user.email`），那就是
    /// 「一个人多身份」在真机器上的样子。名字一律不参与匹配 —— 同名太常见，拿 `dev`
    /// 这种名字匹配会把别人的提交算到我头上。
    #[test]
    fn parse_identities_keeps_emails_only_and_dedups() {
        let out = "user.name baorong\nuser.email SuperLBR@126.com\nuser.name 张三\n\
                   user.email a@x.com\nuser.email a@x.com\n";
        assert_eq!(
            parse_identities(out),
            ["superlbr@126.com", "a@x.com"],
            "只留邮箱、统一小写、去重、保持出现顺序"
        );
        assert!(parse_identities("").is_empty(), "没配身份是空集，不是报错");
        // 别的 user.* 键不能混进来（值里带 @ 的 signingkey 尤其危险）。
        assert!(
            parse_identities("user.signingkey 44321FA96209A48 \nuser.helper /usr/bin/gpg\n")
                .is_empty()
        );
    }

    /// 摘要里那几个角标 + 时间窗内的我的提交。窗口必须真挡住旧提交：夹具里造一条
    /// 30 天前、同样属于我的提交，它要是出现在结果里，说明「7 天」这条规则没落地。
    #[test]
    fn activity_reports_badges_and_windowed_mine() {
        let (_d, p) = init_repo();
        commit_at(&p, "2026-09-20T10:00:00+08:00", "tester", "me@x.com", "很久以前我的");
        commit_at(&p, "2026-10-20T10:00:00+08:00", "tester", "me@x.com", "我的第二新");
        commit_at(&p, "2026-10-21T10:00:00+08:00", "别人", "other@x.com", "不是我的");
        commit_at(&p, "2026-10-22T10:00:00+08:00", "tester", "me@x.com", "我的最新");

        fs::write(format!("{}/a.txt", p), "dirty\n").unwrap();
        fs::write(format!("{}/n.txt", p), "new\n").unwrap();
        git(&p, &["add", "n.txt"]);
        // 只卷走 n.txt：不带 pathspec 的 `git stash` 会把 a.txt 也收走，
        // 那时 `dirty` 本该是 0，这条判据就白写了。
        git(&p, &["stash", "push", "-q", "-m", "keep", "--", "n.txt"]);

        let a = recent_activity(&p, &["me@x.com".to_string()], "2026-10-18T00:00:00+08:00", 5)
            .unwrap();
        assert!(a.branch == "master" || a.branch == "main");
        assert_eq!(a.stash, 1);
        assert_eq!(a.dirty, 1, "窗口里只剩 a.txt 一处改动");
        assert!(a.op.is_none(), "没有进行中操作时不能凭空给一个");
        let subjects: Vec<&str> = a.mine.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(subjects, ["我的最新", "我的第二新"], "新的在前、只算我的、窗口外的不许进来");
        assert_eq!(a.ahead, 0);
        assert_eq!(a.behind, 0);
    }

    /// `max` 是「活动流最多给几行」，不是 git 的取数上限：先按窗口取、本地按身份筛、
    /// 再截断。反过来写（`--max-count=max`）会在过滤前就截掉，忙仓库里我的提交会整批消失。
    ///
    /// 夹具必须让**最新的那几条是别人提交的**：先前只排到「我的2」在最顶，
    /// `--max-count=1` 先截也照样拿到它，这条判据对着「先截断后过滤」的错实现全绿过。
    #[test]
    fn mine_log_is_filtered_before_truncated() {
        let (_d, p) = init_repo();
        commit_at(&p, "2026-10-20T10:00:00+08:00", "别人", "other@x.com", "别人1");
        commit_at(&p, "2026-10-21T10:00:00+08:00", "tester", "me@x.com", "我的1");
        commit_at(&p, "2026-10-22T10:00:00+08:00", "别人", "other@x.com", "别人2");
        commit_at(&p, "2026-10-23T10:00:00+08:00", "tester", "me@x.com", "我的2");
        commit_at(&p, "2026-10-24T10:00:00+08:00", "别人", "other@x.com", "最新是别人的");
        commit_at(&p, "2026-10-25T10:00:00+08:00", "别人", "other@x.com", "别人的第二条");
        let a = recent_activity(&p, &["me@x.com".to_string()], "2026-10-18T00:00:00+08:00", 1)
            .unwrap();
        assert_eq!(a.mine.len(), 1, "上限要生效");
        assert_eq!(a.mine[0].subject, "我的2", "截的是最旧的那条，不是截掉我的最新");
    }

    /// 匹配只认邮箱，四条边都得钉：同名不同邮箱不算我的、不同名同邮箱算我的、
    /// **两边各自的大小写**都要归一。少钉一边就是「一个值两条出口只验一半」那个坑 ——
    /// 先前只测了传入 alias 的大小写，把提交侧的 `to_lowercase` 删掉照样全绿。
    #[test]
    fn identity_matching_is_by_email_not_by_name() {
        let (_d, p) = init_repo();
        commit_at(&p, "2026-10-20T10:00:00+08:00", "tester", "other@x.com", "同名不是我的");
        commit_at(&p, "2026-10-21T10:00:00+08:00", "张三", "me@x.com", "换了署名但邮箱是我的");
        commit_at(&p, "2026-10-22T10:00:00+08:00", "张三", "ME@X.COM", "提交里邮箱是大写的");
        let a = recent_activity(&p, &["me@x.com".to_string()], "2026-10-18T00:00:00+08:00", 5)
            .unwrap();
        let subjects: Vec<&str> = a.mine.iter().map(|c| c.subject.as_str()).collect();
        assert_eq!(
            subjects,
            ["提交里邮箱是大写的", "换了署名但邮箱是我的"],
            "新的在前；同名不同邮箱那条不许进来"
        );
        // 另一边的写法：身份集给成大写，结果必须一模一样。
        let a = recent_activity(&p, &["ME@X.COM".to_string()], "2026-10-18T00:00:00+08:00", 5)
            .unwrap();
        assert_eq!(a.mine.len(), 2, "邮箱比对不能区分大小写");
    }

    /// 变基停在半路时要把「第几步 / 共几步」报出来（读 `msgnum`/`end`），
    /// 而合并、遴选给不出步数 —— 报成 `Some(0)/Some(0)` 就是画一根永远不动的进度条。
    #[test]
    fn op_reports_rebase_progress_and_leaves_merge_blank() {
        // 分叉后拿两条提交去 rebase 一条冲突的提交，git 会停在 rebase-merge 目录上。
        let (_d, p) = init_repo();
        let base = run_git(&p, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim()
            .to_string();
        // work 上两枚要重放的提交：第一枚改 a.txt（和主线撞），第二枚改 x.txt。
        git(&p, &["checkout", "-q", "-b", "work"]);
        fs::write(format!("{}/a.txt", p), "work1\n").unwrap();
        git(&p, &["add", "-A"]);
        commit_at(&p, "2026-10-20T10:00:00+08:00", "tester", "me@x.com", "第一步");
        fs::write(format!("{}/x.txt", p), "w2\n").unwrap();
        git(&p, &["add", "-A"]);
        commit_at(&p, "2026-10-21T10:00:00+08:00", "tester", "me@x.com", "第二步");
        // 主线把同一个文件改成别的内容，然后**回到 work** 再变基 —— `git rebase <onto>`
        // 重放的是当前分支的提交，站错分支这一停就停在别的地方，`end` 也就不是 2。
        git(&p, &["checkout", "-q", &base]);
        fs::write(format!("{}/a.txt", p), "main\n").unwrap();
        git(&p, &["commit", "-q", "-am", "主线改了同一个文件"]);
        git(&p, &["checkout", "-q", "work"]);
        let out = Proc::new("git")
            .arg("-C")
            .arg(&p)
            .args(["rebase", &base])
            .output()
            .unwrap();
        assert!(!out.status.success(), "这次变基本该停在冲突上");

        let a = recent_activity(&p, &[], "2026-01-01T00:00:00+08:00", 5).unwrap();
        let op = a.op.expect("变基停住了，摘要必须报进行中");
        assert_eq!(op.kind, "rebase");
        assert_eq!(
            op.total,
            Some(2),
            "total 来自 .git/rebase-merge/end，实测就是被重放的提交数"
        );
        assert!(op.step.is_some_and(|s| s >= 1), "step 来自 msgnum，停在第几步就是几");

        // 合并没有步数这个概念。
        let (_d2, m) = merge_conflict();
        let a = recent_activity(&m, &[], "2026-01-01T00:00:00+08:00", 5).unwrap();
        let op = a.op.expect("冲突合并该报进行中");
        assert_eq!((op.kind.as_str(), op.step, op.total), ("merge", None, None));
    }

    /// 变基目录在、但计数读不到（git 写到一半被打断就是这个形状）：进度只能报「进行中」，
    /// 不能报成「0 / 5」—— 一根停在 0 的进度条比没有进度条更假。
    #[test]
    fn a_missing_rebase_counter_stays_blank_instead_of_zero() {
        let (_d, p) = init_repo();
        let dir = std::path::Path::new(&p).join(".git/rebase-merge");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("end"), "5\n").unwrap();
        let op = recent_activity(&p, &[], "2026-01-01T00:00:00+08:00", 5)
            .unwrap()
            .op
            .expect("rebase-merge 目录在就该报变基进行中");
        assert_eq!((op.kind.as_str(), op.total), ("rebase", Some(5)));
        assert_eq!(op.step, None, "读不到就是读不到，不许编一个 0");
        // 数字写歪了同理：`0x3` 不是步数。
        fs::write(dir.join("msgnum"), "0x3\n").unwrap();
        let op = recent_activity(&p, &[], "2026-01-01T00:00:00+08:00", 5)
            .unwrap()
            .op
            .unwrap();
        assert_eq!(op.step, None, "非数字的计数一样只能回 None");
    }

    /// 成本判据：一次 status + 一次 log，**两次 fork**，身份问 config 不算在里面
    /// （由调用方缓存）。把 `identities()` 挪进来就多一次，200 个仓库多 200 个进程。
    #[test]
    fn recent_activity_is_two_forks_per_repo() {
        let (_d, p) = init_repo();
        commit_at(&p, "2026-10-20T10:00:00+08:00", "tester", "me@x.com", "我的");
        let before = shim_calls(&p);
        let a = recent_activity(&p, &["me@x.com".to_string()], "2026-10-18T00:00:00+08:00", 5)
            .unwrap();
        assert_eq!(shim_calls(&p) - before, 2, "一次 status + 一次 log");
        assert_eq!(a.mine.len(), 1, "fork 数省下来了还得真拿到东西");
    }

    /// 让替身在「写下 B」和「交给真 git」之间睡 60ms，把在飞的窗口撑到日志看得出来。
    /// 环境变量是进程级的，所以这个窗口里**别的测试**的 git 也跟着慢 60ms —— 只会变慢，
    /// 不会算错（判据只数自己仓库的行）。测完必须撤掉，不然全量套件白添几十秒。
    struct ShimSleep;
    impl Drop for ShimSleep {
        fn drop(&mut self) {
            std::env::remove_var("GITX_SHIM_SLEEP");
        }
    }

    impl ShimSleep {
        fn start() -> ShimSleep {
            std::env::set_var("GITX_SHIM_SLEEP", "1");
            ShimSleep
        }
    }

    /// 把仓库列表变成待查项，身份统一按「本机就这一个邮箱」填 —— 聚合的判据只关心
    /// 次数、顺序、并发和错误隔离，身份匹配本身另有专门的判据。
    fn queries(repos: &[String]) -> Vec<RepoQuery> {
        repos
            .iter()
            .map(|r| RepoQuery {
                repo: r.clone(),
                emails: vec!["me@x.com".to_string()],
            })
            .collect()
    }

    /// 聚合的成本判据：N 个仓库 = 2N 次 fork，**不多不少**。
    /// 「每批一次」那种优化（把 log 合成一条跨仓库查询）在这里根本不成立，
    /// 所以这条钉的是上限，不是下限。
    #[test]
    fn recent_activity_all_is_two_forks_per_repo() {
        let (_d1, a) = init_repo();
        let (_d2, b) = init_repo();
        commit_at(&a, "2026-10-20T10:00:00+08:00", "tester", "me@x.com", "甲");
        commit_at(&b, "2026-10-20T10:00:00+08:00", "tester", "me@x.com", "乙");
        let repos = vec![a.clone(), b.clone()];
        let before: Vec<usize> = repos.iter().map(|r| shim_calls(r)).collect();
        let out = recent_activity_all(&queries(&repos), "2026-10-18T00:00:00+08:00", 5, 4);
        assert_eq!(out.len(), 2);
        for (i, r) in repos.iter().enumerate() {
            assert_eq!(shim_calls(r) - before[i], 2, "{} 只该两次 fork", r);
        }
        // fork 省下来了还得真拿到东西：两行各属于自己的仓库。
        assert_eq!(out[0].result.as_ref().unwrap().mine[0].subject, "甲");
        assert_eq!(out[1].result.as_ref().unwrap().mine[0].subject, "乙");
    }

    /// 并发判据钉的是**重叠深度**，不是「墙钟时间变短」——后者在 CI 上是掷硬币。
    /// 深度 ≥2 的意思很具体：日志里存在两条 B，中间没有对应的 E，也就是同一时刻
    /// 确有两个 git 在跑。把它改成串行 `for`，每个仓库自己 B E B E 走完，峰值永远是 1。
    #[test]
    fn recent_activity_all_overlaps_its_git_forks() {
        let (_d1, a) = init_repo();
        let (_d2, b) = init_repo();
        let (_d3, c) = init_repo();
        let repos = vec![a.clone(), b.clone(), c.clone()];
        let refs: Vec<&str> = repos.iter().map(String::as_str).collect();
        let _sleep = ShimSleep::start();
        let out = recent_activity_all(&queries(&repos), "2026-01-01T00:00:00+08:00", 5, 3);
        drop(_sleep);
        assert!(
            shim_max_inflight(&refs) >= 2,
            "三个仓库、三个 worker，git 却一个接一个在跑 —— 这就是串行"
        );
        assert!(out.iter().all(|o| o.result.is_ok()));
    }

    /// 顺序 + 错误隔离：一个仓库的路径已经不存在（挪走了、外接盘拔了），它那一行是
    /// `Err` 且带得出仓库名，其余照常，返回顺序仍等于输入顺序。
    /// 首屏按「最近打开」排，顺序反了面板就会把上周的仓库顶到第一行。
    #[test]
    fn recent_activity_all_keeps_input_order_and_isolates_a_dead_repo() {
        let (_d1, a) = init_repo();
        let (_d2, c) = init_repo();
        let dead = {
            let d = tempfile::tempdir().unwrap();
            let p = d.path().to_str().unwrap().to_string();
            git(&p, &["init", "-q"]);
            drop(d); // 目录一删，这个路径就再也读不出来
            p
        };
        let repos = vec![a.clone(), dead.clone(), c.clone()];
        let out = recent_activity_all(&queries(&repos), "2026-01-01T00:00:00+08:00", 5, 3);
        assert_eq!(out.len(), 3);
        for (i, r) in repos.iter().enumerate() {
            assert_eq!(out[i].repo, *r, "第 {} 行的仓库名要对得上输入", i);
        }
        assert!(out[0].result.is_ok());
        assert!(out[1].result.is_err(), "读不了的仓库只能红它自己那一行");
        assert!(out[2].result.is_ok(), "后面的仓库不能被它拖累");
    }

    /// 一个仓库都没添、以及 `workers` 被传成 0：都不许 panic，也不能凭空产出行。
    /// 首屏那两个状态都会真的出现（空库列表、设置里把并发数拉到 0）。
    #[test]
    fn recent_activity_all_tolerates_empty_input_and_zero_workers() {
        assert!(recent_activity_all(&[], "2026-01-01T00:00:00+08:00", 5, 4).is_empty());
        let (_d, p) = init_repo();
        let one = vec![p.clone()];
        let before = shim_calls(&p);
        let out = recent_activity_all(&queries(&one), "2026-01-01T00:00:00+08:00", 5, 0);
        assert_eq!(out.len(), 1);
        assert!(out[0].result.is_ok());
        assert_eq!(shim_calls(&p) - before, 2);
    }

    /// 流式聚合的判据钉的是**过程**：第一个结果到手的那一刻，慢仓库还得「在飞」
    /// （它的 B 已写、E 未写）。这是一个瞬间状态，可在日志里数出来，不是「早了 30ms」那种
    /// 掷硬币。把机器改成「全部跑完再一次性交」，收到第一条时慢仓库早就结束了 → 红。
    /// 夹具：慢仓库排在**输入的第一位**，快仓库第二位 —— 否则「先回的是快仓库」这条
    /// 会被「先交的就是先排的」巧合满足，判据杀不掉攒批。
    #[test]
    fn recent_activity_stream_delivers_each_repo_as_it_is_read() {
        // 替身是「第一次被用到」才装进 PATH 的。这条判据只读日志、不调 `shim_calls`，
        // 不先走一次就等于整场都在问真 git：环境变量设得再对也不会睡，日志也数不出东西 ——
        // 表现是「慢仓库反而先回来」，看着像产品有 bug，其实是夹具没接上。
        git_shim_log();
        let (_d1, fast) = init_repo();
        let (_d2, slow) = init_repo();
        let _slow = ShimSlowRepo::start(&slow);
        let items = queries(&[slow.clone(), fast.clone()]);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| recent_activity_stream(&items, "2026-01-01T00:00:00+08:00", 5, 2, &tx));
            let first = rx.recv().expect("总得交回一条");
            assert_eq!(first.repo, fast, "先交出来的该是那个没被拖慢的仓库");
            assert_eq!(
                shim_inflight(&slow),
                1,
                "快仓库都到手了，慢仓库却已经跑完 —— 这是攒到最后一起交"
            );
        });
        // 到这里 `scope` 已经把那个后台线程收干净（它撞上发送失败后自己退休），
        // 全程没有 panic —— panic 会由 `thread::scope` 原样抛到测试里。
    }

    /// 接收端不想要了（界面那批刷新被新的盖掉、窗口关了），worker 就把手上这一个仓库
    /// 干完然后退休，不再领新任务 —— 否则 200 个仓库的旧刷新会继续白占进程。
    ///
    /// 夹具：4 个仓库、2 个 worker、`rx` 一上来就丢。提前收摊时正好两个仓库被读到
    /// （各 2 次 fork），另两个一次都不该有；改成「发不出去也照样跑完」，四个仓库全被读 → 红。
    #[test]
    fn recent_activity_stream_stops_when_the_receiver_is_gone() {
        let (_d1, a) = init_repo();
        let (_d2, b) = init_repo();
        let (_d3, c) = init_repo();
        let (_d4, d) = init_repo();
        let repos = vec![a, b, c, d];
        let items = queries(&repos);
        let (tx, rx) = std::sync::mpsc::channel();
        drop(rx);
        let before: Vec<usize> = repos.iter().map(|r| shim_calls(r)).collect();
        recent_activity_stream(&items, "2026-01-01T00:00:00+08:00", 5, 2, &tx);
        let mut deltas: Vec<usize> = repos
            .iter()
            .enumerate()
            .map(|(i, r)| shim_calls(r) - before[i])
            .collect();
        deltas.sort();
        assert_eq!(deltas, vec![0, 0, 2, 2], "两个 worker 各干完手上一个仓库就该收摊");
    }

    #[test]
    fn stage_and_commit_flow() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/b.txt", p), "new\n").unwrap();
        stage(&p, &["b.txt".to_string()]).unwrap();
        let st = status(&p).unwrap();
        assert!(st.files.iter().any(|f| f.path == "b.txt" && f.staged));

        let head = commit(&p, "add b", false, &[], false, false).unwrap();
        assert_eq!(head.len(), 40);
        let commits = commits(&p, 10, None, None).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].subject, "add b");
        // 翻页：`--skip=1` 就是第二页，接在第一页后面、不重叠。
        let second = commits_page(&p, 1, 10, false, &Scope::Current).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].id, commits[1].id);
    }

    /// 一份补丁里有几块，只看行首的 `@@`。
    fn hunks(patch: &str) -> usize {
        patch.lines().filter(|l| l.starts_with("@@")).count()
    }

    /// 两个文件、三块改动，编号连着数（不按文件重置，和差异区那句「块 N」一致）。
    fn three_block_patch() -> String {
        [
            "diff --git a/one.txt b/one.txt",
            "index 1111111..2222222 100644",
            "--- a/one.txt",
            "+++ b/one.txt",
            "@@ -1,3 +1,3 @@",
            " ctx",
            "-old",
            "+new",
            " ctx",
            "@@ -20,3 +20,3 @@",
            " ctx",
            "-far",
            "+near",
            " ctx",
            "diff --git a/two.txt b/two.txt",
            "index 3333333..4444444 100644",
            "--- a/two.txt",
            "+++ b/two.txt",
            "@@ -1,2 +1,2 @@",
            "-x",
            "+y",
        ]
        .join("\n")
            + "\n"
    }

    /// 「只暂存一块」这条路全靠这一步把一块切出来，切错了就是往索引里打错补丁。
    /// 断的是三件事：块数对得上、头跟着**自己那格**文件走、越界与缺头都得是 None。
    #[test]
    fn hunk_patch_takes_one_block_with_its_own_header() {
        let p = three_block_patch();
        let one = hunk_patch(&p, 1).unwrap();
        assert!(one.starts_with("diff --git a/one.txt"), "头不是第一格文件的：{one}");
        assert_eq!(hunks(&one), 1, "抠出来不止一块：{one}");
        assert!(one.contains("+new") && !one.contains("+near") && !one.contains("+y"), "混进了别块的内容：{one}");

        let two = hunk_patch(&p, 2).unwrap();
        assert!(two.contains("+near") && !two.contains("+new"));

        // 第 3 块在**第二个**文件里，头必须跟着换过去，否则 git 会把补丁打到 one.txt 上。
        let three = hunk_patch(&p, 3).unwrap();
        assert!(three.starts_with("diff --git a/two.txt"), "第三块带错了文件头：{three}");
        assert!(three.contains("+y") && !three.contains("+near"), "第三块里混进了上一格文件的内容：{three}");

        assert!(hunk_patch(&p, 4).is_none(), "越界的编号还给了补丁");
        assert!(hunk_patch("", 1).is_none());
        // 缺 `---`/`+++` 的半截补丁不能往外发：`git apply` 会当成路径错误的噪音。
        assert!(hunk_patch("diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n", 1).is_none());
    }

    /// 真仓库上端到端：改两处（隔得够远，git 会切成两块），只暂存第 1 块，
    /// 断的是「暂存区里正好一块、工作区里还剩一块」——两边都不许吞掉对方。
    #[test]
    fn stage_hunk_moves_exactly_that_block() {
        let (_d, p) = init_repo();
        let body: String = (1..=30).map(|i| format!("line {i}\n")).collect();
        fs::write(format!("{}/a.txt", p), &body).unwrap();
        git(&p, &["add", "a.txt"]);
        git(&p, &["commit", "-q", "-m", "三十行"]);
        let edited: String = (1..=30)
            .map(|i| match i {
                2 => "TWO\n".to_owned(),
                20 => "TWENTY\n".to_owned(),
                _ => format!("line {i}\n"),
            })
            .collect();
        fs::write(format!("{}/a.txt", p), &edited).unwrap();
        assert_eq!(hunks(&working_patch(&p, false, Some("a.txt")).unwrap()), 2, "夹具没造出两块，这条判据什么也没验");

        stage_hunk(&p, "a.txt", 1).unwrap();
        let staged = working_patch(&p, true, Some("a.txt")).unwrap();
        let rest = working_patch(&p, false, Some("a.txt")).unwrap();
        assert_eq!(hunks(&staged), 1, "暂存区里该只有一块：{staged}");
        assert!(staged.contains("+TWO") && !staged.contains("+TWENTY"), "暂存的那块不是第 1 块：{staged}");
        assert_eq!(hunks(&rest), 1, "工作区里该还剩一块：{rest}");
        assert!(rest.contains("+TWENTY"), "剩下的应该是第 2 块：{rest}");

        // 反过来打第 2 块的补丁（取消暂存那一步的机制），工作区内容不能被改动。
        unstage_hunk(&p, "a.txt", 1).unwrap();
        assert_eq!(hunks(&working_patch(&p, true, Some("a.txt")).unwrap()), 0, "取消暂存之后索引还没这块");
        assert_eq!(hunks(&working_patch(&p, false, Some("a.txt")).unwrap()), 2, "退回工作区时丢了一块");
        assert_eq!(
            fs::read_to_string(format!("{}/a.txt", p)).unwrap(),
            edited,
            "取消暂存动了工作区的文件内容"
        );
    }

    /// 已暂存的文件也能只退一块，另一块必须留在索引里。
    #[test]
    fn unstage_hunk_keeps_the_other_block_staged() {
        let (_d, p) = init_repo();
        let body: String = (1..=30).map(|i| format!("line {i}\n")).collect();
        fs::write(format!("{}/a.txt", p), &body).unwrap();
        git(&p, &["add", "a.txt"]);
        git(&p, &["commit", "-q", "-m", "三十行"]);
        let edited: String = (1..=30)
            .map(|i| match i {
                2 => "TWO\n".to_owned(),
                20 => "TWENTY\n".to_owned(),
                _ => format!("line {i}\n"),
            })
            .collect();
        fs::write(format!("{}/a.txt", p), &edited).unwrap();
        stage_all(&p).unwrap();
        assert_eq!(hunks(&working_patch(&p, true, Some("a.txt")).unwrap()), 2);

        unstage_hunk(&p, "a.txt", 1).unwrap();
        let staged = working_patch(&p, true, Some("a.txt")).unwrap();
        assert_eq!(hunks(&staged), 1, "退掉一块之后索引里该还剩一块：{staged}");
        assert!(staged.contains("+TWENTY") && !staged.contains("+TWO"), "退掉的不是第 1 块：{staged}");
    }

    /// 报不出块号时要说清是哪块，别让「第 9 块不存在」变成一句 git 噪音。
    #[test]
    fn a_missing_block_number_reports_the_block() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/a.txt", p), "hello\nchanged\n").unwrap();
        let err = stage_hunk(&p, "a.txt", 7).unwrap_err().to_string();
        assert!(err.contains("7"), "错误里没带上块号：{err}");
    }

    /// 刚 `git init` 出来、一条提交还没有的仓库：日志是**空的**，不是错误。
    /// 当成错误，「新建仓库」那一步开出来的本子永远读不出快照，界面停在转圈上。
    /// 反向的要一起钉：真错误（那不是仓库）必须还是错误——否则这条判据等于把
    /// `log_commits` 的 `Err` 全咽下去，改坏任何一类失败都不会红。
    #[test]
    fn a_repo_without_any_commits_has_an_empty_log() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_str().unwrap().to_string();
        git(&p, &["init", "-q", "-b", "main"]);
        git(&p, &["config", "user.email", "t@t.com"]);
        git(&p, &["config", "user.name", "tester"]);
        assert!(commits(&p, 100, None, None).unwrap().is_empty(), "空仓库该读出一份空的日志");
        assert!(
            commits(&dir.path().join("没在这儿").to_str().unwrap().to_owned(), 100, None, None).is_err(),
            "指向不存在的路径也当成了空日志"
        );
    }

    /// 「层级顺序」这个开关只该多一条 `--topo-order`，不许顺带改掉筛选条件——
    /// 否则切一次排序，用户看到的提交集合会静默变掉。
    #[test]
    fn topo_switch_only_adds_the_order_flag() {
        let plain = log_args(10, Some("nobody"), Some("src/a.rs"), false, 0, &Scope::Current);
        let topo = log_args(10, Some("nobody"), Some("src/a.rs"), true, 0, &Scope::Current);
        assert!(!plain.iter().any(|a| a == "--topo-order"), "日期序里冒出 topo 参数");
        assert_eq!(topo.iter().filter(|a| *a == "--topo-order").count(), 1);
        let without = |v: Vec<String>| {
            v.into_iter()
                .filter(|a| a != "--topo-order")
                .collect::<Vec<String>>()
        };
        assert_eq!(without(topo), plain, "切换排序顺带改了别的参数：{plain:?}");
    }

    /// 取下一页只该多一条 `--skip`，别顺带把筛选或排序改掉。第一页连 `--skip` 都不发——
    /// `git log --skip=0` 合法但白拼一个参数，也让「有没有下一页」的判断少了对照。
    #[test]
    fn the_page_offset_only_adds_skip() {
        let first = log_args(3, None, None, false, 0, &Scope::Current);
        let second = log_args(3, None, None, false, 3, &Scope::Current);
        assert!(!first.iter().any(|a| a.starts_with("--skip")), "第一页发了 --skip");
        assert_eq!(second.iter().filter(|a| a.starts_with("--skip")).count(), 1);
        assert!(second.contains(&"--skip=3".to_owned()));
        let without = second.into_iter().filter(|a| !a.starts_with("--skip")).collect::<Vec<String>>();
        assert_eq!(without, first, "翻页顺带改了别的参数：{first:?}");
    }

    /// 按分支筛日志 = 只多那一个**位置参数**。它必须落在 `--` **前面**：落到后面 git 会把它
    /// 当 pathspec，列表安静地回一个空——「筛了个啥都没有」看着像功能正常。
    /// 和 `topo_switch_only_adds_the_order_flag` 同一个理由：换筛选条件不许顺带改别的参数。
    #[test]
    fn the_rev_argument_only_adds_the_rev_before_the_pathspec() {
        let all = log_args(10, Some("nobody"), Some("src/a.rs"), false, 0, &Scope::Current);
        let one = log_args(10, Some("nobody"), Some("src/a.rs"), false, 0, &Scope::Rev("side".into()));
        assert_eq!(one.len(), all.len() + 1, "多出来的不止那一个参数：{one:?}");
        let without: Vec<String> = one.iter().filter(|a| **a != "side").cloned().collect();
        assert_eq!(without, all, "选分支顺带改了别的参数：{all:?}");
        let dash = one.iter().position(|a| a == "--").expect("带路径筛选时该有 `--`");
        let at = one.iter().position(|a| a == "side").unwrap();
        assert!(at < dash, "rev 落在 `--` 后面就被当成路径了：{one:?}");
        // 空白的那一位等于没选：不给 git 传一个空参数（它会报 ambiguous argument）。
        assert_eq!(
            log_args(10, None, None, false, 0, &Scope::Rev("  ".into())),
            log_args(10, None, None, false, 0, &Scope::Current)
        );
    }

    /// 「所有分支」那一档只多 `--all` 这一位，而且它占的正是 rev 那一格：两种范围**互斥**，
    /// 同一条 argv 里不该同时出现 `--all` 和一个位置参数（`git log --all side` 会把 side
    /// 之外的分支也算进去，用户在菜单上选的到底是哪一档就说不清了）。
    /// `--all` 同样必须在 `--` 前面——落到后面 git 当它是路径，列表安静地回空。
    #[test]
    fn the_all_scope_adds_only_dash_all_in_the_rev_slot() {
        let cur = log_args(10, Some("nobody"), Some("src/a.rs"), false, 0, &Scope::Current);
        let all = log_args(10, Some("nobody"), Some("src/a.rs"), false, 0, &Scope::All);
        assert_eq!(all.len(), cur.len() + 1, "多出来的不止 --all：{all:?}");
        assert_eq!(all.iter().filter(|a| *a == "--all").count(), 1, "--all 发了不止一次");
        let without: Vec<String> = all.iter().filter(|a| *a != "--all").cloned().collect();
        assert_eq!(without, cur, "开 --all 顺带改了别的参数：{cur:?}");
        let dash = all.iter().position(|a| a == "--").expect("带路径筛选时该有 `--`");
        let at = all.iter().position(|a| a == "--all").unwrap();
        assert!(at < dash, "--all 落在 `--` 后面就被当成路径了：{all:?}");
        // 默认那一档不许偷偷带 --all：`commits()` 和 `commits_on(HEAD)` 必须是同一份列表。
        assert!(!cur.iter().any(|a| a == "--all"), "当前分支那一档发了 --all");
    }

    /// 端到端：`commits_on("side")` 只走那条支线的祖先，主线独有的提交得不在列表里；
    /// 而 `commits_on(主线)` 反过来要有它。两个方向都判一次，防的是「rev 参数被吃掉、
    /// 列表原样返回」——那种写法在第一句里就会红。
    #[test]
    fn scoping_to_a_rev_walks_only_that_revs_ancestors() {
        let (_d, p) = init_repo();
        let base = status(&p).unwrap().branch;
        // 支线先从**初始提交**分出去，主线再往前走一步：这样「只在支线」「只在主线」
        // 各自只在自己的那条线上，两个方向的断言才有东西可判。
        git(&p, &["checkout", "-q", "-b", "side"]);
        fs::write(format!("{}/side.txt", p), "侧\n").unwrap();
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-q", "-m", "只在支线"]);
        git(&p, &["checkout", "-q", base.as_str()]);
        fs::write(format!("{}/main.txt", p), "一\n").unwrap();
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-q", "-m", "只在主线"]);

        let subjects = |l: Vec<Commit>| -> Vec<String> { l.iter().map(|c| c.subject.clone()).collect() };
        let side = subjects(commits_on(&p, "side", 50, false).unwrap());
        let main = subjects(commits_on(&p, base.as_str(), 50, false).unwrap());
        assert!(side.contains(&"只在支线".to_owned()) && !side.contains(&"只在主线".to_owned()), "支线列表串了：{side:?}");
        assert!(main.contains(&"只在主线".to_owned()) && !main.contains(&"只在支线".to_owned()), "主线列表串了：{main:?}");
        // 不传 rev 时 `git log` 走的本来就是 HEAD 那条线，所以「不筛」和「按当前分支筛」
        // 必须是同一份列表——这条钉住的是**默认那一位没被偷偷换成 `--all`**。
        assert_eq!(subjects(commits(&p, 50, None, None).unwrap()), main, "默认列表和当前分支的列表不一致");
        // 层级序走同一条参数：换排序不该让「按分支筛」筛出别的东西。
        assert_eq!(subjects(commits_on(&p, "side", 50, true).unwrap()), side, "topo 下支线列表变了");
    }

    /// 端到端：`Scope::All` 那条档必须真把**只有别的分支走得到**的提交取回来。
    /// 只钉 argv 不算——`--all` 发出去但解析时丢掉，argv 判据照样绿。
    /// 判据两边都问：`Current` 里没有「只在支线」，`All` 里两条都有；反过来「All 比 Current
    /// 多几条」单独成立，因为多出来的可能是别的仓库对象。
    #[test]
    fn the_all_scope_reaches_commits_only_a_side_branch_has() {
        let (_d, p) = init_repo();
        let base = status(&p).unwrap().branch;
        git(&p, &["checkout", "-q", "-b", "side"]);
        fs::write(format!("{}/side.txt", p), "侧\n").unwrap();
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-q", "-m", "只在支线"]);
        git(&p, &["checkout", "-q", base.as_str()]);
        fs::write(format!("{}/main.txt", p), "一\n").unwrap();
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-q", "-m", "只在主线"]);

        let subjects = |l: Vec<Commit>| -> Vec<String> { l.iter().map(|c| c.subject.clone()).collect() };
        let cur = subjects(commits_page(&p, 0, 50, false, &Scope::Current).unwrap());
        let all = subjects(commits_page(&p, 0, 50, false, &Scope::All).unwrap());
        assert!(cur.contains(&"只在主线".to_owned()) && !cur.contains(&"只在支线".to_owned()), "当前分支那档串了：{cur:?}");
        for want in ["只在主线", "只在支线"] {
            assert!(all.contains(&want.to_owned()), "所有分支那档没有「{want}」：{all:?}");
        }
        // 对照：仓库里可走得到的提交总数（所有 ref），`--all` 就该给出这些。
        let want = run_git(&p, &["rev-list", "--all", "--count"]).unwrap().trim().to_string();
        assert_eq!(all.len().to_string(), want, "--all 的条数和仓库里的对不上：{all:?}");
    }

    /// 图序列是一行一行往下排的，靠的是 git 那条保证：**父提交一定排在子提交下面**。
    /// 日期序和层级序都得守这条——守不住，画到合并那里竖线就得往上拐。
    /// 顺带钉住 `%P` 真的把两个父提交都解析出来了（少一个就是合并线画不出来）。
    #[test]
    fn every_parent_is_listed_below_its_child() {
        let (_d, p) = init_repo();
        let base = status(&p).unwrap().branch;
        fs::write(format!("{}/main.txt", p), "一\n").unwrap();
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-q", "-m", "main one"]);
        git(&p, &["checkout", "-q", "-b", "side"]);
        fs::write(format!("{}/side.txt", p), "侧\n").unwrap();
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-q", "-m", "side one"]);
        git(&p, &["checkout", "-q", base.as_str()]);
        fs::write(format!("{}/main.txt", p), "二\n").unwrap();
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-q", "-m", "main two"]);
        git(&p, &["merge", "--no-ff", "-q", "-m", "merge side", "side"]);

        for (name, list) in [
            ("日期序", commits(&p, 50, None, None).unwrap()),
            ("层级序", commits_topo(&p, 50, None, None).unwrap()),
        ] {
            assert!(
                list.iter().any(|c| c.parents.len() == 2),
                "{name}：这份仓里没造出合并提交，断言跑不出东西"
            );
            for (i, c) in list.iter().enumerate() {
                for pid in &c.parents {
                    match list.iter().position(|x| &x.id == pid) {
                        Some(row) => assert!(
                            row > i,
                            "{name}：第 {i} 行的父提交 {pid} 出现在第 {row} 行，图序列得往上拐",
                        ),
                        None => panic!("{name}：第 {i} 行的父提交 {pid} 不在列表里"),
                    }
                }
            }
        }
    }

    #[test]
    fn branch_create_and_list() {
        let (_d, p) = init_repo();
        create_branch(&p, "feature", None, true).unwrap();
        let br = branches(&p).unwrap();
        assert!(br.iter().any(|b| b.name == "feature" && b.is_current));
    }

    /// 重命名当前分支：ref 换了名字，提交一条不少，HEAD 指的还是同一个 oid。
    #[test]
    fn renaming_the_current_branch_keeps_its_commits_and_head() {
        let (_d, p) = init_repo();
        let old = run_git(&p, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap()
            .trim()
            .to_string();
        let head = run_git(&p, &["rev-parse", "HEAD"]).unwrap();
        rename_branch(&p, &old, "renamed").unwrap();

        let br = branches(&p).unwrap();
        assert!(br.iter().any(|b| b.name == "renamed" && b.is_current));
        assert!(!br.iter().any(|b| b.name == old), "旧名字还留在 refs 里");
        assert_eq!(
            run_git(&p, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap().trim(),
            "renamed"
        );
        assert_eq!(run_git(&p, &["rev-parse", "HEAD"]).unwrap(), head);
    }

    /// `origin/main` is the only thing that tells you where the other end is, and
    /// `branches()` cannot supply it — so the query needs its own evidence.
    #[test]
    fn remote_refs_are_the_fetched_remote_pointers() {
        let (_d, p) = init_repo();
        let remote = tempfile::tempdir().unwrap();
        let r = remote.path().to_str().unwrap();
        git(r, &["init", "-q", "--bare", "."]);

        // No remote configured at all: the history graph must show zero remote chips,
        // not a phantom one.
        assert!(remote_refs(&p).unwrap().is_empty());

        git(&p, &["push", "-q", r, "HEAD:refs/heads/main"]);
        git(&p, &["remote", "add", "origin", r]);
        // Adding the remote does not create the tracking ref; only fetching does.
        assert!(remote_refs(&p).unwrap().is_empty());
        git(&p, &["fetch", "-q", "origin"]);

        let refs = remote_refs(&p).unwrap();
        assert_eq!(refs.len(), 1, "{:?}", refs);
        assert_eq!(refs[0].name, "origin/main");
        assert!(
            !refs.iter().any(|r| r.name == "origin"),
            "`origin/HEAD` is a symbolic ref; `%(refname:short)` prints it as `origin` \
             and the filter misses it: {:?}",
            refs
        );
        assert_eq!(
            refs[0].target,
            head(&p).unwrap(),
            "the chip has to point at the commit the graph highlights"
        );

        // Same story when `origin/HEAD` is a plain ref (older git, or a manual
        // `update-ref`): neither shape belongs in the graph.
        let target = head(&p).unwrap();
        git(&p, &["update-ref", "refs/remotes/origin/HEAD", &target]);
        let refs = remote_refs(&p).unwrap();
        assert_eq!(refs.len(), 1, "origin/HEAD must be filtered: {:?}", refs);
        assert_eq!(refs[0].name, "origin/main");
    }

    #[test]
    fn diff_between_commits() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/a.txt", p), "hello\nworld\n").unwrap();
        stage(&p, &["a.txt".to_string()]).unwrap();
        commit(&p, "extend a", false, &[], false, false).unwrap();
        let diff = working_diff(&p, false).unwrap();
        assert!(diff.is_empty());
        let log = commits(&p, 2, None, None).unwrap();
        let parent = &log[1].id;
        let diff = range_diff(&p, parent, &log[0].id).unwrap();
        assert!(diff.iter().any(|d| d.path == "a.txt" && d.additions > 0));
    }

    /// 某条提交的补丁**只有补丁**：`git show` 默认在前面塞一段日志头
    /// （`commit <oid>` / `Author:` / `Date:` / 空行 / 缩进的提交说明），
    /// 差异区照原样画出来就是用户说的「commit、Author、Date、改动描述」。
    /// SourceTree 那几行一个都不画（见 `commit_patch` 里那条带子的量法）。
    #[test]
    fn commit_patch_carries_no_log_header() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/a.txt", p), "hello\nworld\n").unwrap();
        stage(&p, &["a.txt".to_string()]).unwrap();
        commit(&p, "extend a", false, &[], false, false).unwrap();
        let oid = head(&p).unwrap();
        let patch = commit_patch(&p, &oid, None).unwrap();
        let first = patch.lines().next().unwrap_or_default();
        assert!(first.starts_with("diff --git "), "补丁第一行是 {first:?}，说明日志头还在");
        for line in patch.lines() {
            for noise in ["commit ", "Author: ", "Date: ", "AuthorDate: ", "CommitDate: "] {
                assert!(!line.starts_with(noise), "日志头漏出来了：{line:?}");
            }
        }
        assert!(!patch.contains("extend a"), "提交说明（改动描述）还在补丁里：{patch}");
        assert!(patch.contains("+world\n"), "补丁本体不能跟着一起没了：{patch}");

        // 带路径过滤那一格同理：单文件差异也不许带日志头。
        let one = commit_patch(&p, &oid, Some("a.txt")).unwrap();
        assert!(one.starts_with("diff --git "), "带路径那格漏了日志头");
        assert!(!one.contains("extend a"));
    }

    #[test]
    fn stash_roundtrip() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/c.txt", p), "wip\n").unwrap();
        stash_push(&p, "wip work", true).unwrap();
        let list = stash_list(&p).unwrap();
        assert_eq!(list.len(), 1);
        // 侧栏只显示用户写的那句。`On main: ` 是 git 的包装，`stash@{0}: ` 是列表的包装。
        assert_eq!(list[0].message, "wip work", "贮藏说明还带着 git 那层 `On <分支>: ` 包装");
        assert_eq!(list[0].oid.len(), 40, "没带上贮藏的提交号，点开就取不到内容");
        stash_pop(&p, 0).unwrap();
        let list = stash_list(&p).unwrap();
        assert_eq!(list.len(), 0);
    }

    /// 命令行 `git stash`（不给 `-m`）存出来的是 `WIP on <分支>: <短oid> <提交标题>`。
    /// 剥掉包装之后剩下的仍是 git 自己那句兜底说明，不是空行。
    #[test]
    fn a_stash_without_a_message_falls_back_to_gits_own_wip_line() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/a.txt", p), "changed\n").unwrap();
        git(&p, &["stash", "push"]);
        let list = stash_list(&p).unwrap();
        assert!(
            !list[0].message.starts_with("WIP on "),
            "WIP 贮藏还带着 git 的包装：{:?}",
            list[0].message
        );
        assert!(list[0].message.contains("init"), "剥完连 git 那句兜底说明都没了：{:?}", list[0].message);
    }

    /// 点开一条贮藏要看的是**整条贮藏**：`-u` 存进去的未跟踪文件也在一块补丁里。
    #[test]
    fn a_stash_keeps_the_untracked_files_it_was_stored_with() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/a.txt", p), "changed\n").unwrap();
        fs::write(format!("{}/new.txt", p), "从没进过索引\n").unwrap();
        stash_push(&p, "带未跟踪", true).unwrap();
        let list = stash_list(&p).unwrap();
        let oid = &list[0].oid;

        let files = stash_diff(&p, oid).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert!(names.contains(&"new.txt"), "贮藏的改动列表里没有未跟踪文件：{names:?}");
        assert!(names.contains(&"a.txt"), "贮藏的改动列表里没有改过的 a.txt：{names:?}");

        let patch = stash_patch(&p, oid, None).unwrap();
        assert!(patch.starts_with("diff --git "), "贮藏补丁第一行不是 diff --git：{patch:?}");
        assert!(patch.contains("+从没进过索引"), "整份补丁里没有未跟踪文件的内容：{patch}");
        assert!(patch.contains("+changed"), "整份补丁里没有改过的内容：{patch}");

        let one = stash_patch(&p, oid, Some("new.txt")).unwrap();
        assert!(one.contains("new.txt") && !one.contains("a.txt"), "按路径过滤那份串到别的文件了：{one}");
    }

    /// 没带 `-u` 存出来的贮藏**没有第三父提交**：`^3` 会报 `bad revision`。
    /// 少了这一条，「未跟踪那半边取不到就忽略」会被写成「取不到就报错」，
    /// 而最常见的普通贮藏恰好没有第三父提交。
    #[test]
    fn a_stash_without_untracked_files_still_has_a_patch() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/a.txt", p), "changed\n").unwrap();
        stash_push(&p, "只有已跟踪", false).unwrap();
        let list = stash_list(&p).unwrap();
        let patch = stash_patch(&p, &list[0].oid, None).unwrap();
        assert!(patch.contains("+changed"), "普通贮藏的补丁读不出来：{patch}");
        assert_eq!(stash_diff(&p, &list[0].oid).unwrap().len(), 1, "普通贮藏该只有一个文件");
    }

    #[test]
    fn tags_list_and_create() {
        let (_d, p) = init_repo();
        create_tag(&p, "v1.0", "HEAD", Some("release")).unwrap();
        let tags = tags(&p).unwrap();
        assert!(tags.iter().any(|t| t.name == "v1.0" && t.annotated));
    }

    /// annotated 标签的 `target` 必须是**它指向的提交**，不是标签对象自己。
    /// 界面层拿 `target` 和 `Commit.id` 做相等判断来给那一行配药丸：不 peel 的话，
    /// 「打了标签的那一行」永远少一枚药丸，而且轻量标签有、annotated 没有，
    /// 看起来像是打了更正式的标签反而丢了信息。
    #[test]
    fn tag_targets_are_peeled_to_the_commit() {
        let (_d, p) = init_repo();
        let head = head(&p).unwrap();
        create_tag(&p, "v1.0", "HEAD", Some("release")).unwrap();
        create_tag(&p, "light", "HEAD", None).unwrap();
        let tags = tags(&p).unwrap();
        let find = |n: &str| tags.iter().find(|t| t.name == n).unwrap();
        assert_eq!(find("v1.0").target, head);
        assert!(find("v1.0").annotated);
        assert_eq!(find("light").target, head);
        assert!(!find("light").annotated);
    }

    #[test]
    fn commits_filter_by_path_and_author() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/only.txt", p), "x\n").unwrap();
        stage(&p, &["only.txt".to_string()]).unwrap();
        commit(&p, "touch only", false, &[], false, false).unwrap();

        let by_path = commits(&p, 10, None, Some("only.txt")).unwrap();
        assert_eq!(
            by_path
                .iter()
                .map(|c| c.subject.as_str())
                .collect::<Vec<_>>(),
            ["touch only"]
        );
        assert!(commits(&p, 10, Some("nobody"), None).unwrap().is_empty());
        // Blank filters behave like no filter at all.
        assert_eq!(commits(&p, 10, Some(""), Some("  ")).unwrap().len(), 2);
    }

    /// A bare pathspec is a prefix match, which is what made the history page's
    /// path filter look dead: in a monorepo `admin` matched every commit and
    /// `main.ts` matched none. Subjects below are unique per commit so each
    /// assertion says which half of the fix is load-bearing.
    #[test]
    fn commits_path_filter_matches_substrings_not_just_prefixes() {
        let (_d, p) = init_repo();
        fs::create_dir_all(format!("{}/admin/packages/finance/src", p)).unwrap();
        fs::write(format!("{}/admin/packages/finance/src/main.ts", p), "x\n").unwrap();
        stage(&p, &["admin/packages/finance/src/main.ts".to_string()]).unwrap();
        commit(&p, "deep", false, &[], false, false).unwrap();
        fs::write(format!("{}/only.txt", p), "y\n").unwrap();
        stage(&p, &["only.txt".to_string()]).unwrap();
        commit(&p, "root", false, &[], false, false).unwrap();

        let subjects = |q: &str| {
            commits(&p, 10, None, Some(q))
                .unwrap()
                .iter()
                .map(|c| c.subject.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(subjects("main.ts"), ["deep"]);
        assert_eq!(subjects("packa"), ["deep"]);
        assert_eq!(subjects("MAIN.TS"), ["deep"]);
        assert_eq!(subjects("admin/packages/finance/src/main.ts"), ["deep"]);
        // 「复制路径」给的是绝对路径或 ./ 开头的路径，只有第一条裸 pathspec 认它。
        assert_eq!(subjects("./admin/packages/finance/src/main.ts"), ["deep"]);
        assert_eq!(subjects(format!("{}/only.txt", p).as_str()), ["root"]);
        assert_eq!(subjects("only"), ["root"]);
        // The globs must not degenerate into "match everything".
        assert!(subjects("zzz-not-a-path").is_empty());
    }

    #[test]
    fn repo_files_lists_tracked_paths_up_to_the_cap() {
        let (_d, p) = init_repo();
        fs::create_dir_all(format!("{}/admin/src", p)).unwrap();
        fs::write(format!("{}/admin/src/main.ts", p), "x\n").unwrap();
        stage(&p, &["admin/src/main.ts".to_string()]).unwrap();
        commit(&p, "add", false, &[], false, false).unwrap();

        let mut files = repo_files(&p, 500).unwrap();
        files.sort();
        assert_eq!(files, ["a.txt", "admin/src/main.ts"]);
        files = repo_files(&p, 1).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn commit_message_keeps_body_and_pushed_refs_only_list_published() {
        let (_d, p) = init_repo();
        // Multi-line messages survive intact so the editor can prefill them,
        // and the "is this published?" answer must not lie.
        git(
            &p,
            &["commit", "-q", "--amend", "-m", "改写前\n\n正文第二行"],
        );
        assert_eq!(commit_message(&p, "HEAD").unwrap(), "改写前\n\n正文第二行");
        assert!(pushed_refs(&p, "HEAD").unwrap().is_empty());

        let remote = tempfile::tempdir().unwrap();
        let remote_dir = remote.path().to_str().unwrap();
        git(remote_dir, &["init", "--bare", "-q", "upstream.git"]);
        git(
            &p,
            &[
                "remote",
                "add",
                "origin",
                &format!("{}/upstream.git", remote_dir),
            ],
        );
        let branch = current_branch(&p).unwrap();
        git(&p, &["push", "-q", "origin", &branch]);

        assert_eq!(
            pushed_refs(&p, "HEAD").unwrap(),
            [format!("origin/{}", branch)]
        );
        // A commit that is only local reads as unpublished.
        fs::write(format!("{}/next.txt", p), "x\n").unwrap();
        stage(&p, &["next.txt".to_string()]).unwrap();
        commit(&p, "还没推", false, &[], false, false).unwrap();
        assert!(pushed_refs(&p, "HEAD").unwrap().is_empty());
    }

    /// amend 换的是 HEAD 本身：提交条数不变、信息变成新的、暂存内容并进同一条。
    #[test]
    fn amend_replaces_head_instead_of_adding_a_commit() {
        let (_d, p) = init_repo();
        assert_eq!(run_git(&p, &["rev-list", "--count", "HEAD"]).unwrap().trim(), "1");
        fs::write(format!("{}/late.txt", p), "新内容\n").unwrap();
        stage(&p, &["late.txt".to_string()]).unwrap();

        commit(&p, "修正后的信息", true, &[], false, false).unwrap();

        assert_eq!(
            run_git(&p, &["rev-list", "--count", "HEAD"]).unwrap().trim(),
            "1",
            "amend 不该多出一条提交"
        );
        assert_eq!(commit_message(&p, "HEAD").unwrap(), "修正后的信息");
        assert!(
            run_git(&p, &["ls-tree", "--name-only", "HEAD"]).unwrap().contains("late.txt"),
            "已暂存的内容没并进 amend 后的 HEAD"
        );
    }

    /// The first push of a fresh branch: `-u` is the only way ahead/behind
    /// stops being a silent 0/0.
    #[test]
    fn a_first_push_can_record_its_upstream() {
        let (_d, p) = init_repo();
        let remote = tempfile::tempdir().unwrap();
        let remote_dir = remote.path().to_str().unwrap();
        git(remote_dir, &["init", "--bare", "-q", "upstream.git"]);
        git(
            &p,
            &[
                "remote",
                "add",
                "origin",
                &format!("{}/upstream.git", remote_dir),
            ],
        );
        let branch = current_branch(&p).unwrap();
        let up_of = |name: &str| {
            branches(&p)
                .unwrap()
                .into_iter()
                .find(|b| b.name == name)
                .and_then(|b| b.upstream)
        };
        assert_eq!(up_of(&branch), None, "新分支本来就没有上游");

        // 只给远程不给分支时 git 自己就拒（`The current branch … has no upstream
        // branch`，英文原文已被中文解释层覆盖）。前置检查不新增行为，只是把同一个
        // 契约说死、少起一个进程、不依赖那句英文原文的措辞——两条路都必须什么都没记下。
        assert!(push(&p, Some("origin"), None, false, true).is_err());
        assert_eq!(up_of(&branch), None, "被拒的调用不得留下半个上游");

        push(&p, Some("origin"), Some(&branch), false, true).unwrap();
        assert_eq!(up_of(&branch), Some(format!("origin/{}", branch)));
    }

    /// 推送卡那一行可以「本地叫 main、远端叫 main2」——落地必须按卡上那格说的算，
    /// 而不是按本地的名字。这条也是 `push_ref` 带 refspec 的那一半唯一被盯着的地方：
    /// 不带 refspec 时 git 会推成同名分支，而**同名那一行看不出错**，只有改名那条能暴露。
    #[test]
    fn a_push_row_lands_under_the_name_on_the_card() {
        let (_d, p) = init_repo();
        let remote = tempfile::tempdir().unwrap();
        let remote_dir = remote.path().to_str().unwrap();
        git(remote_dir, &["init", "--bare", "-q", "upstream.git"]);
        git(&p, &["remote", "add", "origin", &format!("{remote_dir}/upstream.git")]);
        let branch = current_branch(&p).unwrap();

        push_ref(&p, "origin", &branch, "renamed", false, true).unwrap();

        let on_remote = remote_branches(&p, "origin").unwrap();
        assert_eq!(on_remote, vec!["renamed".to_owned()], "远端上只有卡上那一格说的那个名字");
        // `-u` 记的也是那一格：记成 `origin/main` 的话，下一次 push 会往一个远端上
        // 并不存在的分支去比，ahead/behind 永远是假的。
        assert_eq!(
            branches(&p)
                .unwrap()
                .into_iter()
                .find(|b| b.name == branch)
                .and_then(|b| b.upstream),
            Some("origin/renamed".to_owned())
        );
    }

    /// 「远程分支」那一格被清空 = 填错了，**当场拒**，一个 git 进程都不起。
    /// 拼成 `main:` 交给 git 会报「src refspec main does not match any」——说的是左边、
    /// 坏的是右边，用户照着报错去查本地分支名，什么都查不到。
    #[test]
    fn a_push_row_with_a_blank_target_never_reaches_git() {
        let (_d, p) = init_repo();
        let remote = tempfile::tempdir().unwrap();
        let remote_dir = remote.path().to_str().unwrap();
        git(remote_dir, &["init", "--bare", "-q", "upstream.git"]);
        git(&p, &["remote", "add", "origin", &format!("{remote_dir}/upstream.git")]);
        let branch = current_branch(&p).unwrap();

        assert!(push_ref(&p, "origin", &branch, "", false, false).is_err());
        assert!(remote_branches(&p, "origin").unwrap().is_empty(), "被拒的那一行不该在远端留下东西");
    }

    /// 拉取卡「要拉取的远程分支」那一格按**远端**列，不是把所有远端跟踪引用摊在一张表里：
    /// 一本仓库可以同时挂 origin 和 fork，两边的同名分支是两条不同的东西。
    /// `refs/remotes/origin/HEAD` 是「默认分支」那条指针，不是位置——它必须在源头就被滤掉，
    /// 否则那一格会多出一条拉不动的空选项（理由同 [`remote_refs`] 里 lstrip 那一段）。
    #[test]
    fn remote_branches_are_listed_per_remote_without_the_prefix() {
        let (_d, p) = init_repo();
        let r1 = tempfile::tempdir().unwrap();
        let r2 = tempfile::tempdir().unwrap();
        let (d1, d2) = (r1.path().to_str().unwrap(), r2.path().to_str().unwrap());
        git(d1, &["init", "--bare", "-q", "a.git"]);
        git(d2, &["init", "--bare", "-q", "b.git"]);
        git(&p, &["remote", "add", "origin", &format!("{d1}/a.git")]);
        git(&p, &["remote", "add", "fork", &format!("{d2}/b.git")]);
        let branch = current_branch(&p).unwrap();
        git(&p, &["branch", "feature/x"]);
        push(&p, Some("origin"), Some(&branch), false, false).unwrap();
        push(&p, Some("origin"), Some("feature/x"), false, false).unwrap();
        push(&p, Some("fork"), Some(&branch), false, false).unwrap();
        git(&p, &["remote", "set-head", "origin", &branch]);

        assert_eq!(
            remote_branches(&p, "origin").unwrap(),
            // 顺序是 `for-each-ref` 给的（按引用名排），不是推送顺序——那一格是给用户挑的
            // 列表，稳定的顺序比「推的先后」有用，判据钉的是**集合**加这个确定性。
            vec!["feature/x".to_owned(), branch.clone()],
            "嵌套的分支名要整个留下，只剥掉远端名前缀那一段"
        );
        assert_eq!(remote_branches(&p, "fork").unwrap(), vec![branch.clone()]);
        assert!(remote_branches(&p, "没有这个远端").unwrap().is_empty());
    }

    /// 「推送所有标签」那一句得真的把标签推到远端去——它是 `--tags`，不是「顺便推一下
    /// 当前分支」。这条也是这整批里唯一一条**盯着远端看一眼**的标签判据：
    /// 只测本地建了标签，等于什么都没测。
    #[test]
    fn push_all_tags_sends_the_tags_and_only_the_tags() {
        let (_d, p) = init_repo();
        let remote = tempfile::tempdir().unwrap();
        let remote_dir = remote.path().to_str().unwrap();
        git(remote_dir, &["init", "--bare", "-q", "upstream.git"]);
        git(&p, &["remote", "add", "origin", &format!("{remote_dir}/upstream.git")]);
        let branch = current_branch(&p).unwrap();
        git(&p, &["tag", "v1"]);
        push_all_tags(&p, "origin").unwrap();

        let on_remote = run_git(remote_dir, &["ls-remote", "--tags", "-q", "upstream.git"]).unwrap();
        assert!(on_remote.contains("refs/tags/v1"), "远端上没有那个标签：{on_remote}");
        // `--tags` 不推分支：分支那一条得留到卡上表格那一行自己落地时才走。
        let refs_on_remote = run_git(remote_dir, &["ls-remote", "--heads", "upstream.git"]).unwrap();
        assert!(!refs_on_remote.contains(&branch), "推标签时把分支也推上去了：{refs_on_remote}");
    }

    /// 拉取卡「选项」里那三句真的换一个 git 参数，而且**换出来的结果看得见的不同**：
    /// 同一条分叉，`--rebase` 下面是线性历史，`--no-ff` 下面必然多一枚合并提交。
    /// 只测 argv 不测结果是假的（契约见 docs/PROCESS.md「判据方法论」）。
    #[test]
    fn pull_options_change_what_lands_in_the_history() {
        // 造一份「本地和远端各多一条」的真分叉，按给定选项拉完，数历史里有几枚合并提交。
        // 两个临时目录要活到断言之后，所以连着仓库路径一起还回去。
        let forked_then_pull = |rebase: bool, no_ff: bool| -> (tempfile::TempDir, tempfile::TempDir, usize) {
            let (mine, base) = init_repo();
            let upstream = tempfile::tempdir().unwrap();
            let up_dir = upstream.path().to_str().unwrap();
            git(up_dir, &["init", "--bare", "-q", "upstream.git"]);
            git(&base, &["remote", "add", "origin", &format!("{up_dir}/upstream.git")]);
            let branch = current_branch(&base).unwrap();
            push(&base, Some("origin"), Some(&branch), false, true).unwrap();

            // 远端那一侧由另一份工作区推进。
            let other = tempfile::tempdir().unwrap();
            let o = other.path().to_str().unwrap();
            git(o, &["clone", "-q", &format!("{up_dir}/upstream.git"), "w"]);
            git(o, &["-C", "w", "config", "user.email", "t@t.com"]);
            git(o, &["-C", "w", "config", "user.name", "other"]);
            fs::write(format!("{o}/w/b.txt"), "远端那条\n").unwrap();
            git(o, &["-C", "w", "add", "."]);
            git(o, &["-C", "w", "commit", "-q", "-m", "远端多一条"]);
            git(o, &["-C", "w", "push", "-q"]);
            drop(other);

            // 本地也推进一条 → 到这儿才是真的分叉，合还是变基这才看得出来。
            fs::write(format!("{base}/c.txt"), "本地那条\n").unwrap();
            git(&base, &["add", "."]);
            git(&base, &["commit", "-q", "-m", "本地多一条"]);

            let opts = PullOpts { rebase, no_ff, log: false };
            pull(&base, Some("origin"), Some(&branch), &opts, None).unwrap();
            let merges = run_git(&base, &["log", "--format=%P"])
                .unwrap()
                .lines()
                .filter(|l| l.split_whitespace().count() > 1)
                .count();
            (mine, upstream, merges)
        };

        let (m1, u1, rebase_merges) = forked_then_pull(true, false);
        let (m2, u2, no_ff_merges) = forked_then_pull(false, true);
        assert_eq!(rebase_merges, 0, "变基之后不该有合并提交");
        assert!(no_ff_merges >= 1, "`--no-ff` 那一侧必须多出一枚合并提交");
        drop((m1, u1, m2, u2));
    }

    /// A number without a baseline is not "in sync", and the interface must be
    /// able to say which of the two it is looking at.
    #[test]
    fn counts_are_only_claimed_when_a_branch_actually_tracks_something() {
        let (_d, p) = init_repo();
        let st = status(&p).unwrap();
        assert_eq!(st.upstream, None, "新分支没有上游");
        assert_eq!((st.ahead, st.behind), (0, 0), "没有上游时两个数都是假的 0");

        let remote = tempfile::tempdir().unwrap();
        let remote_dir = remote.path().to_str().unwrap();
        git(remote_dir, &["init", "--bare", "-q", "upstream.git"]);
        git(
            &p,
            &[
                "remote",
                "add",
                "origin",
                &format!("{}/upstream.git", remote_dir),
            ],
        );
        let branch = current_branch(&p).unwrap();
        push(&p, Some("origin"), Some(&branch), false, true).unwrap();

        let st = status(&p).unwrap();
        assert_eq!(st.upstream, Some(format!("origin/{}", branch)));
        assert_eq!((st.ahead, st.behind), (0, 0), "刚推完才是真的同步");

        fs::write(format!("{}/more.txt", p), "x\n").unwrap();
        stage(&p, &["more.txt".to_string()]).unwrap();
        commit(&p, "本地多一条", false, &[], false, false).unwrap();
        let st = status(&p).unwrap();
        assert_eq!((st.ahead, st.behind, st.upstream.is_some()), (1, 0, true));
    }

    /// What a push would send, asked once for the whole list.
    #[test]
    fn unpushed_commits_are_only_local_ones() {
        let (_d, p) = init_repo();
        // No remote: nothing to be behind, so the answer is empty rather than
        // "every commit ever", which would badge the whole history.
        assert!(unpushed_commits(&p).is_empty());

        let remote = tempfile::tempdir().unwrap();
        let remote_dir = remote.path().to_str().unwrap();
        git(remote_dir, &["init", "--bare", "-q", "upstream.git"]);
        git(
            &p,
            &[
                "remote",
                "add",
                "origin",
                &format!("{}/upstream.git", remote_dir),
            ],
        );
        let branch = current_branch(&p).unwrap();
        git(&p, &["push", "-q", "origin", &branch]);
        assert!(
            unpushed_commits(&p).is_empty(),
            "freshly pushed is fully published"
        );

        for i in 0..2 {
            fs::write(format!("{}/p{}.txt", p, i), "x\n").unwrap();
            stage(&p, &[format!("p{}.txt", i)]).unwrap();
            commit(&p, &format!("本地提交 {}", i), false, &[], false, false).unwrap();
        }
        let u = unpushed_commits(&p);
        assert_eq!(u.len(), 2, "both local commits are waiting for a push");
        assert_eq!(u[0], head(&p).unwrap(), "newest first");
    }

    #[test]
    fn file_log_follows_renames_and_labels_changes() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/old.txt", p), "a\n").unwrap();
        stage(&p, &["old.txt".to_string()]).unwrap();
        commit(&p, "add old", false, &[], false, false).unwrap();
        fs::rename(format!("{}/old.txt", p), format!("{}/new.txt", p)).unwrap();
        stage(&p, &["old.txt".to_string(), "new.txt".to_string()]).unwrap();
        commit(&p, "rename it", false, &[], false, false).unwrap();
        fs::write(format!("{}/new.txt", p), "a\nb\n").unwrap();
        stage(&p, &["new.txt".to_string()]).unwrap();
        commit(&p, "edit new", false, &[], false, false).unwrap();

        let log = file_log(&p, "new.txt", 50).unwrap();
        let got: Vec<&str> = log.iter().map(|e| e.commit.subject.as_str()).collect();
        assert_eq!(got, ["edit new", "rename it", "add old"]);
        assert_eq!(log[0].change, ChangeStatus::Modified);
        assert_eq!(log[1].change, ChangeStatus::Renamed);
        assert_eq!(log[1].old_path.as_deref(), Some("old.txt"));
        assert_eq!(log[2].change, ChangeStatus::Added);
    }

    #[test]
    fn file_log_marks_deletion() {
        let (_d, p) = init_repo();
        fs::remove_file(format!("{}/a.txt", p)).unwrap();
        stage(&p, &["a.txt".to_string()]).unwrap();
        commit(&p, "drop a", false, &[], false, false).unwrap();

        let log = file_log(&p, "a.txt", 20).unwrap();
        assert_eq!(log[0].commit.subject, "drop a");
        assert_eq!(log[0].change, ChangeStatus::Deleted);
        assert_eq!(log[1].change, ChangeStatus::Added);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn rev_file_text_and_restore_file() {
        let (_d, p) = init_repo(); // a.txt = "hello\n" at HEAD
        let first = commits(&p, 1, None, None).unwrap()[0].id.clone();
        fs::write(format!("{}/a.txt", p), "changed\n").unwrap();
        stage(&p, &["a.txt".to_string()]).unwrap();
        commit(&p, "change a", false, &[], false, false).unwrap();

        assert_eq!(rev_file_text(&p, &first, "a.txt").unwrap(), "hello\n");

        restore_file(&p, &first, "a.txt").unwrap();
        assert_eq!(
            fs::read_to_string(format!("{}/a.txt", p)).unwrap(),
            "hello\n"
        );
        // git checkout <oid> -- <path> also stages the restored content.
        let st = status(&p).unwrap();
        assert!(st.files.iter().any(|f| f.path == "a.txt" && f.staged));
    }

    #[test]
    fn resolving_with_theirs_stages_that_side_and_continue_finishes_the_merge() {
        let (_d, p) = merge_conflict();
        let before = status(&p).unwrap();
        assert_eq!(before.repo_state, "merge");
        assert!(before
            .files
            .iter()
            .any(|f| f.path == "a.txt" && f.index_status == ChangeStatus::Unmerged));

        resolve_theirs(&p, "a.txt").unwrap();
        assert_eq!(fs::read_to_string(format!("{}/a.txt", p)).unwrap(), "theirs\n");
        let resolved = status(&p).unwrap();
        assert_eq!(resolved.repo_state, "merge");
        assert!(!resolved
            .files
            .iter()
            .any(|f| f.index_status == ChangeStatus::Unmerged));

        continue_op(&p).unwrap();
        let after = status(&p).unwrap();
        assert_eq!(after.repo_state, "none");
        assert!(after.files.is_empty(), "{}", after.files.len());
    }

    #[test]
    fn mark_resolved_stages_the_hand_edited_file_and_mark_unresolved_puts_it_back() {
        let (_d, p) = merge_conflict();
        // The user says "keep both": write a merged file by hand and stage it.
        fs::write(format!("{}/a.txt", p), "mine\ntheirs\n").unwrap();
        mark_resolved(&p, "a.txt").unwrap();
        let resolved = status(&p).unwrap();
        assert!(resolved.files.iter().any(|f| f.path == "a.txt" && f.staged));
        assert!(!resolved
            .files
            .iter()
            .any(|f| f.index_status == ChangeStatus::Unmerged));

        mark_unresolved(&p, "a.txt").unwrap();
        let back = status(&p).unwrap();
        assert!(back
            .files
            .iter()
            .any(|f| f.path == "a.txt" && f.index_status == ChangeStatus::Unmerged));
        let text = fs::read_to_string(format!("{}/a.txt", p)).unwrap();
        assert!(text.contains("<<<<<<<"), "{text}");
    }

    #[test]
    fn abort_restores_the_pre_merge_worktree() {
        let (_d, p) = merge_conflict();
        abort_op(&p).unwrap();
        assert_eq!(status(&p).unwrap().repo_state, "none");
        assert_eq!(fs::read_to_string(format!("{}/a.txt", p)).unwrap(), "mine\n");
    }

    #[test]
    fn untracked_patch_synthesizes_added_file() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/new.txt", p), "one\ntwo\n").unwrap();
        // git diff reports nothing for an untracked file...
        assert!(working_patch(&p, false, Some("new.txt"))
            .unwrap()
            .is_empty());
        let patch = untracked_patch(&p, "new.txt").unwrap();
        assert!(patch.starts_with("diff --git a/new.txt b/new.txt\n"));
        assert!(patch.contains("new file mode 100644\n"));
        assert!(patch.contains("@@ -0,0 +1,2 @@\n+one\n+two\n"));
    }

    #[test]
    fn untracked_patch_reports_binary_without_content() {
        let (_d, p) = init_repo();
        fs::write(format!("{}/blob.bin", p), [0u8, 1, 2, 3]).unwrap();
        let patch = untracked_patch(&p, "blob.bin").unwrap();
        assert!(patch.starts_with("diff --git a/blob.bin b/blob.bin\n"));
        assert!(patch.contains("二进制新文件"));
        assert!(!patch.contains("+++"));
    }

    /// Git's own default (`core.quotepath=true`) turns `中文文件.txt` into
    /// `"\344\270\255\246\207\207\207.txt"`, so every Chinese path in the
    /// working copy would arrive as escape soup.
    #[test]
    fn chinese_paths_are_not_octal_escaped() {
        let (_d, p) = init_repo();
        let name = "中文文件.txt";
        fs::write(format!("{}/{}", p, name), "旧内容\n").unwrap();
        stage(&p, &[name.to_string()]).unwrap();
        fs::write(format!("{}/{}", p, name), "新内容\n").unwrap();

        let st = status(&p).unwrap();
        assert!(
            st.files.iter().any(|f| f.path == name),
            "暂存与工作区都应给出原文名，实际得到 {:?}",
            st.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>()
        );
        assert!(
            !st.files.iter().any(|f| f.path.contains("\\3")),
            "不该出现八进制转义：{:?}",
            st.files
        );

        // The same name has to survive the diff paths, or the right-hand pane
        // would be opened by a path git never printed.
        let patch = working_patch(&p, true, Some(name)).unwrap();
        assert!(patch.contains(name), "暂存 diff 里也要是原文名：{}", patch);
    }

    /// A GBK-encoded source file from a Windows-era domestic repo is invalid
    /// UTF-8 at the byte level. Before `decode` that made `git diff` fail
    /// outright, i.e. the file (and its diff) simply could not be opened.
    #[test]
    fn legacy_gbk_bytes_degrade_instead_of_failing() {
        use std::io::Write;
        let (_d, p) = init_repo();
        // "中文门槛" in GBK — high bytes, no NUL, so git treats it as text.
        let gbk: &[u8] = &[0xd6, 0xd0, 0xce, 0xc4, 0xc3, 0xc5, 0xa3, 0xba, 0x0a];
        fs::File::create(format!("{}/gbk.txt", p))
            .unwrap()
            .write_all(gbk)
            .unwrap();
        stage(&p, &["gbk.txt".to_string()]).unwrap();
        commit(&p, "add legacy file", false, &[], false, false).unwrap();
        fs::File::create(format!("{}/gbk.txt", p))
            .unwrap()
            .write_all(&[0xb4, 0xf3, 0xd2, 0xbb, 0xd0, 0xd0, 0x0a])
            .unwrap();

        let patch = working_patch(&p, false, Some("gbk.txt"));
        assert!(
            patch.is_ok(),
            "非法字节不该让整份 diff 失败：{:?}",
            patch.err()
        );
        assert!(
            patch.unwrap().contains('\u{fffd}'),
            "坏字节应替换为 U+FFFD 而不是丢弃整段"
        );

        // And the committed blob stays readable the same way.
        let hist = commits(&p, 10, None, None).unwrap();
        assert_eq!(hist[0].subject, "add legacy file");
    }

    #[test]
    fn node_templates_become_one_shot_rewrites() {
        let url = "https://github.com/a/b.git";
        // The `{url}` form the clone UI already offers.
        assert_eq!(
            node_rewrite_args("https://ghproxy.net/{url}", url).unwrap(),
            [
                "-c",
                "url.https://ghproxy.net/https://github.com/a/b.git.insteadOf=https://github.com/a/b.git"
            ],
            "节点必须看到整条 URL，前缀型节点的文档写法就是这样"
        );
        // A bare prefix means the same thing; the missing slash is added.
        assert_eq!(
            node_rewrite_args("https://ghproxy.net", url).unwrap()[1],
            "url.https://ghproxy.net/https://github.com/a/b.git.insteadOf=https://github.com/a/b.git"
        );
        // SSH remotes cannot be wrapped — said before anything touches the network.
        let ssh = node_rewrite_args("https://n/{url}", "git@github.com:a/b.git")
            .unwrap_err()
            .to_string();
        assert!(ssh.contains("只能改写 https"), "{}", ssh);
        // Template text comes from a settings field, so junk is refused, not escaped.
        for bad in ["", "   ", "https://n/ a{url}", "https://n/{url}=x"] {
            assert!(
                node_rewrite_args(bad, url).is_err(),
                "should reject {:?}",
                bad
            );
        }
    }

    /// A node is a URL prefix, so a local `file://` prefix stands in for one and
    /// the fake `127.0.0.1:9` remote refuses instantly — this runs offline.
    #[test]
    fn a_node_routes_only_the_network_half_of_a_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let s = |p: std::path::PathBuf| p.to_str().unwrap().to_string();

        // The upstream the node serves.
        let seed = s(root.join("seed"));
        fs::create_dir_all(&seed).unwrap();
        git(&seed, &["init", "-q"]);
        git(&seed, &["config", "user.email", "t@t.com"]);
        git(&seed, &["config", "user.name", "tester"]);
        fs::write(format!("{}/a.txt", seed), "hello\n").unwrap();
        git(&seed, &["add", "."]);
        git(&seed, &["commit", "-q", "-m", "from upstream"]);
        git(&seed, &["branch", "-M", "main"]);
        let node = root.join("node");
        // `file://` + `http://host/…` collapses to `http:/host` on disk.
        fs::create_dir_all(node.join("http:/127.0.0.1:9/ghost")).unwrap();
        Proc::new("git")
            .args(["clone", "-q", "--bare"])
            .arg(&seed)
            .arg(node.join("http:/127.0.0.1:9/ghost/xyz.git"))
            .output()
            .unwrap();

        // The work repo's origin is a URL that cannot be reached here.
        let work = s(root.join("work"));
        fs::create_dir_all(&work).unwrap();
        git(&work, &["init", "-q"]);
        git(
            &work,
            &[
                "remote",
                "add",
                "origin",
                "http://127.0.0.1:9/ghost/xyz.git",
            ],
        );

        assert!(
            fetch(&work, None, None).is_err(),
            "不走节点应当失败，否则这条测试没有判别力"
        );
        // A node is a URL prefix, so the stand-in is a `file://` one; without the
        // leading scheme git reads the path's own `://` as the protocol.
        let tpl = format!("file://{}/{{url}}", node.display());
        fetch(&work, None, Some(&tpl)).expect("节点应当接管网络那一半");

        let refs = run_git(&work, &["for-each-ref", "refs/remotes"]).unwrap();
        assert!(
            refs.contains("refs/remotes/origin/main"),
            "提交应落在 origin/*：{}",
            refs
        );
        assert_eq!(
            remote_url(&work, "origin").unwrap(),
            "http://127.0.0.1:9/ghost/xyz.git",
            "origin 应当保持用户填写的地址"
        );
    }

    #[test]
    fn git_failures_are_explained_in_chinese_and_keep_the_original() {
        let out = explain_git_error(
            "error: The following untracked working tree files would be overwritten by merge:\n\ta.txt\n",
        );
        assert!(out.starts_with("本地改动会被覆盖"), "{}", out);
        assert!(
            out.contains("would be overwritten by merge"),
            "原文必须留着，方便搜索"
        );

        // Matching is on lowercased substrings, so git's capitalisation varies.
        assert!(explain_git_error(
            "fatal: Not a git repository (or any of the parent directories)"
        )
        .starts_with("这里不是一个 git 仓库"));
        assert!(
            explain_git_error(
                "fatal: The current branch main has no upstream branch.\nTo push the current branch and set the remote as upstream, use"
            )
            .contains("当前分支没有关联远程分支")
        );

        // Unrecognised output must pass through untouched — inventing an
        // explanation for a failure we do not understand would be worse.
        let odd = "fatal: this error predates every hint above";
        assert_eq!(explain_git_error(odd), odd);
        assert_eq!(explain_git_error("  \n"), "");

        // Clone is where domestic users hit the network hardest, and it runs
        // through the same layer as everything else.
        let timeout = explain_git_error(
            "Cloning into 'gitx'....\nfatal: unable to access 'https://github.com/a/b/': \
             Failed to connect to github.com port 443 after 75000 ms: Operation timed out",
        );
        assert!(timeout.contains("连接超时"), "{}", timeout);
        assert!(timeout.contains("加速节点"), "{}", timeout);
        assert!(explain_git_error(
            "remote: Repository not found.\nfatal: repository 'https://github.com/a/b/' not found"
        )
        .starts_with("远程仓库不存在"));

        // Wired into every call rather than just the helper: a real failure
        // reaching the UI already reads as guidance.
        let (_d, p) = init_repo();
        let err = checkout_branch(&p, "nope").unwrap_err().to_string();
        assert!(err.contains("没有匹配到这些路径"), "{}", err);
        assert!(err.contains("pathspec"));
    }
}
