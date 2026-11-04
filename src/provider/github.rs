use serde_json::Value;

use crate::models::{Check, CheckOutcome, Ci, PullRequest};
use crate::provider::{err, field, list_values, ApiRequest, Capability, Host, RepoRef, Result};
use crate::tr;

pub struct GitHub;

impl GitHub {
    fn api_root(repo: &RepoRef) -> String {
        let host = if repo.host == "github.com" {
            "api.github.com".to_string()
        } else {
            // GitHub Enterprise serves the API under <host>/api/v3.
            format!("{}/api/v3", repo.host)
        };
        format!("https://{}/repos/{}", host, repo.path)
    }
}

impl Host for GitHub {
    fn id(&self) -> &'static str {
        "github"
    }

    fn label(&self) -> &'static str {
        "GitHub"
    }

    fn handles(&self, url: &str) -> bool {
        crate::provider::parse_remote(url)
            .map(|r| r.host == "github.com" || r.host.ends_with(".github.com"))
            .unwrap_or(false)
    }

    fn capabilities(&self) -> Vec<Capability> {
        // 三格都声明：列表、按号的详情（`mergeable_state` 只在这里）、按 sha 的检查。
        vec![
            Capability::PullRequests,
            Capability::PullRequestDetail,
            Capability::CiStatus,
        ]
    }

    fn token_hint(&self) -> &'static str {
        tr!(
            "personal access token（fine-grained 或 classic 均可，需要读仓库与 PR 的权限）",
            "a personal access token (fine-grained or classic), able to read the repo and its pull requests"
        )
    }

    fn pull_requests_request(&self, repo: &RepoRef, token: &str) -> Result<ApiRequest> {
        if token.trim().is_empty() {
            return Err(err(tr!(
                "未绑定 GitHub token，请在「设置 → 代码托管」中填写。",
                "No GitHub token bound — fill one in under Preferences → Hosting."
            )));
        }
        let url = format!(
            "{}/pulls?state=open&per_page=50&sort=updated",
            Self::api_root(repo)
        );
        Ok(ApiRequest::get(url)
            .with_header("authorization", format!("Bearer {}", token.trim()))
            .with_header("accept", "application/vnd.github+json")
            .with_header("user-agent", "gitx"))
    }

    fn parse_pull_requests(&self, body: &str) -> Result<Vec<PullRequest>> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            err(tr!(
                format!("GitHub 响应解析失败：{}", e),
                format!("Cannot parse the GitHub response: {e}")
            ))
        })?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return Err(err(tr!(
                format!("GitHub：{}", msg),
                format!("GitHub answered: {msg}")
            )));
        }
        Ok(list_values(&v, &["items"])
            .into_iter()
            .filter_map(pr_from)
            .collect())
    }

    fn pull_request_detail_request(&self, repo: &RepoRef, token: &str, number: u32) -> Result<ApiRequest> {
        if token.trim().is_empty() {
            return Err(err(tr!(
                "未绑定 GitHub token，请在「设置 → 代码托管」中填写。",
                "No GitHub token bound — fill one in under Preferences → Hosting."
            )));
        }
        Ok(ApiRequest::get(format!("{}/pulls/{}", Self::api_root(repo), number))
            .with_header("authorization", format!("Bearer {}", token.trim()))
            .with_header("accept", "application/vnd.github+json")
            .with_header("user-agent", "gitx"))
    }

    /// 详情端点回的是**一个对象**，不是数组。这里唯一的新东西是 `mergeable_state`
    /// ——列表里没有（2026-10-31 实测），所以看板那一句话必须等这一步。
    /// 其余格子与列表同义，共用 `pr_from`：两份解析迟早会漂移，这里不给它漂移的机会。
    fn parse_pull_request_detail(&self, body: &str) -> Result<PullRequest> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            err(tr!(
                format!("GitHub 响应解析失败：{}", e),
                format!("Cannot parse the GitHub response: {e}")
            ))
        })?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return Err(err(tr!(
                format!("GitHub：{}", msg),
                format!("GitHub answered: {msg}")
            )));
        }
        pr_from(&v).ok_or_else(|| {
            err(tr!(
                "GitHub 的详情响应里没有能认出的 PR（缺 `number`？）",
                "The GitHub detail response has no recognisable pull request (missing `number`?)"
            ))
        })
    }

    fn checks_request(&self, repo: &RepoRef, token: &str, sha: &str) -> Result<ApiRequest> {
        if token.trim().is_empty() {
            return Err(err(tr!(
                "未绑定 GitHub token，请在「设置 → 代码托管」中填写。",
                "No GitHub token bound — fill one in under Preferences → Hosting."
            )));
        }
        Ok(ApiRequest::get(format!(
            "{}/commits/{}/check-runs?per_page=100",
            Self::api_root(repo),
            sha
        ))
        .with_header("authorization", format!("Bearer {}", token.trim()))
        .with_header("accept", "application/vnd.github+json")
        .with_header("user-agent", "gitx"))
    }

    /// check run → 归一的一条。**同名只留 `id` 最大的那一条**：实测 GitHub 会把
    /// 同一个名字返回好几次（vllm 的一次刷新里 `notify` 出现三回），而它的
    /// `completed_at` 比 `started_at` 还早（跳过态就这样），所以时间戳不能当顺序，
    /// 只有 `id` 是单调的。认不出的 conclusion 一律算「还在跑」而不是「过了」——
    /// 把没把握的东西报成绿，比报成黄坏得多。
    fn parse_checks(&self, body: &str) -> Result<Vec<Check>> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            err(tr!(
                format!("GitHub 响应解析失败：{}", e),
                format!("Cannot parse the GitHub response: {e}")
            ))
        })?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return Err(err(tr!(
                format!("GitHub：{}", msg),
                format!("GitHub answered: {msg}")
            )));
        }
        let mut newest: std::collections::HashMap<String, (u64, CheckOutcome)> =
            std::collections::HashMap::new();
        for run in list_values(&v, &["check_runs"]) {
            let name = field(run, &["name"]);
            if name.is_empty() {
                continue;
            }
            let id = run.get("id").and_then(Value::as_u64).unwrap_or(0);
            let outcome = if field(run, &["status"]) != "completed" {
                CheckOutcome::Pending
            } else {
                match field(run, &["conclusion"]).as_str() {
                    "success" | "neutral" | "skipped" => CheckOutcome::Pass,
                    "failure" | "timed_out" | "action_required" | "cancelled" | "stale" => {
                        CheckOutcome::Fail
                    }
                    _ => CheckOutcome::Pending,
                }
            };
            match newest.get(&name) {
                Some((seen, _)) if *seen >= id => {}
                _ => {
                    newest.insert(name, (id, outcome));
                }
            }
        }
        let mut out: Vec<Check> = newest
            .into_iter()
            .map(|(name, (_, outcome))| Check { name, outcome })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn pull_request_web_url(&self, repo: &RepoRef, number: u32) -> String {
        format!("https://{}/{}/pull/{}", repo.host, repo.path, number)
    }
}

/// 一条 PR，列表和详情两个端点的形状在这一格上是同名的，所以共用一份。
/// `None` = 这一条认不出来（没有 `number`），跳过它而不是整批丢——
/// 一家平台多加一种半成品，不该让整屏看板空掉。
fn pr_from(v: &Value) -> Option<PullRequest> {
    let number = v.get("number")?.as_u64()? as u32;
    // 合并了的 PR 在 REST 里 `state` 还是 `closed`，只有 `merged` 那枚布尔分得开。
    // 不接这一格，「已合并」那一档就永远出不来。
    let state = if v.get("merged").and_then(Value::as_bool).unwrap_or(false) {
        "merged".to_string()
    } else {
        field(v, &["state"])
    };
    Some(PullRequest {
        number,
        title: field(v, &["title"]),
        state,
        author: field(v, &["user.login"]),
        head: field(v, &["head.label"]),
        base: field(v, &["base.label"]),
        head_sha: field(v, &["head.sha"]),
        draft: v.get("draft").and_then(Value::as_bool).unwrap_or(false),
        url: field(v, &["html_url"]),
        updated: field(v, &["updated_at"]),
        mergeable_state: field(v, &["mergeable_state"]),
        // 列表与详情都不带检查；那是按 sha 的第三个请求，只有看得见的行才去取。
        ci: Ci::NotFetched,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{parse_remote, Host};

    fn repo() -> RepoRef {
        parse_remote("git@github.com:acme/app.git").unwrap()
    }

    #[test]
    fn builds_an_authenticated_request_and_refuses_without_a_token() {
        let req = GitHub.pull_requests_request(&repo(), "  tok  ").unwrap();
        assert_eq!(
            req.url,
            "https://api.github.com/repos/acme/app/pulls?state=open&per_page=50&sort=updated"
        );
        assert!(req
            .headers
            .contains(&("authorization".to_string(), "Bearer tok".to_string())));
        assert!(GitHub.pull_requests_request(&repo(), "").is_err());
    }

    #[test]
    fn enterprise_hosts_use_the_v3_api() {
        let r = parse_remote("git@github.acme.com:team/app.git").unwrap();
        let req = GitHub.pull_requests_request(&r, "t").unwrap();
        assert!(req
            .url
            .starts_with("https://github.acme.com/api/v3/repos/team/app/pulls"));
    }

    #[test]
    fn parses_fixture_prs_and_surfaces_api_errors() {
        let json = r#"[{"number":12,"title":"加个按钮","state":"open","user":{"login":"lisa"},
            "head":{"label":"acme:feat"},"base":{"label":"acme:main"},"draft":true,
            "html_url":"https://github.com/acme/app/pull/12","updated_at":"2026-10-10T01:00:00Z"}]"#;
        let prs = GitHub.parse_pull_requests(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(
            (prs[0].head.as_str(), prs[0].base.as_str()),
            ("acme:feat", "acme:main")
        );
        assert!(prs[0].draft);

        // an error response is an object with a message, not a list
        let err = GitHub
            .parse_pull_requests(r#"{"message":"Bad credentials"}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Bad credentials"), "{}", err);
    }

    #[test]
    fn declares_its_capabilities_and_web_url() {
        assert_eq!(
            GitHub.capabilities(),
            vec![
                Capability::PullRequests,
                Capability::PullRequestDetail,
                Capability::CiStatus
            ]
        );
        assert_eq!(
            GitHub.pull_request_web_url(&repo(), 12),
            "https://github.com/acme/app/pull/12"
        );
    }

    /// 这一批的**起因**：2026-10-31 拿真 API 核过，`/pulls` 那一片段里**没有**
    /// `mergeable_state`，`/pulls/{号}` 才有。所以「按号取详情」不是锦上添花，
    /// 看板那一句话全靠它。夹具按**实测过的字段集**写，不按想象写。
    #[test]
    fn the_detail_endpoint_is_where_mergeable_state_actually_lives() {
        let req = GitHub.pull_request_detail_request(&repo(), "tok", 64900).unwrap();
        assert_eq!(
            req.url,
            "https://api.github.com/repos/acme/app/pulls/64900"
        );
        // Enterprise 那一档不能漏：列表走的是同一个 api_root。
        let ent = crate::provider::parse_remote("git@github.acme.com:team/app.git").unwrap();
        assert!(GitHub
            .pull_request_detail_request(&ent, "t", 3)
            .unwrap()
            .url
            .starts_with("https://github.acme.com/api/v3/repos/team/app/pulls/3"));
        assert!(GitHub.pull_request_detail_request(&repo(), "", 1).is_err());

        let detail = r#"{"number":64900,"state":"open","merged":false,"draft":false,
            "title":"跑通它","user":{"login":"lisa"},"head":{"label":"acme:feat","sha":"2fd8dd57"},
            "base":{"label":"acme:main"},"html_url":"https://github.com/acme/app/pull/64900",
            "updated_at":"2026-10-31T16:47:24Z","mergeable":true,"mergeable_state":"unstable"}"#;
        let pr = GitHub.parse_pull_request_detail(detail).unwrap();
        assert_eq!(pr.mergeable_state, "unstable", "详情端点唯一的新格子没接上");
        assert_eq!(pr.head_sha, "2fd8dd57");
        // 详情**不带**检查：那一格留给按 sha 的第三个请求，别在这儿假装它是「没有检查」。
        assert_eq!(pr.ci, Ci::NotFetched);

        // 列表里没有这一格——这正是必须打详情的理由，判据钉住它免得将来「顺手」退回列表。
        let listed = r#"[{"number":64900,"state":"open","draft":false,"title":"跑通它",
            "user":{"login":"lisa"},"head":{"label":"acme:feat","sha":"2fd8dd57"},
            "base":{"label":"acme:main"},"html_url":"https://x","updated_at":"2026-10-31T16:47:24Z"}]"#;
        let one = &GitHub.parse_pull_requests(listed).unwrap()[0];
        assert_eq!(one.mergeable_state, "", "列表要是给了这一格，注释与代码就该一起改");
        assert_eq!(one.head_sha, "2fd8dd57", "列表给 sha：按 sha 取检查不必先等详情");
    }

    /// 合并了的 PR 在 REST 里 `state` 仍是 `closed`，只有 `merged` 那枚布尔分得开。
    /// 不读它，「已合并」那一档在这家平台上永远出不来——而它是个**永远**，
    /// 靠一条断言钉住比靠印象留着便宜得多。
    #[test]
    fn reads_merged_out_of_the_boolean_not_the_state_string() {
        let json = r#"{"number":9,"state":"closed","merged":true,"title":"过了"}"#;
        assert_eq!(GitHub.parse_pull_request_detail(json).unwrap().state, "merged");
        let closed = r#"{"number":9,"state":"closed","merged":false,"title":"没过"}"#;
        assert_eq!(GitHub.parse_pull_request_detail(closed).unwrap().state, "closed");
    }

    /// 夹具是**实测形状**：同名多条（`notify` ×3）、`completed_at` 比 `started_at` 还早、
    /// 有 skipped。少留一条判据，将来就会有人拿时间戳排序、把最新那版算成旧的。
    #[test]
    fn keeps_the_newest_run_per_name_and_refuses_to_guess_conclusions() {
        let req = GitHub.checks_request(&repo(), "tok", "abc123").unwrap();
        assert_eq!(
            req.url,
            "https://api.github.com/repos/acme/app/commits/abc123/check-runs?per_page=100"
        );
        let body = r#"{"total_count":6,"check_runs":[
            {"name":"notify","id":30,"status":"completed","conclusion":"skipped",
             "started_at":"2026-10-31T16:46:39Z","completed_at":"2026-10-31T16:46:31Z"},
            {"name":"notify","id":20,"status":"completed","conclusion":"failure"},
            {"name":"build","id":10,"status":"in_progress"},
            {"name":"lint","id":11,"status":"completed","conclusion":"brand_new_value"},
            {"name":"deploy","id":40,"status":"completed","conclusion":"cancelled"},
            {"name":"cleanup","id":41,"status":"completed","conclusion":"stale"}
        ]}"#;
        let checks = GitHub.parse_checks(body).unwrap();
        let got: Vec<(String, CheckOutcome)> =
            checks.into_iter().map(|c| (c.name, c.outcome)).collect();
        assert_eq!(
            got,
            vec![
                ("build".into(), CheckOutcome::Pending),
                ("cleanup".into(), CheckOutcome::Fail),
                ("deploy".into(), CheckOutcome::Fail),
                ("lint".into(), CheckOutcome::Pending),
                ("notify".into(), CheckOutcome::Pass),
            ],
            "同名要按 id 取最新那版（30 > 20，跳过算过）；认不出的 conclusion 不许报成绿；\
             `cancelled` / `stale` 是**挂了**而不是「没跑」"
        );
        // 空的 check_runs 是「真的没有」，不是解析失败——汇总那一层靠这一格分档。
        assert!(GitHub.parse_checks(r#"{"total_count":0,"check_runs":[]}"#).unwrap().is_empty());
        assert!(GitHub
            .parse_checks(r#"{"message":"Not Found"}"#)
            .unwrap_err()
            .to_string()
            .contains("Not Found"));
    }
}
