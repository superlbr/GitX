//! SSH diagnostics: read what is already on the machine (`~/.ssh`) and turn the
//! terse, host-specific output of `ssh -T git@host` into a verdict the UI can
//! show. Everything decidable from text lives here, so the whole interpretation
//! layer is unit-testable with no network and no key.

use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{GitxError, Result};

/// The private-key names `ssh-keygen` creates by default, most preferred first.
pub const DEFAULT_KEY_NAMES: &[&str] = &["id_ed25519", "id_ecdsa", "id_rsa", "id_dsa"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyKind {
    /// A private key — the file the user must never hand to anyone.
    Private,
    /// The matching `.pub`, which is what gets pasted into a provider's settings.
    Public,
    Config,
    KnownHosts,
    /// Anything else in the folder: `environment`, `rc`, a note to self.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshKey {
    pub name: String,
    pub path: String,
    pub kind: KeyKind,
    /// Group/world readable private keys make ssh refuse to use them, which is
    /// the most common "permission denied" that is not actually a bad key.
    pub insecure: bool,
    /// A matching `.pub` sits next to it, so the UI can offer 「复制公钥」.
    pub has_public_pair: bool,
}

/// One `Host` block of `~/.ssh/config`, reduced to what a user recognizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHost {
    pub pattern: String,
    pub hostname: String,
    pub user: String,
    pub identity_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshInventory {
    pub dir: String,
    pub exists: bool,
    pub keys: Vec<SshKey>,
    pub hosts: Vec<SshHost>,
    /// The preferred default key that actually exists, so the UI can name it.
    pub default_key: Option<String>,
}

/// Extension first, then content: a hand-named key like `work_key` is still a
/// private key, and `known_hosts` is still not one whatever it contains.
fn classify(name: &str, head: &str) -> KeyKind {
    if name.ends_with(".pub") {
        KeyKind::Public
    } else if name == "config" {
        KeyKind::Config
    } else if name == "known_hosts" || name.starts_with("known_hosts.") {
        KeyKind::KnownHosts
    } else if head.contains("PRIVATE KEY") || head.starts_with("openssh-key-v1") {
        KeyKind::Private
    } else if name.starts_with("id_") {
        // Unreadable, but a name like `id_ed25519` is a key nobody wants hidden.
        KeyKind::Private
    } else {
        KeyKind::Other
    }
}

/// First bytes of a file as text, for the private-key sniff. Never fails the
/// scan: an unreadable file simply contributes no content hint.
fn head_of(path: &std::path::Path) -> String {
    std::fs::read(path)
        .map(|bytes| {
            let take = bytes.len().min(64);
            String::from_utf8_lossy(&bytes[..take]).into_owned()
        })
        .unwrap_or_default()
}

/// Group/world readable bits on a key. Windows reports no POSIX modes, so the
/// check stays false there instead of guessing at ACLs.
fn readable_by_others(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o077 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// Parse `~/.ssh/config` into Host blocks. ssh's own grammar is loose — keywords
/// are case-insensitive, separated by whitespace *or* `=`, and `#` starts a
/// comment — so the split takes whichever delimiter comes first.
pub fn parse_config(text: &str) -> Vec<SshHost> {
    let mut out: Vec<SshHost> = Vec::new();
    let mut cur: Option<SshHost> = None;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some(sep) = line.find(|c: char| c.is_whitespace() || c == '=') else {
            continue;
        };
        let key = line[..sep].to_ascii_lowercase();
        let value = line[sep..].trim_start_matches(|c: char| c.is_whitespace() || c == '=');
        if key == "host" {
            if let Some(host) = cur.take() {
                out.push(host);
            }
            cur = Some(SshHost {
                pattern: value.to_string(),
                hostname: String::new(),
                user: String::new(),
                identity_file: String::new(),
            });
            continue;
        }
        // Anything above the first `Host` belongs to no block ssh will read, and
        // matching after the `host` check keeps `HostName` out of trouble.
        let Some(host) = cur.as_mut() else { continue };
        match key.as_str() {
            "hostname" => host.hostname = value.to_string(),
            "user" => host.user = value.to_string(),
            // A block may list several identities; ssh tries them in order, so
            // the first is the one worth showing.
            "identityfile" if host.identity_file.is_empty() => {
                host.identity_file = expand_tilde(value)
            }
            _ => {}
        }
    }
    if let Some(host) = cur {
        out.push(host);
    }
    out
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return format!("{}/{}", home, rest);
        }
    }
    path.to_string()
}

/// `$HOME` on unix, `%USERPROFILE%` on Windows.
pub fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("USERPROFILE").ok().filter(|v| !v.is_empty()))
}

/// What is in `~/.ssh` right now. A missing folder is a normal answer, not an
/// error: plenty of machines use HTTPS only and have no SSH at all.
pub fn inventory() -> SshInventory {
    inventory_in(&home_dir().unwrap_or_default())
}

pub fn inventory_in(home: &str) -> SshInventory {
    let dir = if home.is_empty() {
        String::new()
    } else {
        format!("{}/.ssh", home)
    };
    let exists = std::path::Path::new(&dir).is_dir();
    let mut inv = SshInventory {
        dir,
        exists,
        keys: Vec::new(),
        hosts: Vec::new(),
        default_key: None,
    };
    if !inv.exists {
        return inv;
    }
    let path = std::path::Path::new(&inv.dir);
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                files.push((entry.file_name().to_string_lossy().into_owned(), p));
            }
        }
    }
    for (name, full) in &files {
        let kind = classify(name, &head_of(full));
        if !matches!(kind, KeyKind::Private | KeyKind::Public) {
            continue;
        }
        let pair = format!("{}.pub", name.strip_suffix(".pub").unwrap_or(name));
        inv.keys.push(SshKey {
            name: name.clone(),
            path: full.to_string_lossy().into_owned(),
            kind,
            insecure: kind == KeyKind::Private && readable_by_others(full),
            has_public_pair: kind == KeyKind::Private && files.iter().any(|(n, _)| *n == pair),
        });
    }
    inv.keys.sort_by(|a, b| a.name.cmp(&b.name));
    inv.default_key = DEFAULT_KEY_NAMES
        .iter()
        .find(|k| {
            inv.keys
                .iter()
                .any(|e| e.kind == KeyKind::Private && &e.name == *k)
        })
        .map(|k| format!("{}/{}", inv.dir, k));
    inv.hosts = std::fs::read_to_string(path.join("config"))
        .map(|text| parse_config(&text))
        .unwrap_or_default();
    inv
}

/// Read a `.pub` file, which is the one thing in `~/.ssh` meant to be shared —
/// pasted into a provider's "new SSH key" form. Guarded on purpose: the path must
/// be inside `~/.ssh` and must end in `.pub`, so this can never be used to dump a
/// private key.
pub fn public_key(path: &str) -> Result<String> {
    public_key_in(&inventory().dir, path)
}

pub fn public_key_in(dir: &str, path: &str) -> Result<String> {
    let base = std::path::Path::new(dir);
    let p = std::path::Path::new(path);
    if !base.as_os_str().is_empty() && p.parent() != Some(base) {
        return Err(GitxError::Other("只能复制 ~/.ssh 里的公钥。".into()));
    }
    if !p
        .file_name()
        .map(|n| n.to_string_lossy().ends_with(".pub"))
        .unwrap_or(false)
    {
        return Err(GitxError::Other(
            "只有 .pub 公钥可以复制，私钥不会离开磁盘。".into(),
        ));
    }
    std::fs::read_to_string(p).map_err(|e| GitxError::Other(format!("读取公钥失败: {}", e)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthOutcome {
    Success,
    PermissionDenied,
    /// The key exists but its file mode is too open, so ssh refuses to read it.
    KeyPermissions,
    UnknownHost,
    Timeout,
    Network,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthReport {
    pub outcome: AuthOutcome,
    pub label: String,
    /// The account the server named, e.g. `t8y2` from "Hi t8y2!".
    pub account: String,
    /// What the server actually said, kept for a details disclosure.
    pub detail: String,
}

/// Providers word the same success differently, so dig the login out of the two
/// shapes that matter: "Hi <name>!" and "Welcome to GitLab, @<name>!".
fn account_from(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if let Some(i) = lower.find("hi ") {
        let tail = &text[i + 3..];
        if let Some(j) = tail.find('!') {
            let name = tail[..j].trim();
            if !name.is_empty() && !name.contains(' ') {
                return name.to_string();
            }
        }
    }
    if let Some(i) = lower.find("welcome") {
        let tail = &text[i..];
        if let Some(at) = tail.find('@') {
            let name: String = tail[at + 1..]
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '!' && *c != ',')
                .collect();
            if !name.is_empty() {
                return name;
            }
        }
    }
    String::new()
}

/// Turn ssh's own words into a verdict. Order matters: a successful GitHub login
/// also says "using publickey", which must not read back as a denial.
pub fn interpret_auth(text: &str) -> AuthReport {
    let lower = text.to_ascii_lowercase();
    let account = account_from(text);
    let hit = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));
    let (outcome, label) = if hit(&["successfully authenticated", "welcome to gitlab"])
        || (lower.contains("hi ") && !account.is_empty())
    {
        (AuthOutcome::Success, "认证通过")
    } else if hit(&["bad owner or permissions", "unprotected private key"]) {
        (
            AuthOutcome::KeyPermissions,
            "密钥文件权限太宽，ssh 拒绝使用（改成 600）",
        )
    } else if hit(&[
        "permission denied",
        "no supported authentication methods",
        "too many authentication failures",
        "invalid key file",
        "no mutual signature",
    ]) {
        (
            AuthOutcome::PermissionDenied,
            "被拒绝：服务器不认识这台机器的密钥",
        )
    } else if hit(&["host key verification failed", "known hosts", "fingerprint"]) {
        (
            AuthOutcome::UnknownHost,
            "主机指纹未被信任：先在终端里 ssh 一次确认",
        )
    } else if hit(&["timed out", "timeout"]) {
        (AuthOutcome::Timeout, "连接超时，可能被网络或代理挡住")
    } else if hit(&[
        "could not resolve",
        "connection refused",
        "network is unreachable",
        "no route to host",
        "connection closed",
    ]) {
        (AuthOutcome::Network, "连不上服务器，检查网络或代理")
    } else {
        (AuthOutcome::Unknown, "没有得到明确的答复")
    };
    AuthReport {
        outcome,
        label: label.to_string(),
        account,
        detail: text.trim().to_string(),
    }
}

/// Ask a host to authenticate without implementing any of the transport: plain
/// batch-mode `ssh -T`, so nothing waits on a password prompt. Host verification
/// stays at ssh's default on purpose — auto-accepting a fingerprint would write
/// into the user's `known_hosts` and quietly drop a security check, so a first
/// connection reports `unknown_host` and tells the user to confirm once by hand.
pub fn probe(host: &str, user: Option<&str>) -> Result<AuthReport> {
    let host = host.trim();
    if host.is_empty() || host.split_whitespace().count() != 1 {
        return Err(GitxError::Other("主机名不能为空，也不能包含空格。".into()));
    }
    let user = user
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .unwrap_or("git");
    let out = Command::new("ssh")
        .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "-T"])
        .arg(format!("{}@{}", user, host))
        .output()?;
    // ssh puts everything useful on stderr, and GitHub exits 1 even on success
    // (it refused the shell we never asked for), so the text decides.
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let text = if stderr.trim().is_empty() {
        String::from_utf8_lossy(&out.stdout).into_owned()
    } else {
        stderr
    };
    Ok(interpret_auth(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    const PEM: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaA\n";
    const OPENSSH_MAGIC: &str = "openssh-key-v1\0\0\0\0\u{7}";

    #[test]
    fn classifies_by_extension_then_content() {
        assert_eq!(classify("id_ed25519", PEM), KeyKind::Private);
        // A hand-named key is still a private key.
        assert_eq!(classify("work_key", OPENSSH_MAGIC), KeyKind::Private);
        assert_eq!(classify("work_key", "notes to self"), KeyKind::Other);
        // An unreadable `id_*` stays a key rather than vanishing from the list.
        assert_eq!(classify("id_rsa", ""), KeyKind::Private);
        assert_eq!(
            classify("id_ed25519.pub", "ssh-ed25519 AAAA"),
            KeyKind::Public
        );
        assert_eq!(classify("id_rsa-cert.pub", ""), KeyKind::Public);
        // A key-shaped payload does not make a config file into a key.
        assert_eq!(classify("config", PEM), KeyKind::Config);
        assert_eq!(classify("known_hosts.old", ""), KeyKind::KnownHosts);
    }

    const CONFIG: &str = "\
# 我的 GitHub 账号
Host github
  HostName github.com
  User git
  IdentityFile ~/.ssh/id_ed25519
  IdentityFile ~/.ssh/id_rsa
  IdentitiesOnly yes

Host *
\tControlMaster auto
\tIdentityFile = ~/.ssh/id_rsa

Host gitee
  HostName=gitee.com
  USER=giteeuser
";

    #[test]
    fn reads_host_blocks_through_loose_spacing_and_case() {
        let hosts = parse_config(CONFIG);
        assert_eq!(hosts.len(), 3);
        assert_eq!(hosts[0].pattern, "github");
        assert_eq!(hosts[0].hostname, "github.com");
        assert_eq!(hosts[0].user, "git");
        // Several identities: ssh tries them in order, so show the first one.
        assert_eq!(hosts[0].identity_file, expand_tilde("~/.ssh/id_ed25519"));
        // Tab indentation and `Key = value` both parse.
        assert_eq!(hosts[1].pattern, "*");
        assert_eq!(hosts[1].identity_file, expand_tilde("~/.ssh/id_rsa"));
        // `HostName=gitee.com` and `USER=` are the same keywords to ssh.
        assert_eq!(hosts[2].hostname, "gitee.com");
        assert_eq!(hosts[2].user, "giteeuser");
        assert!(
            hosts[2].identity_file.is_empty(),
            "a block must not inherit one"
        );
    }

    #[test]
    fn ignores_comments_and_orphan_settings() {
        let hosts = parse_config("ControlPath /tmp/%h\nHost b\n  HostName b.example\n");
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].pattern, "b");
        assert_eq!(hosts[0].hostname, "b.example");
        assert!(parse_config("#Host c\n#  HostName c.example\n").is_empty());
        assert!(parse_config("").is_empty());
        assert!(parse_config("HostName orphan.example").is_empty());
    }

    fn touch(ssh: &std::path::Path, name: &str, content: &str, mode: u32) {
        fs::write(ssh.join(name), content).unwrap();
        #[cfg(unix)]
        fs::set_permissions(ssh.join(name), std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn lists_key_files_and_their_pairing() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = dir.path().join(".ssh");
        fs::create_dir_all(&ssh).unwrap();
        touch(&ssh, "id_ed25519", PEM, 0o600);
        touch(
            &ssh,
            "id_ed25519.pub",
            "ssh-ed25519 AAAA user@host\n",
            0o644,
        );
        // A private key readable by the group is the classic silent failure.
        touch(&ssh, "work_key", PEM, 0o644);
        touch(
            &ssh,
            "config",
            "Host github\n  HostName github.com\n",
            0o600,
        );
        touch(&ssh, "known_hosts", "github.com ssh-rsa AAAA\n", 0o644);
        touch(&ssh, "README", "not a key at all\n", 0o644);
        fs::create_dir(ssh.join("sockets")).unwrap();

        let inv = inventory_in(dir.path().to_str().unwrap());
        assert!(inv.exists);
        let names: Vec<&str> = inv.keys.iter().map(|k| k.name.as_str()).collect();
        // config / known_hosts / README / the subdirectory are not offered as keys.
        assert_eq!(names, ["id_ed25519", "id_ed25519.pub", "work_key"]);
        assert_eq!(inv.hosts.len(), 1);
        assert_eq!(
            inv.default_key.as_deref(),
            Some(format!("{}/id_ed25519", ssh.display()).as_str())
        );

        let find = |n: &str| inv.keys.iter().find(|k| k.name == n).unwrap().clone();
        assert_eq!(find("id_ed25519").kind, KeyKind::Private);
        assert_eq!(find("work_key").kind, KeyKind::Private);
        assert_eq!(find("id_ed25519.pub").kind, KeyKind::Public);
        assert!(find("id_ed25519").has_public_pair);
        assert!(!find("work_key").has_public_pair);
        assert!(!find("id_ed25519").insecure, "0600 is fine");
        // Public keys are never flagged, whatever their mode.
        assert!(!find("id_ed25519.pub").insecure);
        #[cfg(unix)]
        assert!(
            find("work_key").insecure,
            "0644 private key must be flagged"
        );
    }

    #[test]
    fn missing_ssh_dir_is_an_answer_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_str().unwrap();
        let inv = inventory_in(home);
        assert!(!inv.exists);
        assert!(inv.keys.is_empty() && inv.hosts.is_empty() && inv.default_key.is_none());
        assert_eq!(inv.dir, format!("{}/.ssh", home));
        assert!(!inventory_in("").exists);
        assert_eq!(inventory_in("").dir, "");
    }

    #[test]
    fn only_a_public_key_inside_the_ssh_dir_can_be_copied() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = dir.path().join(".ssh");
        fs::create_dir_all(&ssh).unwrap();
        touch(&ssh, "id_ed25519", PEM, 0o600);
        touch(
            &ssh,
            "id_ed25519.pub",
            "ssh-ed25519 AAAAC3 user@host\n",
            0o644,
        );
        let base = ssh.to_str().unwrap();

        let text = public_key_in(base, ssh.join("id_ed25519.pub").to_str().unwrap()).unwrap();
        assert!(text.starts_with("ssh-ed25519 "), "{}", text);

        // The private key, a file elsewhere, and a path outside the folder all
        // get refused before anything is opened.
        for bad in [
            ssh.join("id_ed25519"),
            dir.path().join("outside.pub"),
            std::path::PathBuf::from("/etc/passwd"),
        ] {
            let e = public_key_in(base, bad.to_str().unwrap())
                .unwrap_err()
                .to_string();
            assert!(e.contains("公钥"), "{:?} → {}", bad, e);
        }
        // A missing .pub is an IO error, still not a panic.
        assert!(public_key_in(base, ssh.join("nope.pub").to_str().unwrap()).is_err());
    }

    #[test]
    fn reads_githubs_success_though_ssh_exits_nonzero() {
        let r = interpret_auth(
            "Hi t8y2! You've successfully authenticated, but GitHub does not provide shell access.\n",
        );
        assert_eq!(r.outcome, AuthOutcome::Success);
        assert_eq!(r.account, "t8y2");
    }

    #[test]
    fn reads_each_providers_wording_of_the_same_success() {
        let r = interpret_auth("Welcome to GitLab, @root!");
        assert_eq!(r.outcome, AuthOutcome::Success);
        assert_eq!(r.account, "root");
        // "Authenticated using publickey" is a success line, not a denial.
        let r = interpret_auth(
            "Authenticated to github.com ([20.205.243.166]:22) using \"publickey\".\nHi t8y2! You've successfully authenticated.\n",
        );
        assert_eq!(r.outcome, AuthOutcome::Success);
        assert_eq!(r.account, "t8y2");
    }

    #[test]
    fn tells_apart_the_failures_everyone_blames_on_the_key() {
        assert_eq!(
            interpret_auth("git@github.com: Permission denied (publickey).\r\n").outcome,
            AuthOutcome::PermissionDenied
        );
        assert_eq!(
            interpret_auth("WARNING: UNPROTECTED PRIVATE KEY FILE!\r\nBad permissions.").outcome,
            AuthOutcome::KeyPermissions
        );
        assert_eq!(
            interpret_auth("Bad owner or permissions on /Users/me/.ssh/config").outcome,
            AuthOutcome::KeyPermissions
        );
        assert_eq!(
            interpret_auth("ssh: connect to host github.com port 22: Operation timed out").outcome,
            AuthOutcome::Timeout
        );
        assert_eq!(
            interpret_auth("ssh: Could not resolve hostname github.com: nodename nor servname")
                .outcome,
            AuthOutcome::Network
        );
        assert_eq!(
            interpret_auth("Host key verification failed.").outcome,
            AuthOutcome::UnknownHost
        );
        assert_eq!(interpret_auth("").outcome, AuthOutcome::Unknown);
        assert_eq!(
            interpret_auth("some other thing").outcome,
            AuthOutcome::Unknown
        );
    }

    #[test]
    fn probe_refuses_hosts_that_are_not_one_token() {
        // The host becomes an argv, never a shell string — but a stray space in
        // it would still smuggle extra options, so it is rejected outright.
        for bad in ["", "   ", "github.com -i /etc/passwd", "a b"] {
            assert!(probe(bad, None).is_err(), "accepted {:?}", bad);
        }
    }
}
