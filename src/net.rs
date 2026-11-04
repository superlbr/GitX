//! Executes the `ApiRequest`s the provider plugins build. The provider layer
//! stops at "shape the request" on purpose, so the socket lives here and
//! nowhere else.
//!
//! 测试规矩（PROCESS.md）：不连真远端。所以这一层只有薄封装与纯函数有判据，
//! 真正的取数路径由 provider 的 fixture 解析测试与 UI 的状态判据合起来看住。

use std::time::Duration;

use crate::error::{GitxError, Result};
use crate::provider::{ApiRequest, HttpMethod};
use crate::tr;

/// One shot per call: no connection pool to keep alive between background
/// fetches that may be minutes apart.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Header values with control characters would make ureq panic; a mangled
/// token should be an error toast, not a crash.
fn header_is_safe(value: &str) -> bool {
    value.bytes().all(|b| (32..127).contains(&b))
}

fn short(body: &str) -> String {
    let body = body.trim();
    if body.chars().count() <= 300 {
        body.to_string()
    } else {
        let cut: String = body.chars().take(300).collect();
        format!("{}…", cut)
    }
}

pub fn execute(req: &ApiRequest) -> Result<String> {
    for (k, v) in &req.headers {
        if !header_is_safe(v) {
            return Err(GitxError::Other(tr!(
                format!("请求头 {} 的值含非法字符，检查令牌是否复制完整。", k),
                format!("The value of the {} header has illegal characters — check the token was copied in full.", k)
            )));
        }
    }
    let agent = ureq::AgentBuilder::new().timeout(TIMEOUT).build();
    let mut call = match req.method {
        HttpMethod::Get => agent.get(&req.url),
        HttpMethod::Post => agent.post(&req.url),
    };
    for (k, v) in &req.headers {
        call = call.set(k, v);
    }
    let resp = match &req.body {
        Some(body) => call.send_string(body),
        None => call.call(),
    };
    match resp {
        Ok(r) => Ok(r.into_string()?),
        Err(ureq::Error::Status(code, r)) => {
            let detail = r.into_string().unwrap_or_default();
            Err(GitxError::Other(tr!(
                format!("HTTP {}：{}", code, short(&detail)),
                format!("HTTP {} — {}", code, short(&detail))
            )))
        }
        Err(ureq::Error::Transport(t)) => Err(GitxError::Other(tr!(
            format!("网络错误：{}", t),
            format!("Network error: {t}")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_header_values_a_request_cannot_carry() {
        assert!(header_is_safe("Bearer abc123_-."));
        assert!(!header_is_safe("tok\ninjected"));
        assert!(!header_is_safe("中文令牌"));
        // `execute` 要在**开任何 socket 之前**拒绝，所以这条判据不连网也跑得成。
        let mut bad = ApiRequest::get("https://api.example.com/x".into());
        bad.headers.push(("authorization".into(), "bad\r\nx".into()));
        let e = execute(&bad).unwrap_err().to_string();
        assert!(e.contains("非法字符"), "{}", e);
    }

    #[test]
    fn long_error_bodies_are_cut_for_the_toast() {
        let long = "x".repeat(1000);
        let s = short(&long);
        assert_eq!(s.chars().count(), 301);
        assert!(s.ends_with('…'));
        assert_eq!(short("  简短  "), "简短");
    }
}
