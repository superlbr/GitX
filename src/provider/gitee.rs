use serde_json::Value;

use crate::models::{Ci, PullRequest};
use crate::provider::{err, field, list_values, ApiRequest, Capability, Host, RepoRef, Result};
use crate::tr;

pub struct Gitee;

impl Host for Gitee {
    fn id(&self) -> &'static str {
        "gitee"
    }

    fn label(&self) -> &'static str {
        "Gitee"
    }

    fn handles(&self, url: &str) -> bool {
        crate::provider::parse_remote(url)
            .map(|r| r.host == "gitee.com")
            .unwrap_or(false)
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::PullRequests]
    }

    fn token_hint(&self) -> &'static str {
        tr!(
            "私人令牌（个人设置 → 私人令牌，勾选 projects 权限）",
            "a private token (personal settings → private tokens, tick the projects scope)"
        )
    }

    fn pull_requests_request(&self, repo: &RepoRef, token: &str) -> Result<ApiRequest> {
        if token.trim().is_empty() {
            return Err(err(tr!(
                "未绑定 Gitee 令牌，请在「设置 → 代码托管」中填写。",
                "No Gitee token bound — fill one in under Preferences → Hosting."
            )));
        }
        // Gitee v5 takes the token as a query parameter, not a header.
        let url = format!(
            "https://gitee.com/api/v5/repos/{}/pulls?state=open&per_page=50&access_token={}",
            repo.path,
            urlencode(token.trim())
        );
        Ok(ApiRequest::get(url).with_header("user-agent", "gitx"))
    }

    fn parse_pull_requests(&self, body: &str) -> Result<Vec<PullRequest>> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            err(tr!(
                format!("Gitee 响应解析失败：{}", e),
                format!("Cannot parse the Gitee response: {e}")
            ))
        })?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return Err(err(tr!(
                format!("Gitee：{}", msg),
                format!("Gitee answered: {msg}")
            )));
        }
        Ok(list_values(&v, &["pull_requests", "data"])
            .into_iter()
            .filter_map(|pr| {
                let number = pr.get("number")?.as_u64()? as u32;
                Some(PullRequest {
                    number,
                    title: field(pr, &["title"]),
                    state: field(pr, &["state"]),
                    author: field(pr, &["user.login"]),
                    head: field(pr, &["head.ref"]),
                    base: field(pr, &["base.ref"]),
                    head_sha: field(pr, &["head.sha"]),
                    draft: pr.get("draft").and_then(Value::as_bool).unwrap_or(false),
                    url: field(pr, &["html_url"]),
                    updated: field(pr, &["updated_at"]),
                    // Gitee 的列表里没有 `mergeable_state` 这一格（它在单条 PR 的详情里，
                    // 而且叫 `mergeable` 布尔）。列表留空串：看板那一行退成「还没算出来」，
                    // 不猜。**GitHub 的列表同样不给这一格**（2026-10-31 实测），
                    // 所以这一格在两家都是「等详情那一次」。
                    mergeable_state: field(pr, &["mergeable_state"]),
                    ci: Ci::NotFetched,
                })
            })
            .collect())
    }

    fn pull_request_web_url(&self, repo: &RepoRef, number: u32) -> String {
        format!(
            "https://{}/{}/pullrequests/{}",
            repo.host, repo.path, number
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{parse_remote, Host};

    fn repo() -> RepoRef {
        parse_remote("https://gitee.com/acme/app.git").unwrap()
    }

    #[test]
    fn puts_a_percent_encoded_token_in_the_query() {
        let req = Gitee.pull_requests_request(&repo(), "a b&c").unwrap();
        assert!(req
            .url
            .starts_with("https://gitee.com/api/v5/repos/acme/app/pulls?state=open&per_page=50"));
        assert!(req.url.contains("access_token=a%20b%26c"), "{}", req.url);
        assert!(Gitee.pull_requests_request(&repo(), " ").is_err());
    }

    #[test]
    fn parses_the_wrapped_list_and_nested_branch_names() {
        let json = r#"{"pull_requests":[{"number":7,"title":"修复登录","state":"open",
            "user":{"login":"lisa"},"head":{"ref":"feat/a"},"base":{"ref":"master"},
            "html_url":"https://gitee.com/acme/app/pullrequests/7","updated_at":"2026-10-10T01:00:00+08:00"}]}"#;
        let prs = Gitee.parse_pull_requests(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(
            (prs[0].head.as_str(), prs[0].base.as_str()),
            ("feat/a", "master")
        );
        assert!(!prs[0].draft);
        assert_eq!(
            Gitee.pull_request_web_url(&repo(), 7),
            "https://gitee.com/acme/app/pullrequests/7"
        );
    }

    #[test]
    fn only_handles_gitee() {
        assert!(Gitee.handles("git@gitee.com:a/b.git"));
        assert!(!Gitee.handles("git@github.com:a/b.git"));
    }

    /// 没接的两格必须**明说不接**，而不是回一个空对象让看板当成「状态是干净的」。
    /// 默认实现回 Err，且 `capabilities()` 里没有它们——外壳靠后者决定发不发请求。
    #[test]
    fn refuses_the_endpoints_it_has_not_wired_up() {
        assert_eq!(Gitee.capabilities(), vec![Capability::PullRequests]);
        let e = Gitee
            .pull_request_detail_request(&repo(), "tok", 7)
            .unwrap_err()
            .to_string();
        assert!(e.contains("没有接"), "{}", e);
        assert!(Gitee.checks_request(&repo(), "tok", "abc").unwrap_err().to_string().contains("没有接"));
    }
}

/// Providers take secrets through the query string, so percent-encode them.
fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}
