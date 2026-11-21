pub(super) fn cookie_without_ddid(cookie: &str) -> String {
    cookie
        .split(';')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let name = part
                .split_once('=')
                .map(|(name, _)| name.trim())
                .unwrap_or(part);
            if name.eq_ignore_ascii_case("ddid") {
                None
            } else {
                Some(part)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn cookie_for_request(cookie: &str, needs_ddid: bool, ddid: Option<&str>) -> String {
    let cookie = cookie_without_ddid(cookie);
    if !needs_ddid {
        return cookie;
    }
    match ddid.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => merge_cookie_value(&cookie, "ddid", &encode_login_cookie_value(value)),
        None => cookie,
    }
}

/// 官方 CookieHelper 会为挂件等内置网页注入 DID；它与写操作会话 ddid 是两个字段。
pub(super) fn user_plugin_cookie(cookie: &str, did: Option<&str>, ddid: Option<&str>) -> String {
    let cookie = cookie_for_request(cookie, ddid.is_some(), ddid);
    match did.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => merge_cookie_value(&cookie, "DID", &encode_login_cookie_value(value)),
        None => cookie,
    }
}

/// 按酷安客户端 CookieInterceptor 的规则编码账号信息。
pub(super) fn encode_login_cookie_value(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (*byte as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", byte),
        })
        .collect()
}

/// 覆盖 Cookie 中指定字段，保留 WebView 带回的其它会话与验证字段。
pub(super) fn merge_cookie_value(cookie: &str, name: &str, value: &str) -> String {
    let mut entries = Vec::new();
    let mut replaced = false;
    for part in cookie.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let key = part.split_once('=').map(|(key, _)| key.trim()).unwrap_or(part);
        if key.eq_ignore_ascii_case(name) {
            if !replaced {
                entries.push(format!("{name}={value}"));
                replaced = true;
            }
        } else {
            entries.push(part.to_string());
        }
    }
    if !replaced {
        entries.push(format!("{name}={value}"));
    }
    entries.join("; ")
}

/// 清除旧版登录信息字段，保证授权码交换只使用本次 WebView 会话。
pub(super) fn remove_cookie_values(cookie: &str, names: &[&str]) -> String {
    cookie
        .split(';')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let key = part.split_once('=').map(|(key, _)| key.trim()).unwrap_or(part);
            if names.iter().any(|name| key.eq_ignore_ascii_case(name)) {
                None
            } else {
                Some(part.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn cookie_with_captcha_validation(cookie: &str, token: Option<&str>) -> String {
    let Some(token) = token.filter(|value| !value.trim().is_empty()) else { return cookie.to_string(); };
    // 与 Android URLEncoder 一致地编码凭证，不输出到日志。
    let mut encoded = reqwest::Url::parse("https://api.coolapk.com").expect("static API URL");
    encoded.query_pairs_mut().append_pair("validate", token);
    let mut cookies: Vec<&str> = cookie.split(';').map(str::trim).filter(|value| !value.is_empty()
        && value.split_once('=').is_none_or(|(name, _)| name.trim() != "validate")).collect();
    cookies.push(encoded.query().unwrap_or_default());
    cookies.join("; ")
}
