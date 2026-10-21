//! Application updates: read a release answer and decide whether it is newer
//! than the running build. Like the provider layer, this module never touches
//! the network — the UI layer fetches a URL and hands the text here.

use std::cmp::Ordering;

use serde_json::Value;

use crate::error::{GitcraftError, Result};
use crate::models::{Asset, Release};

/// A release repo is exactly `owner/repo`; anything else can only be a typo, and
/// validating here keeps the string out of a URL it would silently 404 on.
pub fn check_slug(slug: &str) -> Result<()> {
    let slug = slug.trim();
    let parts: Vec<&str> = slug.split('/').collect();
    if parts.len() != 2 || parts.iter().any(|p| p.is_empty()) {
        return Err(GitcraftError::Other(
            "发布仓库要写成 owner/repo，例如 t8y2/dbx。".into(),
        ));
    }
    for p in parts {
        let ok = p
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !ok {
            return Err(GitcraftError::Other(format!(
                "仓库名「{}」里有不支持的字符，只能用字母、数字、-、_、.。",
                p
            )));
        }
    }
    Ok(())
}

/// GitHub's own release API. It is the default, not the only option: from the
/// mainland it is often unreachable, which would make 「检查更新」 fail forever.
pub const DEFAULT_API_BASE: &str = "https://api.github.com";

/// The endpoint for the newest non-draft release.
///
/// `base` is what the user typed in 设置 → 更新. It is either another API host
/// (`https://mirror.example/api`, whose path is kept) or a node template
/// containing `{url}`, which receives the official URL — the same two shapes the
/// clone nodes accept, so one setting never surprises them twice.
pub fn release_api_url(slug: &str, base: &str) -> Result<String> {
    check_slug(slug)?;
    let official = format!("{}/repos/{}/releases/latest", DEFAULT_API_BASE, slug.trim());
    let b = base.trim();
    if b.is_empty() || b == DEFAULT_API_BASE {
        return Ok(official);
    }
    if !b.starts_with("https://") && !b.starts_with("http://") {
        return Err(GitcraftError::Other(format!(
            "更新检查地址「{}」要以 https:// 或 http:// 开头。",
            b
        )));
    }
    if b.chars().any(char::is_whitespace) {
        return Err(GitcraftError::Other("更新检查地址不能含空格。".into()));
    }
    if b.contains("{url}") {
        return Ok(crate::git::apply_node_template(b, &official));
    }
    Ok(format!(
        "{}/repos/{}/releases/latest",
        b.trim_end_matches('/'),
        slug.trim()
    ))
}

/// Parse GitHub's `/releases/latest` response.
pub fn parse_release(text: &str) -> Result<Release> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| GitcraftError::Other(format!("发布信息的 JSON 解析失败: {}", e)))?;
    if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
        return Err(GitcraftError::Other(format!("GitHub 返回：{}", msg)));
    }
    let get = |k: &str| -> String {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let version = get("tag_name");
    if version.is_empty() {
        return Err(GitcraftError::Other(
            "发布信息里没有 tag_name，无法判断版本。".into(),
        ));
    }
    let assets = v
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let name = a.get("name").and_then(|n| n.as_str())?;
                    let url = a.get("browser_download_url").and_then(|u| u.as_str())?;
                    Some(Asset {
                        name: name.to_string(),
                        url: url.to_string(),
                        size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Release {
        version,
        notes: get("body"),
        url: get("html_url"),
        date: get("published_at"),
        assets,
    })
}

/// Signature and checksum files sit next to the real artifacts on a release and
/// are useless to a person clicking 「下载」.
const SIDECARS: &[&str] = &[".sig", ".sha256", ".sha512", ".asc"];

/// Installer kinds per OS, most convenient first.
fn preferred_exts(os: &str) -> &'static [&'static str] {
    match os {
        "macos" => &[".dmg", ".zip"],
        "windows" => &["-setup.exe", ".msi", ".exe", ".zip"],
        "linux" => &[".appimage", ".deb", ".rpm", ".tar.gz"],
        _ => &[],
    }
}

/// Which name fragments mean "this build runs here" and which mean "it does not".
fn arch_tokens(arch: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match arch {
        "aarch64" | "arm64" | "arm" => (&["arm64", "aarch64"], &["x64", "x86_64", "amd64", "i686"]),
        "x86_64" | "x64" | "amd64" => (&["x64", "x86_64", "amd64"], &["arm64", "aarch64"]),
        _ => (&[], &[]),
    }
}

fn score(name: &str, want: &[&str]) -> i32 {
    let n = name.to_lowercase();
    let mut s = 0;
    if want.iter().any(|t| n.contains(t)) {
        s += 100;
    }
    // An offline bundle is many times the size of the ordinary installer.
    if n.contains("offline") {
        s -= 50;
    }
    s
}

/// The one artifact this machine should download, or `None` when the release
/// carries nothing usable (unknown OS, no installer attached).
pub fn pick_asset<'a>(release: &'a Release, os: &str, arch: &str) -> Option<&'a Asset> {
    let (want, avoid) = arch_tokens(arch);
    let pool: Vec<&Asset> = release
        .assets
        .iter()
        .filter(|a| {
            let n = a.name.to_lowercase();
            !SIDECARS.iter().any(|s| n.ends_with(s)) && !avoid.iter().any(|t| n.contains(t))
        })
        .collect();
    preferred_exts(os).iter().find_map(|ext| {
        let mut tier: Vec<&Asset> = pool
            .iter()
            .copied()
            .filter(|a| a.name.to_lowercase().contains(ext))
            .collect();
        if tier.is_empty() {
            return None;
        }
        tier.sort_by_key(|a| std::cmp::Reverse(score(&a.name, want)));
        tier.into_iter().next()
    })
}

/// Split a tag such as `v0.2.0-rc.1` into its numbers and whether it is a
/// prerelease. A tag with no digits in front (a bare `v`, or prose like
/// `weekly-build`) is not a version at all, so callers get `None`.
pub fn parse_version(tag: &str) -> Option<(Vec<u64>, bool)> {
    let t = tag.trim().trim_start_matches(['v', 'V']);
    let core = t.split(['-', '+']).next()?;
    if core.is_empty() {
        return None;
    }
    let mut nums = Vec::new();
    for p in core.split('.') {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        nums.push(p.parse::<u64>().ok()?);
    }
    Some((nums, t[core.len()..].starts_with('-')))
}

fn compare(a: &(Vec<u64>, bool), b: &(Vec<u64>, bool)) -> Ordering {
    for i in 0..a.0.len().max(b.0.len()) {
        let (x, y) = (
            a.0.get(i).copied().unwrap_or(0),
            b.0.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x.cmp(&y);
        }
    }
    // Equal numbers: `1.0.0-rc.1` comes before the `1.0.0` it leads up to.
    match (a.1, b.1) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

/// Is `latest` an upgrade over `current`? An unreadable tag on either side is
/// never an upgrade — a garbled release must not push the user to an older build.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(a), Some(b)) => compare(&a, &b) == Ordering::Greater,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset {
            name: name.into(),
            url: format!(
                "https://github.com/t8y2/dbx/releases/download/v0.6.38/{}",
                name
            ),
            size: 1,
        }
    }

    fn release(names: &[&str]) -> Release {
        Release {
            version: "v0.6.38".into(),
            notes: String::new(),
            url: String::new(),
            date: String::new(),
            assets: names.iter().map(|n| asset(n)).collect(),
        }
    }

    #[test]
    fn only_accepts_an_owner_and_a_repo() {
        assert!(check_slug("t8y2/dbx").is_ok());
        assert!(check_slug("  t8y2/dbx  ").is_ok());
        for bad in [
            "",
            "dbx",
            "t8y2/dbx/releases",
            "t8y2/",
            "/dbx",
            "t8 y2/dbx",
            "a/b?",
        ] {
            assert!(check_slug(bad).is_err(), "accepted {:?}", bad);
        }
    }

    #[test]
    fn builds_the_release_api_url() {
        // An empty or official base means the official endpoint.
        for base in ["", "  ", DEFAULT_API_BASE] {
            assert_eq!(
                release_api_url("t8y2/dbx", base).unwrap(),
                "https://api.github.com/repos/t8y2/dbx/releases/latest"
            );
        }
        assert!(release_api_url("nope", "").is_err());

        // A mirror host keeps the same path shape…
        assert_eq!(
            release_api_url("t8y2/dbx", "https://gh-api.example.com/").unwrap(),
            "https://gh-api.example.com/repos/t8y2/dbx/releases/latest"
        );
        // …and a node template receives the whole official URL, which is how
        // prefix-style proxies are actually addressed.
        assert_eq!(
            release_api_url("t8y2/dbx", "https://p.example/{url}").unwrap(),
            "https://p.example/https://api.github.com/repos/t8y2/dbx/releases/latest"
        );

        // Refused before any request leaves: not a URL, or not a single one.
        assert!(release_api_url("t8y2/dbx", "p.example")
            .unwrap_err()
            .to_string()
            .contains("https://"));
        assert!(release_api_url("t8y2/dbx", "https://p.example /x").is_err());
    }

    #[test]
    fn parses_a_release_and_surfaces_api_errors() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "name": "gitcraft 0.2.0",
            "body": "- 新增更新检查",
            "html_url": "https://github.com/t8y2/dbx/releases/tag/v0.2.0",
            "published_at": "2026-10-10T08:00:00Z",
            "assets": [
              {"name":"gitcraft_0.2.0_arm64.dmg","size":39,"browser_download_url":"https://github.com/x/y/d"},
              {"name":"gitcraft_0.2.0_arm64.dmg.sig","size":4,"browser_download_url":"https://github.com/x/y/s"}
            ]
        }"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.version, "v0.2.0");
        assert_eq!(r.notes, "- 新增更新检查");
        assert_eq!(r.url, "https://github.com/t8y2/dbx/releases/tag/v0.2.0");
        assert_eq!(r.date, "2026-10-10T08:00:00Z");
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.assets[0].name, "gitcraft_0.2.0_arm64.dmg");
        assert_eq!(r.assets[0].size, 39);

        let err = parse_release(r#"{"message":"Not Found"}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Not Found"), "{}", err);
        // A release with no tag cannot be compared, so it is not an answer.
        assert!(parse_release(r#"{"body":"notes"}"#).is_err());
        assert!(parse_release("not json").is_err());
        // No artifacts attached still leaves a version and a page to open.
        assert_eq!(
            parse_release(r#"{"tag_name":"v1.0.0"}"#).unwrap().assets,
            vec![]
        );
    }

    #[test]
    fn picks_the_installer_for_this_platform() {
        // Asset names in the shape a real desktop release publishes.
        let r = release(&[
            "gitcraft_0.6.38_amd64.AppImage",
            "gitcraft_0.6.38_amd64.deb",
            "gitcraft_0.6.38_arm64.AppImage",
            "gitcraft_0.6.38_arm64.dmg",
            "gitcraft_0.6.38_x64.dmg",
            "gitcraft_0.6.38_arm64-setup.exe",
            "gitcraft_0.6.38_x64-offline-setup.exe",
            "gitcraft_0.6.38_x64-setup.exe",
            "gitcraft_0.6.38_x64_en-US.msi",
            "gitcraft_0.6.38_x64-portable.zip",
        ]);
        assert_eq!(
            pick_asset(&r, "macos", "aarch64").unwrap().name,
            "gitcraft_0.6.38_arm64.dmg"
        );
        assert_eq!(
            pick_asset(&r, "macos", "x86_64").unwrap().name,
            "gitcraft_0.6.38_x64.dmg"
        );
        assert_eq!(
            pick_asset(&r, "windows", "x86_64").unwrap().name,
            "gitcraft_0.6.38_x64-setup.exe"
        );
        assert_eq!(
            pick_asset(&r, "linux", "x86_64").unwrap().name,
            "gitcraft_0.6.38_amd64.AppImage"
        );
        // Nothing matched: the caller falls back to the release page.
        assert!(pick_asset(&r, "freebsd", "x86_64").is_none());
        assert!(pick_asset(&release(&[]), "macos", "aarch64").is_none());
    }

    #[test]
    fn skips_checksum_sidecars_and_wrong_arch_builds() {
        let r = release(&[
            "gitcraft_0.6.38_arm64-portable.zip.sig",
            "gitcraft_0.6.38_arm64-portable.zip",
            "gitcraft_0.6.38_x64.dmg",
        ]);
        // Only the zip remains once the .sig and the x64 dmg are out.
        assert_eq!(
            pick_asset(&r, "macos", "aarch64").unwrap().name,
            "gitcraft_0.6.38_arm64-portable.zip"
        );
        // A build with no arch in its name still beats downloading nothing.
        let plain = release(&["gitcraft-latest.dmg", "gitcraft_0.6.38_x64.dmg"]);
        assert_eq!(
            pick_asset(&plain, "macos", "aarch64").unwrap().name,
            "gitcraft-latest.dmg"
        );
    }

    #[test]
    fn reads_a_tag_into_numbers_and_a_prerelease_flag() {
        assert_eq!(parse_version("v0.2.0"), Some((vec![0, 2, 0], false)));
        assert_eq!(parse_version("0.2"), Some((vec![0, 2], false)));
        assert_eq!(parse_version("1.0.0-rc.1"), Some((vec![1, 0, 0], true)));
        assert_eq!(parse_version("1.0.0+build.7"), Some((vec![1, 0, 0], false)));
        for junk in ["", "v", "weekly-build", "0.1.x", "1..2"] {
            assert_eq!(parse_version(junk), None, "parsed {:?}", junk);
        }
    }

    #[test]
    fn compares_versions_by_number_not_by_text() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(
            is_newer("1.10.0", "1.9.0"),
            "must not compare tags as strings"
        );
        assert!(is_newer("0.1.1", "0.1"));
        assert!(
            !is_newer("0.1.0", "0.1.0"),
            "same version is not an upgrade"
        );
        assert!(!is_newer("0.1", "0.1.0"), "missing segments count as zero");
        assert!(!is_newer("0.1.0", "0.2.0"), "older is not an upgrade");
        assert!(
            !is_newer("1.0.0-rc.1", "1.0.0"),
            "a prerelease is not newer"
        );
        assert!(
            is_newer("1.0.0", "1.0.0-rc.2"),
            "the release beats its own rc"
        );
        assert!(
            !is_newer("nightly", "0.1.0"),
            "an unreadable tag never upgrades"
        );
        assert!(!is_newer("9.9.9", "nightly"));
    }
}
