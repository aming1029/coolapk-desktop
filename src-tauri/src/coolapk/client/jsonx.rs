use super::*;

pub(super) fn parse_http_url(value: &Value) -> Option<String> {
    let raw = value.as_str()?.trim();
    let parsed = reqwest::Url::parse(raw).ok()?;
    if parsed.scheme() == "http" || parsed.scheme() == "https" {
        Some(raw.to_string())
    } else {
        None
    }
}

pub(super) fn parse_u64_val(val: &Value) -> Option<u64> {
    if let Some(n) = val.as_u64() {
        return Some(n);
    }
    if let Some(i) = val.as_i64() {
        if i >= 0 {
            return Some(i as u64);
        }
    }
    if let Some(s) = val.as_str() {
        return s.trim().parse::<u64>().ok();
    }
    None
}

pub(super) fn get_u64_by_keys(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> u64 {
    for k in keys {
        if let Some(val) = obj.get(*k) {
            if let Some(n) = parse_u64_val(val) {
                return n;
            }
        }
    }
    0
}

pub(super) fn get_str_by_keys(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(val) = obj.get(*k) {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            } else if let Some(n) = parse_u64_val(val) {
                return Some(n.to_string());
            }
        }
    }
    None
}

pub(super) fn first_value_by_keys<'a>(obj: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| obj.get(*key))
}

pub(super) fn has_non_empty_json_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(_) | Value::Number(_) => true,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => values.iter().any(has_non_empty_json_value),
        Value::Object(values) => values.values().any(has_non_empty_json_value),
    }
}

pub(super) fn has_any_non_empty_field(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> bool {
    first_value_by_keys(obj, keys).map_or(false, has_non_empty_json_value)
}

pub(super) fn copy_first_field(cleaned: &mut Value, obj: &serde_json::Map<String, Value>, output_key: &str, keys: &[&str]) {
    let Some(value) = first_value_by_keys(obj, keys) else {
        return;
    };
    if let Some(cleaned_obj) = cleaned.as_object_mut() {
        cleaned_obj.insert(output_key.to_string(), value.clone());
    }
}

pub(super) fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .unwrap_or_default()
}

pub(super) fn value_to_string_opt(value: &Value) -> Option<String> {
    let s = value_to_string(value);
    if s.is_empty() { None } else { Some(s) }
}

/// 从评论详情的酷安 UA 中提取真实设备代号、系统版本与构建号。
/// 接口经常保留 useragent，却把 device_title 等便捷字段返回为空。
pub(super) fn parse_reply_user_agent(user_agent: &str) -> (String, String, String) {
    let android_version = user_agent
        .split("Android ")
        .nth(1)
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .unwrap_or("");
    let build = user_agent
        .split(" Build/")
        .nth(1)
        .and_then(|value| value.split([')', ' ']).next())
        .map(str::trim)
        .unwrap_or("");

    let fallback_model = user_agent
        .split(" Build/")
        .next()
        .and_then(|value| value.rsplit("; ").next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");

    let build_metadata = user_agent
        .split("(#Build; ")
        .nth(1)
        .and_then(|value| value.split(')').next())
        .map(|value| value.split(';').map(str::trim).collect::<Vec<_>>())
        .unwrap_or_default();
    let manufacturer = build_metadata.first().copied().unwrap_or("");
    let model = build_metadata
        .get(1)
        .copied()
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_model);
    let device_title = [manufacturer, model]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let rom_family = build_metadata
        .get(3)
        .copied()
        .unwrap_or("")
        .split('_')
        .next()
        .unwrap_or("");
    let rom_version = build_metadata.get(4).copied().unwrap_or("");
    let rom_label = [rom_family, rom_version]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let device_rom = [
        (!android_version.is_empty()).then(|| format!("Android {android_version}")),
        (!rom_label.is_empty() && rom_label != android_version).then_some(rom_label),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");

    (device_title, build.to_string(), device_rom)
}

pub(super) fn rank_feed_url(rank_type: &str) -> Option<&'static str> {
    match rank_type {
        // 实测：月榜 statType=30days 有效（6659/3676/3630），statType=month 返回空
        "month" => Some("#/feed/statList?statType=30days&sortField=likenum"),
        // 实测：收藏榜必须 statType=7days（favnum 排序），statType=all 返回空
        "favorite" => Some("#/feed/statList?statType=7days&sortField=favnum"),
        "index" => Some("#/feed/statList?statType=7days&sortField=detailnum"),
        _ => None,
    }
}
