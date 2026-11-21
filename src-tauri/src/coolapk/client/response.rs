use super::*;

// 评论失败必须保留验证码配置，不能把缺少 data 的错误响应转换成空列表。
pub(super) fn comment_list_data(response: Value) -> Result<Vec<Value>, String> {
    if application_failure(&response).is_some()
        || response.get("messageStatus").and_then(Value::as_str).is_some_and(|status| status.starts_with("err_"))
    {
        if matches!(application_failure(&response), Some((_, "captcha")))
            || response.get("messageStatus").and_then(Value::as_str).is_some_and(|status| status.contains("captcha"))
        {
            return Err(response.to_string());
        }
        return Err(response.get("message").and_then(Value::as_str).unwrap_or("酷安拒绝了评论请求，请稍后重试").to_string());
    }
    let normalized = wrap_api_data(response)?;
    normalized.get("data").and_then(Value::as_array).cloned()
        .ok_or_else(|| "酷安返回的评论数据格式不正确".to_string())
}

pub(super) fn append_comment_captcha_token<'a>(query: &mut Vec<(&'a str, String)>, token: Option<&str>, field: Option<&'a str>) {
    // APK：captchaField 为空时使用 validate Cookie，不添加查询字段。
    if field.is_some_and(|value| value.trim().is_empty()) { return; }
    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        let field = field.filter(|value| value.starts_with('_') && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
            .unwrap_or("_v2_post_token");
        query.push((field, token.to_string()));
    }
}

pub(super) fn wrap_api_data(response: Value) -> Result<Value, String> {
    if let Some(err_msg) = response.get("error").and_then(Value::as_str) {
        if !err_msg.is_empty() && err_msg != "0" {
            return Err(err_msg.to_string());
        }
    }

    if let Some(message) = response.get("message").and_then(Value::as_str) {
        let code = response
            .get("code")
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(200);
        let status = response.get("status").and_then(|v| v.as_i64()).unwrap_or(1);
        let msg_status = response
            .get("messageStatus")
            .and_then(Value::as_str)
            .unwrap_or("");

        if msg_status == "err_request_captcha_v2" || response.get("messageExtra").is_some() {
            return Err(serde_json::to_string(&response).unwrap_or_else(|_| message.to_string()));
        }

        // 酷安成功信封：code 为 200/0/1 且 status 为 0/1（status=1004/500 等均属失败，
        // 例如"网络环境异常"会以 HTTP 200 + status=1004/500 的形式返回）
        if (code != 200 && code != 0 && code != 1)
            || !(status == 0 || status == 1)
            || msg_status.starts_with("err_")
        {
            return Err(message.to_string());
        }
    }

    if let Some(status) = response.get("status").and_then(|v| v.as_i64()) {
        if status < 0 {
            let msg = response
                .get("message")
                .or_else(|| response.get("error"))
                .and_then(Value::as_str)
                .unwrap_or("酷安服务端拒绝请求");
            return Err(msg.to_string());
        }
    }

    let data = response.get("data").cloned().unwrap_or(response);
    Ok(json!({ "code": 200, "data": data }))
}

/// 包装页面实体数据时保留分页游标，避免丢失卡片列表需要的上下文。
pub(super) fn wrap_page_data_response(response: Value) -> Result<Value, String> {
    let metadata = response
        .as_object()
        .map(|object| {
            [
                "firstItem",
                "first_item",
                "lastItem",
                "last_item",
                "pageContext",
                "page_context",
                "hasMore",
                "has_more",
                "pagination",
                "pageInfo",
                "page_info",
                "total",
                "current",
            ]
            .into_iter()
            .filter_map(|key| object.get(key).cloned().map(|value| (key, value)))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut wrapped = wrap_api_data(response)?;
    if let Some(output) = wrapped.as_object_mut() {
        for (key, value) in metadata {
            output.entry(key.to_string()).or_insert(value);
        }
    }
    Ok(wrapped)
}

/// 将服务端配置中的额外请求参数安全地转成 dataList 查询参数。
/// 仅允许简单键名和标量值，且不允许覆盖分页、地址和上下文参数。
pub(super) fn parse_discovery_request_args(raw: &str) -> Vec<(String, String)> {
    let Ok(Value::Object(args)) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let reserved = ["url", "title", "subTitle", "sub_title", "page", "firstItem", "first_item", "lastItem", "last_item", "pageContext", "page_context"];
    args.into_iter()
        .filter_map(|(key, value)| {
            if key.is_empty() || key.len() > 64 || reserved.iter().any(|item| *item == key) || !key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') {
                return None;
            }
            let text = match value {
                Value::String(value) => value,
                Value::Number(value) => value.to_string(),
                Value::Bool(value) => value.to_string(),
                Value::Array(_) | Value::Object(_) => serde_json::to_string(&value).ok()?,
                Value::Null => return None,
            };
            if text.len() > 4096 || text.chars().any(|ch| ch.is_control()) {
                return None;
            }
            Some((key, text))
        })
        .collect()
}

pub(super) fn is_safe_discovery_page_url(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return false;
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    !lower.starts_with("http://")
        && !lower.starts_with("https://")
        && !lower.starts_with("javascript:")
        && !lower.starts_with("file:")
        && !lower.starts_with("data:")
}

/// 传输失败时把错误的 source 链拼成一行，便于定位 DNS、证书、代理等真因。
/// 只记录错误类别文本；该链不含请求 URL、Cookie 或响应体。
pub(super) fn error_cause_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut causes = Vec::new();
    let mut current = std::error::Error::source(error);
    while let Some(cause) = current {
        causes.push(cause.to_string());
        current = std::error::Error::source(cause);
    }
    if causes.is_empty() {
        "none".to_string()
    } else {
        causes.join(" <- ")
    }
}

#[derive(Default)]
pub(super) struct ApiFailureLogLimiter {
    // 仅保存固定请求路径和错误类别，限制不同键的数量，避免日志设施持有无界状态。
    pub(super) entries: std::collections::HashMap<String, (Option<std::time::Instant>, u64, u64)>,
}

impl ApiFailureLogLimiter {
    pub(super) fn record(&mut self, key: String, now: std::time::Instant) -> Option<(u64, u64)> {
        if self.entries.len() >= 128 && !self.entries.contains_key(&key) {
            if let Some(oldest) = self.entries.iter().min_by_key(|(_, entry)| entry.0).map(|(key, _)| key.clone()) {
                self.entries.remove(&oldest);
            }
        }
        let entry = self.entries.entry(key).or_insert((None, 0, 0));
        entry.1 = entry.1.saturating_add(1);
        if entry.0.is_some_and(|last| now.saturating_duration_since(last).as_secs() < 60) {
            entry.2 = entry.2.saturating_add(1);
            return None;
        }
        let counts = (entry.1, entry.2);
        entry.0 = Some(now);
        entry.2 = 0;
        Some(counts)
    }
}

pub(super) fn log_api_failure_limited(event: &str, method: &str, origin: &str, path: &str, reason: &str, detail: &str) {
    static LIMITER: std::sync::OnceLock<std::sync::Mutex<ApiFailureLogLimiter>> = std::sync::OnceLock::new();
    let counts = LIMITER.get_or_init(Default::default).lock().ok()
        .and_then(|mut limiter| limiter.record(format!("{event}:{method}:{origin}:{path}:{reason}"), std::time::Instant::now()));
    if let Some((total, suppressed)) = counts {
        log::warn!("{event} method={method} origin={origin} path={path} reason={reason} failures_total={total} suppressed_failures={suppressed} {detail}");
    }
}

pub(super) fn classify_api_failure(text: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();
    if lower.contains("captcha") || text.contains("验证码") {
        "captcha"
    } else if text.contains("请求验证错误") {
        "request_validation"
    } else if lower.contains("rate_limit") || lower.contains("too many") || text.contains("访问过于频繁") {
        "rate_limit"
    } else if lower.contains("login") || lower.contains("auth") || text.contains("未登录") {
        "authentication"
    } else if lower.contains("empty response body") {
        "empty_body"
    } else {
        "other"
    }
}

pub(super) fn application_failure(value: &Value) -> Option<(i64, &'static str)> {
    let number = |key| value.get(key).and_then(|raw| raw.as_i64().or_else(|| raw.as_str()?.parse().ok()));
    // 酷安也使用 HTTP 200 + status=1004 等业务错误信封，不能只检查 code。
    let code = number("code").filter(|code| !(0..400).contains(code))
        .or_else(|| number("status").filter(|status| *status != 0 && *status != 1)
            .filter(|_| value.get("message").and_then(Value::as_str).is_some()))?;
    let reason = ["messageStatus", "messageExtra", "message"]
        .iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .map(classify_api_failure)
        .find(|reason| *reason != "other")
        .unwrap_or("other");
    Some((code, reason))
}

pub(super) async fn response_json(response: reqwest::Response) -> Result<Value, String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("failed to read Coolapk response: {e}"))?;

    if !status.is_success() {
        let detail = if body.trim().is_empty() {
            "empty response body".to_string()
        } else {
            body.chars().take(300).collect()
        };
        return Err(format!("Coolapk API returned HTTP {status}: {detail}"));
    }

    serde_json::from_str(&body).map_err(|e| format!("invalid Coolapk JSON response: {e}"))
}
