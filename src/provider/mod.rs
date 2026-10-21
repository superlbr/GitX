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
pub use crate::error::Result;
use crate::models::PullRequest;

pub use codeup::Codeup;
pub use gitee::Gitee;
pub use github::GitHub;

/// What a provider can actually do. The UI renders only what a provider declares,
/// so an unimplemented capability is invisible rather than a dead button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    PullRequests,
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
}
