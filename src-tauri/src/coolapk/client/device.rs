use super::*;

/// 检查设备码是否符合官方结构（Base64 逆序解码后包含设备信息字段分号分隔符）
pub(super) fn is_valid_device_code(code: &str) -> bool {
    if code.is_empty() {
        return false;
    }
    let mut rev: String = code.chars().rev().collect();
    let pad = (4 - (rev.len() % 4)) % 4;
    rev.push_str(&"=".repeat(pad));
    if let Ok(bytes) = BASE64.decode(rev.as_bytes()) {
        if let Ok(s) = std::str::from_utf8(&bytes) {
            return s.contains("; ");
        }
    }
    false
}

/// 根据指定的数盟设备 ID（设备ID / ShuzlmID）生成官方标准设备码。
///
/// 官方标准 X-App-Device 格式为：
/// `{device_id}; ; ; ; {manufacturer}; {brand}; {model}; {build}; {oaid}`
/// 经 Base64 编码、字符逆序并剔除换行与 `=` 填充符生成。
pub(super) fn device_code_without_model(code: &str) -> Result<String, String> {
    let mut encoded: String = code.chars().rev().collect();
    while encoded.len() % 4 != 0 { encoded.push('='); }
    let raw = BASE64.decode(encoded).ok().and_then(|bytes| String::from_utf8(bytes).ok()).ok_or("发布设备码格式无效")?;
    let mut fields: Vec<&str> = raw.split(';').map(str::trim).collect();
    if fields.len() != 9 { return Err("发布设备码格式无效".into()); }
    // 保留设备标识/OAID 等身份字段，仅清除厂商、品牌、机型和 Build。
    fields[4..8].fill("");
    Ok(BASE64.encode(fields.join("; ")).chars().rev().filter(|value| *value != '=').collect())
}

pub(super) fn device_code_with_profile(code: &str, profile: &DeviceProfile) -> String {
    let mut encoded: String = code.chars().rev().collect();
    while encoded.len() % 4 != 0 { encoded.push('='); }
    let Some(raw) = BASE64.decode(&encoded).ok().and_then(|bytes| String::from_utf8(bytes).ok()) else {
        return code.to_string();
    };
    let mut fields: Vec<String> = raw.split(';').map(|field| field.trim().to_string()).collect();
    if fields.len() != 9 { return code.to_string(); }
    for (index, value, fallback) in [
        (4, profile.manufacturer.as_deref(), "Xiaomi"),
        (5, profile.brand.as_deref(), "Redmi"),
        (6, profile.model.as_deref(), "23113RKC6C"),
        (7, profile.build.as_deref(), "AQ3A.250226.002"),
    ] {
        fields[index] = value.map(str::trim).filter(|value| !value.is_empty()).unwrap_or(fallback).to_string();
    }
    let mut result: String = BASE64.encode(fields.join("; ")).chars().rev().collect();
    result.retain(|char| char != '=');
    result
}

pub(super) fn generate_device_code_with_device_id(
    device_id: &str,
    model: Option<&str>,
    build: Option<&str>,
) -> String {
    let model = model.filter(|s| !s.trim().is_empty()).unwrap_or("23113RKC6C");
    let build = build.filter(|s| !s.trim().is_empty()).unwrap_or("AQ3A.250226.002");
    let raw = format!("{device_id}; ; ; ; Xiaomi; Xiaomi; {model}; {build}; ");
    let b64 = BASE64.encode(raw.as_bytes());
    let mut rev: String = b64.chars().rev().collect();
    rev.retain(|c| c != '=' && c != '\r' && c != '\n');
    rev
}

/// v1.9.1 及更早版本使用的账号设备码。
///
/// 账号请求身份由 UID 派生 Android ID，再按官方客户端的逆序 Base64
/// 格式生成。该函数只生成设备码；`ddid` 由请求层按设置和接口路径添加。
pub(super) fn generate_device_code_for_id(uid: &str) -> String {
    use md5::{Digest, Md5};

    let mut hasher = Md5::new();
    hasher.update(uid.as_bytes());
    let digest = hasher.finalize();
    let android_id = format!(
        "{:016x}",
        u64::from_le_bytes(digest[..8].try_into().unwrap_or_default())
    );
    let raw = format!(
        "{android_id}; ; ; ; Xiaomi; Xiaomi; 23113RKC6C; UKQ1.230804.001; "
    );
    let b64 = BASE64.encode(raw.as_bytes());
    let mut rev: String = b64.chars().rev().collect();
    rev.retain(|c| c != '=' && c != '\r' && c != '\n');
    rev
}

/// 生成随机设备码（官方标准逆序 Base64 格式）
pub(super) fn generate_random_device_code() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use md5::{Digest, Md5};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    seed ^= COUNTER
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_mul(0x9E3779B97F4A7C15);
    seed ^= std::process::id() as u64;

    let mut hasher = Md5::new();
    hasher.update(seed.to_string().as_bytes());
    let digest = hasher.finalize();
    let android_id = format!(
        "{:016x}",
        u64::from_le_bytes(digest[..8].try_into().unwrap_or_default())
    );
    let raw = format!("{android_id}; ; ; ; Xiaomi; Xiaomi; 23113RKC6C; UKQ1.230804.001; ");
    let b64 = BASE64.encode(raw.as_bytes());
    let mut rev: String = b64.chars().rev().collect();
    rev.retain(|c| c != '=' && c != '\r' && c != '\n');
    rev
}
