use super::*;

pub(super) fn build_oss_image_url(prefix: &str, file_name: &str) -> Option<String> {
    let prefix = prefix.trim().trim_end_matches('/');
    let file_name = file_name.trim().trim_start_matches('/');
    if (prefix.starts_with("http://") || prefix.starts_with("https://"))
        && !prefix.is_empty()
        && !file_name.is_empty()
    {
        Some(format!("{prefix}/{file_name}"))
    } else {
        None
    }
}

pub(super) fn image_resolution(image_bytes: &[u8]) -> String {
    let dimensions = if image_bytes.starts_with(b"\x89PNG\r\n\x1a\n") && image_bytes.len() >= 24 {
        Some((
            u32::from_be_bytes(image_bytes[16..20].try_into().unwrap_or([0; 4])),
            u32::from_be_bytes(image_bytes[20..24].try_into().unwrap_or([0; 4])),
        ))
    } else if (image_bytes.starts_with(b"GIF87a") || image_bytes.starts_with(b"GIF89a"))
        && image_bytes.len() >= 10
    {
        Some((
            u16::from_le_bytes(image_bytes[6..8].try_into().unwrap_or([0; 2])) as u32,
            u16::from_le_bytes(image_bytes[8..10].try_into().unwrap_or([0; 2])) as u32,
        ))
    } else if image_bytes.starts_with(b"BM") && image_bytes.len() >= 26 {
        Some((
            u32::from_le_bytes(image_bytes[18..22].try_into().unwrap_or([0; 4])),
            u32::from_le_bytes(image_bytes[22..26].try_into().unwrap_or([0; 4])),
        ))
    } else if image_bytes.starts_with(b"RIFF")
        && image_bytes.len() >= 30
        && &image_bytes[8..12] == b"WEBP"
        && &image_bytes[12..16] == b"VP8X"
    {
        let width = 1
            + (image_bytes[24] as u32
                | ((image_bytes[25] as u32) << 8)
                | ((image_bytes[26] as u32) << 16));
        let height = 1
            + (image_bytes[27] as u32
                | ((image_bytes[28] as u32) << 8)
                | ((image_bytes[29] as u32) << 16));
        Some((width, height))
    } else if image_bytes.starts_with(&[0xff, 0xd8]) {
        jpeg_resolution(image_bytes)
    } else {
        None
    };

    dimensions
        .filter(|(width, height)| *width > 0 && *height > 0)
        .map(|(width, height)| format!("{width}x{height}"))
        .unwrap_or_else(|| "0x0".to_string())
}

pub(super) fn jpeg_resolution(image_bytes: &[u8]) -> Option<(u32, u32)> {
    let mut index = 2;
    while index + 9 < image_bytes.len() {
        if image_bytes[index] != 0xff {
            index += 1;
            continue;
        }
        while index < image_bytes.len() && image_bytes[index] == 0xff {
            index += 1;
        }
        if index >= image_bytes.len() {
            break;
        }
        let marker = image_bytes[index];
        index += 1;
        if marker == 0xda || marker == 0xd9 {
            break;
        }
        if index + 1 >= image_bytes.len() {
            break;
        }
        let segment_length = u16::from_be_bytes([image_bytes[index], image_bytes[index + 1]]) as usize;
        if segment_length < 2 || index + segment_length > image_bytes.len() {
            break;
        }
        let is_sof = matches!(
            marker,
            0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf
        );
        if is_sof && segment_length >= 7 {
            let height = u16::from_be_bytes([image_bytes[index + 3], image_bytes[index + 4]]) as u32;
            let width = u16::from_be_bytes([image_bytes[index + 5], image_bytes[index + 6]]) as u32;
            return Some((width, height));
        }
        index += segment_length;
    }
    None
}

/// 是否属于酷安官方域名：登录 Cookie / App 指纹头等凭据只允许发送给酷安域，
/// 严禁携带到任意第三方域名（防止凭据经外部链接/图片地址泄露）。
pub(super) fn is_coolapk_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "coolapk.com" || host.ends_with(".coolapk.com")
}

pub(super) fn is_weibo_video_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "weibocdn.com" || host.ends_with(".weibocdn.com")
}

pub(super) fn is_weibo_image_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "sinaimg.cn" || host.ends_with(".sinaimg.cn")
}

/// 从 Live Photo 解析响应中兼容提取视频地址。
/// 酷安不同接口版本可能返回单个 url、data.url 或 data.urlList。
pub(super) fn extract_live_photo_video_url(value: &Value) -> Option<String> {
    match value {
        Value::String(_) => parse_http_url(value),
        Value::Array(items) => items.iter().find_map(extract_live_photo_video_url),
        Value::Object(object) => {
            for key in ["url", "videoUrl", "video_url", "finalUrl", "final_url"] {
                if let Some(url) = object.get(key).and_then(parse_http_url) {
                    return Some(url);
                }
            }
            for key in ["urlList", "url_list", "data"] {
                if let Some(value) = object.get(key) {
                    if let Some(url) = extract_live_photo_video_url(value) {
                        return Some(url);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

pub(super) fn normalize_coolapk_image_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "undefined" {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Some(trimmed.to_string())
    } else {
        Some(format!(
            "https://image.coolapk.com/{}",
            trimmed.trim_start_matches('/')
        ))
    }
}

pub(super) fn image_source_url(value: &Value) -> Option<String> {
    if let Some(raw) = value.as_str() {
        return normalize_coolapk_image_url(raw);
    }
    let object = value.as_object()?;
    for key in [
        "sourceUrl",
        "source_url",
        "inSource",
        "in_source",
        "url",
        "pic",
        "imageUrl",
        "image_url",
        "src",
    ] {
        if let Some(raw) = object.get(key).and_then(Value::as_str) {
            if let Some(url) = normalize_coolapk_image_url(raw) {
                return Some(url);
            }
        }
    }
    None
}
