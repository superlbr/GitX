//! Code-host provider plugins (GitHub / Gitee / Codeup …).
//!
//! A provider is a compile-time plugin: it declares which remote URLs it serves
//! and which capabilities it offers, and it turns a repo + token into a plain
//! `ApiRequest`. Doing the HTTP itself is left to the UI layer, so every
//! provider stays unit-testable against fixture JSON with no network access.

mod codeup;
mod gitee;
mod github;

use serde::{Deserialize, Serialize};

use crate::error::GitxError;
use crate::lang::Lang;
pub use crate::error::Result;
use crate::models::{Check, CheckOutcome, Ci, PullRequest};

pub use codeup::Codeup;
pub use gitee::Gitee;
pub use github::GitHub;

/// What a provider can actually do. The UI renders only what a provider declares,
/// so an unimplemented capability is invisible rather than a dead button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    PullRequests,
    /// 单条 PR 的详情端点，里面有列表**不给**的那一格 `mergeable_state`
    /// （2026-10-31 实测：GitHub 的 `/pulls` 没有这一格，`/pulls/{号}` 才有）。
    PullRequestDetail,
    CiStatus,
    CreatePullRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
    Get,
    Post,
}

/// A request the host layer executes verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl ApiRequest {
    pub fn get(url: String) -> Self {
        ApiRequest {
            method: HttpMethod::Get,
            url,
            headers: vec![],
            body: None,
        }
    }

    pub fn with_header(mut self, key: &str, value: impl Into<String>) -> Self {
        self.headers.push((key.to_string(), value.into()));
        self
    }
}

/// `owner/repo` plus the host it lives on, parsed out of a remote URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub host: String,
    pub path: String,
}

/// Accepts `git@host:owner/repo.git`, `https://host/owner/repo.git` and
/// `https://user:token@host/owner/repo.git`; nested groups stay intact.
pub fn parse_remote(url: &str) -> Option<RepoRef> {
    let raw = url.trim();
    if raw.is_empty() {
        return None;
    }
    // scp-style "git@host:owner/repo.git" has to become "host/owner/repo.git"
    // first, otherwise the host would be read as "git@host".
    let rewritten;
    let body = if let Some(i) = raw.find("://") {
        &raw[i + 3..]
    } else if let Some(i) = raw.find(':') {
        rewritten = format!("{}/{}", &raw[..i], &raw[i + 1..]);
        rewritten.as_str()
    } else {
        return None;
    };
    // Drop credentials that some remotes carry in the URL itself.
    let body = body.split('@').next_back().unwrap_or(body);
    let (host, tail) = body.split_once('/')?;
    if host.is_empty() || tail.is_empty() {
        return None;
    }
    let path = tail.trim_matches('/').trim_end_matches(".git").to_string();
    if path.is_empty() || !path.contains('/') {
        return None;
    }
    Some(RepoRef {
        host: host.to_ascii_lowercase(),
        path,
    })
}

pub trait Host: Send + Sync {
    /// Stable key used to store the token: `provider_token_<id>`.
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Which remote URL this plugin serves.
    fn handles(&self, url: &str) -> bool;
    fn capabilities(&self) -> Vec<Capability>;
    /// How to obtain a token, shown next to the input field.
    fn token_hint(&self) -> &'static str;

    /// Request for the repo's open pull requests. Err means the provider is
    /// recognised here but this capability is not implemented yet.
    fn pull_requests_request(&self, repo: &RepoRef, token: &str) -> Result<ApiRequest>;
    fn parse_pull_requests(&self, body: &str) -> Result<Vec<PullRequest>>;
    /// Where to send the browser when the user opens a PR.
    fn pull_request_web_url(&self, repo: &RepoRef, number: u32) -> String;

    /// 按号取**单条** PR：`mergeable_state` 只有这一格给（见 `Capability::PullRequestDetail`）。
    /// 默认「这家没接」而不是假装成功——看板于是继续说实话，不编一个状态出来。
    fn pull_request_detail_request(&self, _repo: &RepoRef, _token: &str, _number: u32) -> Result<ApiRequest> {
        Err(err(format!("{} 没有接「按号取 PR 详情」", self.label())))
    }
    fn parse_pull_request_detail(&self, _body: &str) -> Result<PullRequest> {
        Err(err(format!("{} 没有接「按号取 PR 详情」", self.label())))
    }

    /// 按 head 的 sha 取那一版提交上的检查。声明了 `CiStatus` 才有人来问。
    fn checks_request(&self, _repo: &RepoRef, _token: &str, _sha: &str) -> Result<ApiRequest> {
        Err(err(format!("{} 没有接「按提交取检查」", self.label())))
    }
    /// 解析成**归一后的每条检查**（不是汇总）：汇总要跨行比对名字，归一留给这里、
    /// 求和留给 `aggregate_ci`，这样每一家都能复用同一条汇总规则。
    fn parse_checks(&self, _body: &str) -> Result<Vec<Check>> {
        Err(err(format!("{} 没有接「按提交取检查」", self.label())))
    }
}

/// The built-in registry. Adding a host = one struct here plus its own file.
pub const HOSTS: [&'static dyn Host; 3] = [&GitHub, &Gitee, &Codeup];

/// Pick the provider for a repo's remote URL (origin first).
pub fn detect(remote_urls: &[String]) -> Option<&'static dyn Host> {
    remote_urls
        .iter()
        .find_map(|u| HOSTS.into_iter().find(|h| h.handles(u)))
}

/// The `owner/repo` the provider API needs, taken from the same URL the
/// provider matched, so detection and path parsing can never disagree.
pub fn repo_ref_for(host: &dyn Host, remote_urls: &[String]) -> Option<RepoRef> {
    remote_urls
        .iter()
        .find(|u| host.handles(u))
        .and_then(|u| parse_remote(u))
}

/// 一版提交上的检查归成看板那一档。
///
/// 进来的 `Check` 是**同一个名字只剩最新一条**之后的（去重归 provider 的解析做，
/// 因为只有它手上有 `id`）；这一层只管求和，所以与顺序无关。
/// 「跳过」算过——它不挡路；「在跑」和「挂了」同时存在时**在跑赢**，
/// 因为此刻报出来的失败数还会长。
pub fn aggregate_ci(checks: &[Check]) -> Ci {
    if checks.is_empty() {
        return Ci::NoChecks;
    }
    let (mut passed, mut pending) = (0u32, 0u32);
    let mut failed: Vec<String> = checks
        .iter()
        .filter(|c| c.outcome == CheckOutcome::Fail)
        .map(|c| c.name.clone())
        .collect();
    for c in checks {
        match c.outcome {
            CheckOutcome::Pass => passed += 1,
            CheckOutcome::Fail => {}
            CheckOutcome::Pending => pending += 1,
        }
    }
    failed.sort();
    if pending > 0 {
        return Ci::Running { passed, failed: failed.len() as u32, pending };
    }
    if !failed.is_empty() {
        return Ci::Failed { failed };
    }
    Ci::Passed { total: passed }
}

/// 算一句话时要递进来的两件**行外**的事：它们都不属于某一条 PR，所以不能从
/// `PullRequest` 自己读出来（硬塞进去就成了「界面状态存在数据模型里」那一类）。
#[derive(Debug, Clone, Copy, Default)]
pub struct StatusContext {
    /// 这一行的详情或检查**真的在飞**。不飞的时候要说「还没算出来」，
    /// 说「正在看」就是骗人——压根不取的那一家（Gitee）永远等不到答案。
    pub fetching: bool,
    /// 这条 PR 上挂着的那个 job 名字，**在同一屏别条 PR 上也挂着**（正向证据）。
    pub failing_elsewhere: bool,
}

/// 看板那一行的状态：**一句话说明这条 PR 现在卡在哪、下一步该谁动**。
///
/// 这里是枚举而不是文案：同一条判断此前有**两份实现**（这边出中文串、外壳那边再拿
/// `mergeable_state` 匹配一遍颜色），两份总会漂移。现在文案与配色都只认这一个值。
///
/// 顺序按**挡路的那件事**排，规则是「取最上游的那一档」：草稿挡住评审**和**合并，
/// 冲突只挡住合并，所以草稿在前；CI 的坏消息在合并轴之前说话，因为 `blocked`
/// 那一格到底是被检查挡着还是被评审挡着，光看它自己分不出来。
/// **合并轴只在检查已知（跑完，或确实一个都没有）时才说话。**
/// `mergeable_state` 是 GitHub 的取值；认不出来（含这一格根本没给）就说「还没算出来」——
/// **不猜**，猜错的状态比没有状态更坏。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrStatus {
    Merged,
    Closed,
    Draft,
    Conflicts,
    /// 详情/检查在飞，这一行还没有可说的。
    Fetching,
    CiRunning { failed: u32 },
    /// 挂了，且同一个 job 在别条 PR 上也挂着——这句的意思是不必急着查本次改动。
    CiFailedShared { n: u32 },
    /// 挂了，没有旁证。**不指责任何人**：只有「几个没过」是量出来的。
    CiFailed { n: u32 },
    /// 挂了但不挡合并——这一句是 GitHub 自己说的（`mergeable_state = unstable`）。
    CiWarned { n: u32 },
    /// 这一版提交上一个检查都没有。
    CiNone,
    Behind,
    AwaitingReview,
    MergeReady,
    Unknown,
}

impl PrStatus {
    /// 一句话。**语言是参数而不是环境**：这一层不读线程局部，两个档位在同一条判据里
    /// 就能各自断言，不用 set/restore 那一套。
    /// 带数字的那几档要插值，所以回 `String` 而不是 `&'static str`。
    pub fn label(&self, lang: Lang) -> String {
        use PrStatus::*;
        match (self, lang) {
            (Merged, Lang::Zh) => "已合并".into(),
            (Merged, Lang::En) => "Merged".into(),
            (Closed, Lang::Zh) => "已关闭".into(),
            (Closed, Lang::En) => "Closed".into(),
            (Draft, Lang::Zh) => "草稿 · 作者还没准备好".into(),
            (Draft, Lang::En) => "Draft · the author isn't ready".into(),
            (Conflicts, Lang::Zh) => "有冲突 · 需要作者处理".into(),
            (Conflicts, Lang::En) => "Merge conflicts · the author has to resolve them".into(),
            (Fetching, Lang::Zh) => "正在看它卡在哪…".into(),
            (Fetching, Lang::En) => "Checking what blocks it…".into(),
            (CiRunning { failed }, Lang::Zh) if *failed == 0 => "CI 正在跑".into(),
            (CiRunning { failed }, Lang::Zh) => format!("CI 正在跑 · 已经挂了 {failed} 个"),
            (CiRunning { failed }, Lang::En) if *failed == 0 => "CI running".into(),
            (CiRunning { failed }, Lang::En) => {
                format!("CI running · {failed} failed so far")
            }
            (CiFailedShared { n }, Lang::Zh) => format!("{n} 个失败 · 在别的 PR 上也挂"),
            (CiFailedShared { n }, Lang::En) => {
                format!("{n} failures · the same jobs fail on other pull requests too")
            }
            (CiFailed { n }, Lang::Zh) => format!("{n} 个检查没通过"),
            (CiFailed { n }, Lang::En) => format!("{n} checks failing"),
            (CiWarned { n }, Lang::Zh) if *n == 0 => "有检查没过 · 但不挡合并".into(),
            (CiWarned { n }, Lang::Zh) => format!("{n} 个检查没过 · 但不挡合并"),
            (CiWarned { n }, Lang::En) if *n == 0 => {
                "Some checks failed but they don't block the merge".into()
            }
            (CiWarned { n }, Lang::En) => {
                format!("{n} checks failed but they don't block the merge")
            }
            (CiNone, Lang::Zh) => "最新提交上还没有检查".into(),
            (CiNone, Lang::En) => "No checks on the latest commit".into(),
            (Behind, Lang::Zh) => "落后于目标分支 · 先更新".into(),
            (Behind, Lang::En) => "Behind the base branch · update it first".into(),
            (AwaitingReview, Lang::Zh) => "等待评审".into(),
            (AwaitingReview, Lang::En) => "Waiting for review".into(),
            (MergeReady, Lang::Zh) => "可以合并".into(),
            (MergeReady, Lang::En) => "Ready to merge".into(),
            (Unknown, Lang::Zh) => "状态还没算出来".into(),
            (Unknown, Lang::En) => "Status not computed yet".into(),
        }
    }
}

/// 检查那一次还没落地时，平台的哪几格**自己就把话说完了**：
/// `clean` 是「该有的都有了」（这是把检查算进去之后的结论）、`behind` 是「差更新」、
/// `unstable` / `has_warnings` 是「挂的那些不挡路」。这三类不必再等。
/// 只有 `blocked` 分不出来——它可能是缺评审，也可能是缺一个还没跑完的检查；
/// 那种场合宁可说「正在看」或「还没算出来」，不许猜一个「等待评审」把人派给不该动的人。
fn speaks_for_itself(mergeable_state: &str) -> Option<PrStatus> {
    match mergeable_state {
        "clean" => Some(PrStatus::MergeReady),
        "behind" => Some(PrStatus::Behind),
        "unstable" | "has_warnings" => Some(PrStatus::CiWarned { n: 0 }),
        _ => None,
    }
}

pub fn pr_status(pr: &PullRequest, ctx: &StatusContext) -> PrStatus {
    match pr.state.as_str() {
        "closed" => return PrStatus::Closed,
        "merged" => return PrStatus::Merged,
        _ => {}
    }
    // 草稿挡住评审**和**合并，冲突只挡住合并：同属「作者的回合」时取最上游那一档。
    if pr.draft {
        return PrStatus::Draft;
    }
    let ms = pr.mergeable_state.as_str();
    // 冲突是量得出来的事实，比检查更要紧：有冲突的时候检查就算全绿也合不了。
    if ms == "dirty" {
        return PrStatus::Conflicts;
    }
    // CI 轴先说话。
    match &pr.ci {
        Ci::Running { failed, .. } => return PrStatus::CiRunning { failed: *failed },
        Ci::Failed { failed } => {
            let n = failed.len() as u32;
            // 平台自己说了这些失败不挡路，就先别把它们当成本次改动的问题。
            return if matches!(ms, "unstable" | "has_warnings") {
                PrStatus::CiWarned { n }
            } else if ctx.failing_elsewhere {
                PrStatus::CiFailedShared { n }
            } else {
                PrStatus::CiFailed { n }
            };
        }
        _ => {}
    }
    if let Some(done) = speaks_for_itself(ms) {
        return done;
    }
    if !matches!(&pr.ci, Ci::Passed { .. } | Ci::NoChecks) {
        return if ctx.fetching { PrStatus::Fetching } else { PrStatus::Unknown };
    }
    // 到这里检查已知且没坏，合并轴才有资格说话。
    match ms {
        "blocked" => PrStatus::AwaitingReview,
        // 一个检查都没有、平台又没给结论：这句比「还没算出来」有用。
        _ if matches!(&pr.ci, Ci::NoChecks) => PrStatus::CiNone,
        _ => PrStatus::Unknown,
    }
}

pub(crate) fn err(msg: impl Into<String>) -> GitxError {
    GitxError::Other(msg.into())
}

/// Most provider list endpoints wrap arrays; accept both shapes so a slightly
/// different response never shows up as an empty list.
pub(crate) fn list_values<'a>(
    v: &'a serde_json::Value,
    keys: &[&str],
) -> Vec<&'a serde_json::Value> {
    if let Some(arr) = v.as_array() {
        return arr.iter().collect();
    }
    for k in keys {
        if let Some(arr) = v.get(*k).and_then(|x| x.as_array()) {
            return arr.iter().collect();
        }
    }
    vec![]
}

/// Read a JSON value as a string, trying each key path in turn. A path may
/// descend with dots (`"user.login"`), so one parser covers providers that nest
/// the same data differently.
pub(crate) fn field(item: &serde_json::Value, keys: &[&str]) -> String {
    for k in keys {
        let mut found = Some(item);
        for seg in k.split('.') {
            found = found.and_then(|v| v.get(seg));
        }
        if let Some(v) = found {
            if let Some(t) = v.as_str() {
                if !t.is_empty() {
                    return t.to_string();
                }
            }
            if let Some(n) = v.as_i64() {
                return n.to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const URLS: &[(&str, &str, &str)] = &[
        ("git@github.com:acme/app.git", "github.com", "acme/app"),
        ("https://github.com/acme/app.git", "github.com", "acme/app"),
        (
            "https://oauth2:tok@github.com/acme/app",
            "github.com",
            "acme/app",
        ),
        ("https://gitee.com/acme/app.git", "gitee.com", "acme/app"),
        (
            "https://codeup.aliyun.com/61/infra/svc.git",
            "codeup.aliyun.com",
            "61/infra/svc",
        ),
    ];

    #[test]
    fn parses_scp_and_https_remotes() {
        for &(url, host, path) in URLS {
            let r = parse_remote(url).unwrap_or_else(|| panic!("failed on {}", url));
            assert_eq!((r.host.as_str(), r.path.as_str()), (host, path));
        }
    }

    #[test]
    fn rejects_urls_that_are_not_owner_repo() {
        assert!(parse_remote("").is_none());
        assert!(parse_remote("https://github.com/acme").is_none());
        assert!(parse_remote("not a url").is_none());
    }

    #[test]
    fn detects_the_matching_provider() {
        assert_eq!(
            detect(&["git@github.com:a/b.git".into()]).unwrap().id(),
            "github"
        );
        assert_eq!(
            detect(&["https://gitee.com/a/b.git".into()]).unwrap().id(),
            "gitee"
        );
        assert_eq!(
            detect(&["https://codeup.aliyun.com/o/g/b.git".into()])
                .unwrap()
                .id(),
            "codeup"
        );
        // An unknown host must fall through, not be forced into GitHub.
        assert!(detect(&["https://gitlab.example.com/a/b.git".into()]).is_none());
        assert!(detect(&[]).is_none());
    }

    #[test]
    fn repo_ref_comes_from_the_url_the_provider_matched() {
        let urls = vec![
            "https://gitee.com/x/y.git".to_string(),
            "git@github.com:a/b.git".to_string(),
        ];
        // Detection walks the remotes in order, so whichever URL comes first wins.
        assert_eq!(detect(&urls).unwrap().id(), "gitee");
        // …but each provider still reads its own owner/repo, never the other's.
        let gh = repo_ref_for(&GitHub, &urls).unwrap();
        assert_eq!((gh.host.as_str(), gh.path.as_str()), ("github.com", "a/b"));
        let ee = repo_ref_for(&Gitee, &urls).unwrap();
        assert_eq!((ee.host.as_str(), ee.path.as_str()), ("gitee.com", "x/y"));
        assert!(repo_ref_for(&Codeup, &urls).is_none());
    }

    fn pr(state: &str, draft: bool, ms: &str) -> PullRequest {
        PullRequest {
            number: 1,
            title: String::new(),
            state: state.into(),
            author: String::new(),
            head: String::new(),
            base: String::new(),
            head_sha: "0123456789abcdef".into(),
            draft,
            url: String::new(),
            updated: String::new(),
            mergeable_state: ms.into(),
            ci: Ci::NotFetched,
        }
    }

    fn ctx(fetching: bool, elsewhere: bool) -> StatusContext {
        StatusContext { fetching, failing_elsewhere: elsewhere }
    }

    /// 每一档**有名字的**状态都得有自己的落点，且两两不同：只验「函数返回个什么」的判据，
    /// 把十几档全返回同一句也照样绿（PROCESS 里那条「一个值两条出口只验一半」）。
    /// 认不出来的取值是**故意**共用兜底句的，所以它们单独一组、只验都落进那一句。
    ///
    /// 中文与英文**各跑一遍**「两两不同」：只断中文的话，把英文那十几句全写成同一句话
    /// 也照样绿——那正是「翻译表看着填满了，其实是一格」。
    #[test]
    fn every_status_has_its_own_one_line_sentence() {
        let named = [
            (pr("open", false, "dirty"), PrStatus::Conflicts),
            // `blocked` 单独靠不住：它可能是缺评审，也可能是缺一个还没跑完的检查。
            // 只有检查已知（这里：全绿）才许说「等待评审」。
            (
                PullRequest { ci: Ci::Passed { total: 5 }, ..pr("open", false, "blocked") },
                PrStatus::AwaitingReview,
            ),
            (pr("open", false, "behind"), PrStatus::Behind),
            (pr("open", false, "clean"), PrStatus::MergeReady),
            (pr("closed", false, "clean"), PrStatus::Closed),
            (pr("merged", false, "clean"), PrStatus::Merged),
            // 草稿盖过 `blocked`：GitHub 对草稿 PR 也给 blocked，但那不是它卡住的原因。
            (pr("open", true, "blocked"), PrStatus::Draft),
            // 草稿也盖过冲突：草稿挡住评审**和**合并，冲突只挡住合并——取最上游那一档。
            (pr("open", true, "dirty"), PrStatus::Draft),
            // 检查在跑的时候，`blocked` 到底是检查挡的还是评审挡的分不出来，所以检查先说话。
            (
                PullRequest { ci: Ci::Running { passed: 3, failed: 0, pending: 2 }, ..pr("open", false, "blocked") },
                PrStatus::CiRunning { failed: 0 },
            ),
            (
                PullRequest { ci: Ci::Running { passed: 1, failed: 2, pending: 4 }, ..pr("open", false, "blocked") },
                PrStatus::CiRunning { failed: 2 },
            ),
            // 检查全绿，合并轴才有资格说话。
            (
                PullRequest { ci: Ci::Passed { total: 7 }, ..pr("open", false, "blocked") },
                PrStatus::AwaitingReview,
            ),
            (
                PullRequest { ci: Ci::Passed { total: 7 }, ..pr("open", false, "clean") },
                PrStatus::MergeReady,
            ),
            // 一个检查都没有、平台又没给合并状态：顶到前面说「还没有检查」，
            // 这句比「还没算出来」有用（它就是「CI not started」那一档）。
            (
                PullRequest { ci: Ci::NoChecks, ..pr("open", false, "") },
                PrStatus::CiNone,
            ),
            // 但平台说了 `blocked` 就是等评审，不是等 CI——不许把没有检查当成挡路的那件事。
            (
                PullRequest { ci: Ci::NoChecks, ..pr("open", false, "blocked") },
                PrStatus::AwaitingReview,
            ),
            // GitHub 说这些失败不挡路，就先别把它们当本次改动的问题。
            (
                PullRequest {
                    ci: Ci::Failed { failed: vec!["lint".into()] },
                    ..pr("open", false, "unstable")
                },
                PrStatus::CiWarned { n: 1 },
            ),
            // 没给 `mergeable_state` 时不替 GitHub 下「不挡路」这个结论。
            (
                PullRequest {
                    ci: Ci::Failed { failed: vec!["build".into(), "test".into()] },
                    ..pr("open", false, "")
                },
                PrStatus::CiFailed { n: 2 },
            ),
            // 同一个 job 在别条 PR 上也挂着 → 换一句，且这句**不带**「几个检查没通过」那半截。
            (
                PullRequest {
                    ci: Ci::Failed { failed: vec!["build".into()] },
                    ..pr("open", false, "blocked")
                },
                PrStatus::CiFailed { n: 1 },
            ),
            // 在飞时说「正在看」，不飞时说「还没算出来」——同一格数据，两句假话各一半。
            (pr("open", false, ""), PrStatus::Unknown),
        ];
        for (p, want) in named {
            assert_eq!(
                pr_status(&p, &ctx(false, false)),
                want,
                "state={} draft={} ms={} ci={:?}",
                p.state,
                p.draft,
                p.mergeable_state,
                p.ci
            );
        }
        // 在飞那一档：只有 `Ci` 还没落地时才允许说「正在看」；详情已经给了 `clean`
        // 就别再转圈。
        assert_eq!(pr_status(&pr("open", false, ""), &ctx(true, false)), PrStatus::Fetching);
        assert_eq!(
            pr_status(
                &PullRequest { ci: Ci::Unknown, ..pr("open", false, "") },
                &ctx(true, false)
            ),
            PrStatus::Fetching
        );
        assert_eq!(
            pr_status(&pr("open", false, "clean"), &ctx(true, false)),
            PrStatus::MergeReady,
            "详情已经说了「什么都不缺」还在转圈：检查那一次根本不必等"
        );
        // `blocked` 是这一批唯一**必须**等检查的那一格。不飞了也没取到 → 说实话，不猜评审。
        let ambiguous = pr("open", false, "blocked");
        assert_eq!(pr_status(&ambiguous, &ctx(true, false)), PrStatus::Fetching);
        assert_eq!(
            pr_status(&ambiguous, &ctx(false, false)),
            PrStatus::Unknown,
            "检查没取到就说「等待评审」——万一是检查挡着呢，这句把活派给了错的人"
        );
        // 非阻塞失败但检查那一次没给数字：句子不许写成「0 个检查没过」。
        assert_eq!(
            pr_status(&pr("open", false, "unstable"), &ctx(false, false)).label(Lang::Zh),
            "有检查没过 · 但不挡合并"
        );
        // 旁证那半：只有正向证据才改口，没证据不许说「大概无关」。
        let flaky = PullRequest {
            ci: Ci::Failed { failed: vec!["build".into()] },
            ..pr("open", false, "blocked")
        };
        assert_eq!(pr_status(&flaky, &ctx(false, true)), PrStatus::CiFailedShared { n: 1 });

        // 具名档 + 兜底句：每一档在每一枚语言下互不相同（兜底句也算一档，它不该和谁撞）。
        let all = [
            PrStatus::Merged,
            PrStatus::Closed,
            PrStatus::Draft,
            PrStatus::Conflicts,
            PrStatus::Fetching,
            PrStatus::CiRunning { failed: 0 },
            PrStatus::CiRunning { failed: 2 },
            PrStatus::CiFailedShared { n: 1 },
            PrStatus::CiFailed { n: 1 },
            PrStatus::CiWarned { n: 1 },
            PrStatus::CiNone,
            PrStatus::Behind,
            PrStatus::AwaitingReview,
            PrStatus::MergeReady,
            PrStatus::Unknown,
        ];
        for lang in [Lang::Zh, Lang::En] {
            let mut seen: Vec<String> = Vec::new();
            for s in &all {
                let text = s.label(lang);
                assert!(!text.is_empty(), "{s:?} 在 {lang:?} 下没有句子");
                assert!(
                    !seen.contains(&text),
                    "{lang:?} 下两档共用同一句文案：{text}"
                );
                seen.push(text);
            }
        }
        // 「CI 正在跑」和「已经挂了 2 个」必须是两句：合成一句就丢了这个信息。
        assert_ne!(
            PrStatus::CiRunning { failed: 0 }.label(Lang::Zh),
            PrStatus::CiRunning { failed: 2 }.label(Lang::Zh)
        );
        // 没给的（列表那一格）与给认不出的（将来的新取值）都说「还没算出来」——不猜。
        for ms in ["", "quarantined", "unknown", "DIRTY"] {
            assert_eq!(pr_status(&pr("open", false, ms), &ctx(false, false)), PrStatus::Unknown, "ms={ms}");
        }
    }

    /// 汇总那三件事各钉一条：**空列表**不等于「没取」、在跑压过已报的失败、
    /// 跳过算过。把 `is_empty` 写成「返回 Unknown」，屏上那句「还没有检查」就没了。
    #[test]
    fn aggregates_checks_without_losing_the_three_distinct_cases() {
        let c = |name: &str, outcome| Check { name: name.into(), outcome };
        assert_eq!(aggregate_ci(&[]), Ci::NoChecks);
        assert_eq!(
            aggregate_ci(&[c("a", CheckOutcome::Pass), c("b", CheckOutcome::Pass)]),
            Ci::Passed { total: 2 }
        );
        // 在跑赢：此刻挂了 1 个，但数字还会变，所以这一档是 Running 而不是 Failed。
        assert_eq!(
            aggregate_ci(&[c("a", CheckOutcome::Fail), c("b", CheckOutcome::Pending)]),
            Ci::Running { passed: 0, failed: 1, pending: 1 }
        );
        // 名字要留着（跨 PR 旁证靠它），且排序稳定：不然签名跟着 API 的返回顺序抖。
        assert_eq!(
            aggregate_ci(&[c("zeta", CheckOutcome::Fail), c("alpha", CheckOutcome::Fail), c("m", CheckOutcome::Pass)]),
            Ci::Failed { failed: vec!["alpha".into(), "zeta".into()] }
        );
    }
}
