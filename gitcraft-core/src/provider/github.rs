use serde_json::Value;

use crate::models::PullRequest;
use crate::provider::{err, field, list_values, ApiRequest, Capability, Host, RepoRef, Result};

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
        vec![Capability::PullRequests]
    }

    fn token_hint(&self) -> &'static str {
        "personal access token（fine-grained 或 classic 均可，需要读仓库与 PR 的权限）"
    }

    fn pull_requests_request(&self, repo: &RepoRef, token: &str) -> Result<ApiRequest> {
        if token.trim().is_empty() {
            return Err(err("未绑定 GitHub token，请在「设置 → 代码托管」中填写。"));
        }
        let url = format!(
            "{}/pulls?state=open&per_page=50&sort=updated",
            Self::api_root(repo)
        );
        Ok(ApiRequest::get(url)
            .with_header("authorization", format!("Bearer {}", token.trim()))
            .with_header("accept", "application/vnd.github+json")
            .with_header("user-agent", "gitcraft"))
    }

    fn parse_pull_requests(&self, body: &str) -> Result<Vec<PullRequest>> {
        let v: Value =
            serde_json::from_str(body).map_err(|e| err(format!("GitHub 响应解析失败：{}", e)))?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return Err(err(format!("GitHub：{}", msg)));
        }
        Ok(list_values(&v, &["items"])
            .into_iter()
            .filter_map(|pr| {
                let number = pr.get("number")?.as_u64()? as u32;
                Some(PullRequest {
                    number,
                    title: field(pr, &["title"]),
                    state: field(pr, &["state"]),
                    author: field(pr, &["user.login"]),
                    head: field(pr, &["head.label"]),
                    base: field(pr, &["base.label"]),
                    draft: pr.get("draft").and_then(|d| d.as_bool()).unwrap_or(false),
                    url: field(pr, &["html_url"]),
                    updated: field(pr, &["updated_at"]),
                })
            })
            .collect())
    }

    fn pull_request_web_url(&self, repo: &RepoRef, number: u32) -> String {
        format!("https://{}/{}/pull/{}", repo.host, repo.path, number)
    }
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
        assert_eq!(GitHub.capabilities(), vec![Capability::PullRequests]);
        assert_eq!(
            GitHub.pull_request_web_url(&repo(), 12),
            "https://github.com/acme/app/pull/12"
        );
    }
}
