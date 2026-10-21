//! Fast read-only source preview: turn a repo's remote URL into a browser-based
//! code reader (github1s) so reviewing someone else's repo never needs a clone.
//!
//! Pure string work — the UI layer only opens the URL it hands back.

use crate::provider::{err, parse_remote};
use crate::Result;

const GITHUB1S: &str = "https://github1s.com";
const GITHUB_HOST: &str = "github.com";

/// github1s reads github.com's own URL grammar, so this is a host swap plus the
/// same `/tree/<ref>` and `/blob/<ref>/<path>` tail GitHub uses. `branch` and
/// `path` are optional: repo root → tree at a ref → one file at a ref.
pub fn github1s_url(remote_url: &str, branch: Option<&str>, path: Option<&str>) -> Result<String> {
    let rr = parse_remote(remote_url)
        .ok_or_else(|| err("无法从 remote 地址解析出 owner/repo，这个仓库可能还没有远程。"))?;
    if rr.host != GITHUB_HOST {
        return Err(err(format!(
            "极速预览目前只支持 github.com，当前远程在 {} 上。",
            rr.host
        )));
    }
    let branch = branch.map(str::trim).filter(|b| !b.is_empty());
    let path = path.map(str::trim).filter(|p| !p.is_empty());
    let mut url = format!("{}/{}", GITHUB1S, rr.path);
    match (branch, path) {
        (Some(b), Some(p)) => url.push_str(&format!("/blob/{}/{}", b, p.trim_start_matches('/'))),
        (Some(b), None) => url.push_str(&format!("/tree/{}", b)),
        // github1s locates a file only as `/blob/<ref>/<path>`, so without a ref
        // there is nothing to point at: fall back to the repo root, not a 404.
        (None, _) => {}
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(remote: &str, branch: Option<&str>, path: Option<&str>) -> String {
        github1s_url(remote, branch, path).unwrap()
    }

    #[test]
    fn swaps_the_host_and_keeps_githubs_own_paths() {
        assert_eq!(
            url("git@github.com:acme/app.git", None, None),
            "https://github1s.com/acme/app"
        );
        assert_eq!(
            url("https://github.com/acme/app.git", Some("main"), None),
            "https://github1s.com/acme/app/tree/main"
        );
        assert_eq!(
            url(
                "https://github.com/acme/app",
                Some("main"),
                Some("src/lib.rs")
            ),
            "https://github1s.com/acme/app/blob/main/src/lib.rs"
        );
    }

    #[test]
    fn blank_branch_or_path_falls_back_to_the_shallower_form() {
        for blank in ["", "   "] {
            // No ref means no `/blob/<ref>/…`: the root is what we can honour.
            assert_eq!(
                url("https://github.com/a/b.git", Some(blank), Some("x")),
                "https://github1s.com/a/b"
            );
            assert_eq!(
                url("https://github.com/a/b.git", Some("main"), Some(blank)),
                "https://github1s.com/a/b/tree/main"
            );
            assert_eq!(
                url("https://github.com/a/b.git", Some(blank), Some(blank)),
                "https://github1s.com/a/b"
            );
        }
    }

    #[test]
    fn credentials_and_stray_slashes_do_not_leak_into_the_url() {
        assert_eq!(
            url(
                "https://oauth2:tok@github.com/acme/app/",
                Some("dev"),
                Some("/src/a.ts")
            ),
            "https://github1s.com/acme/app/blob/dev/src/a.ts"
        );
    }

    #[test]
    fn refuses_hosts_github1s_cannot_read() {
        let e = github1s_url("https://gitee.com/a/b.git", None, None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("github.com") && e.contains("gitee.com"), "{}", e);
        // A self-hosted GitHub is still not github.com, so github1s would 404.
        let e = github1s_url("https://git.corp.example.com/a/b.git", None, None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("git.corp.example.com"), "{}", e);
    }

    #[test]
    fn refuses_urls_that_are_not_owner_repo() {
        for bad in [
            "",
            "https://github.com",
            "https://github.com/only-owner.git",
            "not a url",
        ] {
            assert!(github1s_url(bad, None, None).is_err(), "accepted {:?}", bad);
        }
    }
}
