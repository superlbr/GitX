use crate::models::PullRequest;
use crate::provider::{err, ApiRequest, Capability, Host, RepoRef, Result};

/// Recognised but not wired up: Codeup's OpenAPI signs requests with an Alibaba
/// Cloud AK/SK pair (plus an organization id), which is a different credential
/// model from a personal token, so it stays opt-out until that is designed.
pub struct Codeup;

impl Host for Codeup {
    fn id(&self) -> &'static str {
        "codeup"
    }

    fn label(&self) -> &'static str {
        "阿里云 Codeup"
    }

    fn handles(&self, url: &str) -> bool {
        crate::provider::parse_remote(url)
            .map(|r| r.host.ends_with("codeup.aliyun.com") || r.host == "codeup.aliyun.com")
            .unwrap_or(false)
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![]
    }

    fn token_hint(&self) -> &'static str {
        "暂不支持：Codeup OpenAPI 需要阿里云 AK/SK 签名与 organizationId，尚未接入"
    }

    fn pull_requests_request(&self, _repo: &RepoRef, _token: &str) -> Result<ApiRequest> {
        Err(err(self.token_hint()))
    }

    fn parse_pull_requests(&self, _body: &str) -> Result<Vec<PullRequest>> {
        Err(err(self.token_hint()))
    }

    fn pull_request_web_url(&self, repo: &RepoRef, number: u32) -> String {
        format!("https://{}/{}/change/{}", repo.host, repo.path, number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Host;

    /// Codeup is registered so the repo is recognised, but it must offer nothing
    /// rather than a half-implemented signer.
    #[test]
    fn is_recognised_but_declares_no_capabilities() {
        assert!(Codeup.handles("git@codeup.aliyun.com:61/infra/svc.git"));
        assert_eq!(Codeup.capabilities(), vec![]);
        let r = crate::provider::parse_remote("git@codeup.aliyun.com:61/infra/svc.git").unwrap();
        let err = Codeup
            .pull_requests_request(&r, "t")
            .unwrap_err()
            .to_string();
        assert!(err.contains("尚未接入"), "{}", err);
        assert_eq!(
            Codeup.pull_request_web_url(&r, 3),
            "https://codeup.aliyun.com/61/infra/svc/change/3"
        );
    }
}
