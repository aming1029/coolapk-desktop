use super::http_session::{session, Policy as SessionPolicy};
use crate::coolapk::auth::{APP_CODE, CoolapkAuth};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::header::{COOKIE, HeaderMap, HeaderValue, LOCATION, USER_AGENT};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

// 可选字段与界面保持一致，未设置时保留普通动态默认值。
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishOptions {
    pub include_device_model: Option<bool>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub sub_type_id: Option<String>,
    pub sub_data: Option<String>,
    pub visible_status: Option<i32>,
    pub large_cover: Option<bool>,
    pub html_article: Option<bool>,
    pub message_title: Option<String>,
    pub message_cover: Option<String>,
    pub original_type: Option<u32>,
    pub extra_url: Option<String>,
    pub dyh_id: Option<String>,
    pub media_url: Option<String>,
    pub media_info: Option<String>,
}

fn apply_publish_options(form: &mut Vec<(&'static str, String)>, options: &PublishOptions) -> Result<(), String> {
    let target_type = options.target_type.as_deref().unwrap_or("");
    let target_id = options.target_id.as_deref().unwrap_or("");
    if options.visible_status.is_some_and(|status| status != 1 && status != -1) { return Err("动态可见范围无效".to_string()); }
    if options.original_type.is_some_and(|status| status > 3) { return Err("内容声明无效".to_string()); }
    if options.html_article == Some(true) {
        if options.large_cover == Some(true) { return Err("图文发布不能同时设置大图封面模式".to_string()); }
        if !options.message_title.as_deref().is_some_and(|title| !title.trim().is_empty()) { return Err("图文标题不能为空".to_string()); }
    } else if options.message_title.as_deref().is_some_and(|title| !title.trim().is_empty())
        || options.message_cover.as_deref().is_some_and(|cover| !cover.trim().is_empty())
    {
        return Err("图文标题和题图仅支持图文发布".to_string());
    }
    if options.extra_url.as_ref().is_some_and(|url| url.len() > 4096 || url.starts_with("javascript:") || url.starts_with("data:")) { return Err("附加内容链接无效".to_string()); }
    if !["", "tag", "apk", "product_phone"].contains(&target_type) { return Err("不支持的发布板块类型".to_string()); }
    if target_type.is_empty() != target_id.is_empty() { return Err("发布板块信息不完整".to_string()); }
    if let Some(url) = options.media_url.as_deref() {
        let parsed = reqwest::Url::parse(url).map_err(|_| "视频地址无效")?;
        if !["http", "https"].contains(&parsed.scheme()) || url.len() > 4096 { return Err("视频地址无效".to_string()); }
        let info: Value = serde_json::from_str(options.media_info.as_deref().unwrap_or("")).map_err(|_| "视频信息无效")?;
        if info.get("mediaType").and_then(Value::as_str) != Some("video") { return Err("视频信息无效".to_string()); }
    } else if options.media_info.is_some() { return Err("视频地址缺失".to_string()); }
    let sub_type = options.sub_type_id.as_deref().unwrap_or("");
    let sub_data = options.sub_data.as_deref().unwrap_or("");
    if !sub_type.is_empty() {
        if target_type != "product_phone" || !["0", "1", "2", "3", "4", "5", "6"].contains(&sub_type) { return Err("产品子板块无效".to_string()); }
        match sub_type {
            "3" | "4" => { if !form.iter().any(|(key, value)| *key == "pic" && !value.trim().is_empty()) { return Err("上手或样张至少需要一张图片".to_string()); } },
            "1" => { if !sub_data.parse::<f64>().is_ok_and(|hours| hours.is_finite() && hours > 0.0) { return Err("续航时长无效".to_string()); } },
            "2" => {
                let scores: Value = serde_json::from_str(sub_data).map_err(|_| "跑分数据无效".to_string())?;
                let entries = scores.as_object().ok_or("跑分数据无效")?;
                if entries.is_empty() || entries.iter().any(|(key, value)| !["antutu_score", "geek_bench_single_score", "geek_bench_multi_score", "3d_mark_score"].contains(&key.as_str()) || !value.as_f64().is_some_and(|score| score.is_finite() && score > 0.0)) { return Err("跑分数据无效".to_string()); }
            },
            "5" => { if !sub_data.parse::<u32>().is_ok_and(|level| (1..=5).contains(&level)) { return Err("反馈严重度无效".to_string()); } },
            "6" => {
                let price: Value = serde_json::from_str(sub_data).map_err(|_| "到手价数据无效".to_string())?;
                if !price.get("final_price").and_then(Value::as_f64).is_some_and(|price| price.is_finite() && price > 0.0) || !price.get("config_id").and_then(Value::as_u64).is_some_and(|id| id > 0) || !price.get("config_name").and_then(Value::as_str).is_some_and(|name| !name.trim().is_empty()) { return Err("到手价需要价格及产品配置".to_string()); }
            },
            _ => {},
        }
        form.push(("tsubid", sub_type.to_string()));
        form.push(("tsubdata", sub_data.to_string()));
    }
    for (key, value) in form.iter_mut() {
        match *key {
            "targetType" => *value = target_type.to_string(),
            "targetId" => *value = target_id.to_string(),
            "publish_status" if options.visible_status.is_some() => *value = if options.visible_status == Some(-1) { "1" } else { "0" }.to_string(),
            "is_html_article" if options.html_article == Some(true) => *value = "1".to_string(),
            "is_html_article" if options.large_cover.is_some() => *value = if options.large_cover == Some(true) { "2" } else { "0" }.to_string(),
            "message_title" if options.message_title.is_some() => *value = options.message_title.clone().unwrap_or_default(),
            "message_cover" if options.message_cover.is_some() => *value = options.message_cover.clone().unwrap_or_default(),
            "original_type" if options.original_type.is_some() => *value = options.original_type.unwrap_or(0).to_string(),
            "extra_url" if options.extra_url.is_some() => *value = options.extra_url.clone().unwrap_or_default(),
            "media_url" if options.media_url.is_some() => *value = options.media_url.clone().unwrap_or_default(),
            "media_info" if options.media_info.is_some() => *value = options.media_info.clone().unwrap_or_default(),
            "dyhId" if options.dyh_id.is_some() => *value = options.dyh_id.clone().unwrap_or_default(),
            "type" if target_type == "apk" => *value = "comment".to_string(),
            _ => {},
        }
    }
    Ok(())
}

/// 按官方 OSS 清单登记实况关系，视频名使用内容摘要避免同名覆盖。
fn build_publish_upload_files(image: &[u8], name: &str, live_video: Option<&[u8]>, hdr: u32) -> (String, Option<String>) {
    use md5::{Digest, Md5};
    let video_name = live_video.map(|video| format!("{:x}.{}", Md5::digest(video), if video.get(8..12) == Some(b"qt  ") { "mov" } else { "mp4" }));
    let mut files = vec![json!({ "name": name, "resolution": image_resolution(image), "md5": format!("{:x}", Md5::digest(image)), "hdr": hdr })];
    if let (Some(video), Some(name)) = (live_video, video_name.as_ref()) {
        files[0]["livePhotoVideo"] = json!(name);
        files[0]["livePhoto"] = json!(1);
        files.push(json!({ "name": name, "md5": format!("{:x}", Md5::digest(video)) }));
    }
    (Value::Array(files).to_string(), video_name)
}

/// 接口路径需求：记录服务端配置中声明的写接口风控要求。
///
/// `needs_ddid` 标记服务端要求携带 DDI 会话 Cookie 的写接口；请求层会在设置了会话值时添加 `ddid`。
#[derive(Clone, Copy, Debug, Default)]
pub struct PathRequirements {
    /// 服务端配置声明该路径需要 `ddid`。
    pub needs_ddid: bool,
    /// 需要表单携带 `_v2_post_token`（网易易盾滑块验证 Token）。
    pub needs_post_token: bool,
}

/// 酷安服务端下发的 `MainInit.useDDIEventList`（写接口需 ddid）。
/// 可通过 `GET /v6/main/init` 的配置卡片动态更新。
const DDI_EVENT_PATHS: &[&str] = &[
    "/v6/feed/createFeed",
    "/v6/feed/reply",
    "/v6/feed/like",
    "/v6/feed/likeReply",
];

/// 酷安服务端下发的 `PostToken.List`（需网易易盾 `_v2_post_token`）。
const POST_TOKEN_PATHS: &[&str] = &["/v6/feed/createFeed", "/v6/feed/reply"];

fn cookie_without_ddid(cookie: &str) -> String {
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

fn cookie_for_request(cookie: &str, needs_ddid: bool, ddid: Option<&str>) -> String {
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
fn user_plugin_cookie(cookie: &str, did: Option<&str>, ddid: Option<&str>) -> String {
    let cookie = cookie_for_request(cookie, ddid.is_some(), ddid);
    match did.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => merge_cookie_value(&cookie, "DID", &encode_login_cookie_value(value)),
        None => cookie,
    }
}

/// 按酷安客户端 CookieInterceptor 的规则编码账号信息。
fn encode_login_cookie_value(value: &str) -> String {
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
fn merge_cookie_value(cookie: &str, name: &str, value: &str) -> String {
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
fn remove_cookie_values(cookie: &str, names: &[&str]) -> String {
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

/// 根据请求路径自动判断需要哪些风控令牌。
pub fn classify_path(path: &str) -> PathRequirements {
    PathRequirements {
        needs_ddid: DDI_EVENT_PATHS.iter().any(|p| path == *p || path.starts_with(p)),
        needs_post_token: POST_TOKEN_PATHS.iter().any(|p| path == *p || path.starts_with(p)),
    }
}

pub struct CoolapkClient {
    client: RwLock<Client>,
    /// Live Photo 解析只需要拿到重定向地址，不能跟随重定向把视频正文提前下载掉。
    redirect_client: RwLock<Client>,
    auth: RwLock<CoolapkAuth>,
    user_cookie: RwLock<Option<String>>,
    cookie_file: RwLock<Option<PathBuf>>,
    device_profile: RwLock<DeviceProfile>,
    native_device_profile: DeviceProfile,
    device_code: RwLock<String>,
    /// 服务端验证码凭证独立于登录态，仅在再次验证成功时替换。
    captcha_validation: RwLock<Option<String>>,
    generated_identity: RwLock<super::shuzilm::IdentityState>,
    identity_generation: tokio::sync::Mutex<()>,
}

pub struct ProxiedVideoResponse {
    pub status: u16,
    pub content_type: String,
    pub content_length: Option<u64>,
    pub content_range: Option<String>,
    pub body: Vec<u8>,
}

fn build_oss_image_url(prefix: &str, file_name: &str) -> Option<String> {
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

fn image_resolution(image_bytes: &[u8]) -> String {
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

fn jpeg_resolution(image_bytes: &[u8]) -> Option<(u32, u32)> {
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

fn reply_target_params(feed_id: &str, rid: Option<&str>) -> (String, String) {
    rid.filter(|value| !value.trim().is_empty())
        .map(|reply_id| (reply_id.trim().to_string(), "reply".to_string()))
        .unwrap_or_else(|| (feed_id.trim().to_string(), "feed".to_string()))
}

fn build_product_rating_query(product_id: &str, value: i32) -> Vec<(&'static str, String)> {
    vec![
        ("id", product_id.to_string()),
        ("value", value.to_string()),
    ]
}

fn build_product_review_form(
    product_id: &str,
    score: i32,
    message: &str,
    buy_status: bool,
) -> Vec<(&'static str, String)> {
    let mut form = build_create_feed_form_for_type(message, None, None, "rating", "");
    for (key, value) in &mut form {
        match *key {
            "targetType" => *value = "product_phone".to_string(),
            "targetId" => *value = product_id.to_string(),
            _ => {}
        }
    }
    form.extend([
        ("rating_score_1", score.to_string()),
        ("buy_status", if buy_status { "1" } else { "0" }.to_string()),
        ("comment_good", String::new()),
        ("comment_general", String::new()),
        ("comment_bad", String::new()),
        ("comment_good_pic", String::new()),
        ("comment_general_pic", String::new()),
        ("comment_bad_pic", String::new()),
    ]);
    form
}

fn build_product_rating_list_query(
    product_id: &str,
    star: i32,
    is_owner: i32,
    page: u32,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("url", "/feed/nodeRatingList".to_string()),
        ("targetType", "7".to_string()),
        ("targetId", product_id.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if is_owner == 1 {
        query.push(("isOwner", "1".to_string()));
    }
    if star > 0 {
        query.push(("star", star.to_string()));
    }
    query
}

// TopicUserRatingFragment / NodeRatingApi: targetType=3，默认不传筛选参数。
fn build_topic_rating_list_query(
    topic_id: &str, page: u32, star: i32, rating_type: &str,
    detail_only: bool, first_item: &str, last_item: &str,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("url", "/feed/nodeRatingList".to_string()),
        ("targetType", "3".to_string()),
        ("targetId", topic_id.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if (1..=5).contains(&star) { query.push(("star", star.to_string())); }
    if matches!(rating_type, "0" | "1") { query.push(("ratingType", rating_type.to_string())); }
    if detail_only { query.push(("v4RatingItem", "1".to_string())); }
    if !first_item.is_empty() { query.push(("firstItem", first_item.to_string())); }
    if !last_item.is_empty() { query.push(("lastItem", last_item.to_string())); }
    query
}

fn build_secondhand_product_list_query(
    brand_id: &str,
    list_type: &str,
    page: u32,
    first_item: &str,
    last_item: &str,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("id", brand_id.to_string()),
        ("listType", list_type.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if !first_item.trim().is_empty() {
        query.push(("firstItem", first_item.to_string()));
    }
    if !last_item.trim().is_empty() {
        query.push(("lastItem", last_item.to_string()));
    }
    query
}

fn build_create_feed_form(
    message: &str,
    pic: Option<&str>,
    post_token: Option<&str>,
) -> Vec<(&'static str, String)> {
    build_create_feed_form_for_type(message, pic, post_token, "feed", "")
}

fn build_create_feed_form_for_type(
    message: &str,
    pic: Option<&str>,
    post_token: Option<&str>,
    feed_type: &str,
    fid: &str,
) -> Vec<(&'static str, String)> {
    let mut form = vec![
        ("id", String::new()),
        ("message", message.to_string()),
        ("type", feed_type.to_string()),
        ("pic", pic.unwrap_or_default().to_string()),
        ("status", "1".to_string()),
        ("publish_status", "0".to_string()),
        ("location", String::new()),
        ("long_location", String::new()),
        ("latitude", "0.0".to_string()),
        ("longitude", "0.0".to_string()),
        ("media_url", String::new()),
        ("media_type", "0".to_string()),
        ("media_pic", String::new()),
        ("message_title", String::new()),
        ("message_brief", String::new()),
        ("extra_title", String::new()),
        ("extra_url", String::new()),
        ("extra_key", String::new()),
        ("extra_pic", String::new()),
        ("extra_info", String::new()),
        ("message_cover", String::new()),
        ("original_type", "0".to_string()),
        ("is_editInDyh", "0".to_string()),
        ("forwardid", String::new()),
        ("fid", fid.to_string()),
        ("dyhId", String::new()),
        ("targetType", String::new()),
        ("productId", String::new()),
        ("targetId", String::new()),
        ("location_city", String::new()),
        ("location_country", String::new()),
        ("disallow_reply", "0".to_string()),
        ("vote_score", "0".to_string()),
        ("replyWithForward", "0".to_string()),
        ("media_info", String::new()),
        ("insert_product_media", "0".to_string()),
        ("is_ks_doc", "0".to_string()),
        ("goods_list_id", String::new()),
        ("is_html_article", "0".to_string()),
    ];
    if let Some(token) = post_token.filter(|value| !value.trim().is_empty()) {
        form.push(("_v2_post_token", token.to_string()));
    }
    form
}

/// 官方转发动态使用 forwardid 指向原动态；fid 留给回答等其他发布类型。
fn build_forward_form(message: &str, pic: Option<&str>, forward_id: &str) -> Vec<(&'static str, String)> {
    let mut form = build_create_feed_form(message, pic, None);
    if let Some((_, value)) = form.iter_mut().find(|(name, _)| *name == "forwardid") {
        *value = forward_id.to_string();
    }
    form
}

/// 设备信息覆盖配置（由设置页"设备信息"下发，作用于 API 请求头和设备身份）。
/// 字段为 None 时使用客户端默认值；全部留空表示恢复默认。
/// device_id 用于 X-App-Device；ddid 仅在服务端要求 DDI 的写接口中作为 Cookie 发送。
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProfile {
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub brand: Option<String>,
    /// 用户手动填写的数盟设备 ID，用于设备码首字段。
    #[serde(default)]
    pub device_id: Option<String>,
    /// 用户手动填写的 DDI 会话值，仅作为指定写接口的 ddid Cookie 发送。
    #[serde(default)]
    pub ddid: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub build: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub sdk_int: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub app_version: Option<String>,
    #[serde(default)]
    pub app_code: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub dark_mode: Option<String>,
}

/// 移动端 UA：酷安网页（账号安全页/移动版页面）在桌面 UA 下会白屏或重定向
const MOBILE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1";

/// 从 HTML 中提取 <title> 文本（大小写不敏感，无正则依赖）
fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    let title = html[start..end].trim().to_string();
    if title.is_empty() { None } else { Some(title) }
}

/// 剔除网页外壳噪音标签（导航/页脚/脚本/样式等），保留正文骨架。
/// 轻量实现：逐标签扫描跳过指定块，未闭合时跳到下一个标签处兜底。
fn strip_noise_tags(html: &str, tags: &[&str]) -> String {
    let lower = html.to_lowercase();
    let mut result = String::with_capacity(html.len());
    let mut pos = 0;
    let n = lower.len();
    while pos < n {
        let Some(rel) = lower[pos..].find('<') else {
            result.push_str(&html[pos..]);
            break;
        };
        let start = pos + rel;
        // 先拷贝 '<' 之前的纯文本
        result.push_str(&html[pos..start]);
        let rest = &lower[start + 1..];
        let name_end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == ':'))
            .unwrap_or(rest.len());
        let name = &rest[..name_end];

        // 注释/声明等空名标签（<!--、<!DOCTYPE>）：仅保留 '<' 字符
        if name.is_empty() {
            result.push('<');
            pos = start + 1;
            continue;
        }

        if tags.contains(&name) {
            let close_tag = format!("</{name}");
            let after_open = start + 1 + name_end;
            let block_end = if let Some(relc) = lower[after_open..].find(&close_tag) {
                let mut end = after_open + relc + close_tag.len();
                if let Some(gt) = lower[end..].find('>') {
                    end += gt + 1;
                }
                end
            } else {
                // 自闭合或无闭合标签：直接跳过该标签，继续找下一个 <
                after_open
            };
            pos = block_end;
        } else {
            // 普通标签原样保留
            result.push('<');
            pos = start + 1;
        }
    }
    result
}

/// 提取单个标签内部的 HTML（取第一个匹配的开闭标签对）
fn extract_tag_content(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let open = format!("<{tag}");
    let start = lower.find(&open)?;
    let open_end = lower[start..].find('>')? + start + 1;
    let close = format!("</{tag}>");
    let rel = lower[open_end..].find(&close)?;
    Some(html[open_end..open_end + rel].to_string())
}

/// 对外部网页做可读性提取：先剥外壳噪音，再优先取 <article>/<main> 正文容器
fn extract_readable_content(html: &str) -> String {
    let cleaned = strip_noise_tags(
        html,
        &[
            "script", "style", "nav", "header", "footer", "aside", "iframe", "form", "noscript",
        ],
    );
    for tag in ["article", "main"] {
        if let Some(inner) = extract_tag_content(&cleaned, tag) {
            return inner;
        }
    }
    cleaned
}

/// 是否属于酷安官方域名：登录 Cookie / App 指纹头等凭据只允许发送给酷安域，
/// 严禁携带到任意第三方域名（防止凭据经外部链接/图片地址泄露）。
fn is_coolapk_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "coolapk.com" || host.ends_with(".coolapk.com")
}

fn is_weibo_video_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "weibocdn.com" || host.ends_with(".weibocdn.com")
}

fn is_weibo_image_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "sinaimg.cn" || host.ends_with(".sinaimg.cn")
}

fn parse_http_url(value: &Value) -> Option<String> {
    let raw = value.as_str()?.trim();
    let parsed = reqwest::Url::parse(raw).ok()?;
    if parsed.scheme() == "http" || parsed.scheme() == "https" {
        Some(raw.to_string())
    } else {
        None
    }
}

/// 从 Live Photo 解析响应中兼容提取视频地址。
/// 酷安不同接口版本可能返回单个 url、data.url 或 data.urlList。
fn extract_live_photo_video_url(value: &Value) -> Option<String> {
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

fn parse_u64_val(val: &Value) -> Option<u64> {
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

fn get_u64_by_keys(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> u64 {
    for k in keys {
        if let Some(val) = obj.get(*k) {
            if let Some(n) = parse_u64_val(val) {
                return n;
            }
        }
    }
    0
}

fn get_str_by_keys(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
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

fn build_collection_list_query(uid: &str, page: u32, first_item: &str, last_item: &str) -> Vec<(&'static str, String)> {
    vec![
        ("uid", uid.to_string()),
        ("showDefault", "1".to_string()),
        ("page", page.to_string()),
        ("firstItem", first_item.to_string()),
        ("lastItem", last_item.to_string()),
    ]
}

/// 应用收藏只选择系统默认收藏单；取消收藏移除全部已选收藏单，重复操作保持幂等。
fn apk_collection_update_plan(collections: &[Value], favorited: bool) -> Result<Option<(String, String)>, String> {
    let flag = |value: Option<&Value>| value.is_some_and(|v| v.as_bool() == Some(true) || v.as_u64() == Some(1) || matches!(v.as_str(), Some("1" | "true")));
    let id = |row: &Value| row.as_object().and_then(|o| get_str_by_keys(o, &["id", "collectionId"]));
    let mut selected = Vec::new();
    for row in collections {
        if flag(row.get("isBeCollected").or_else(|| row.get("is_be_collected"))) {
            let value = id(row).ok_or("应用所在收藏单缺少有效 ID")?;
            if !selected.contains(&value) { selected.push(value); }
        }
    }
    if favorited {
        if !selected.is_empty() { return Ok(None); }
        let default = collections.iter().find(|row| flag(row.get("defaultCollected").or_else(|| row.get("default_collected")).or_else(|| row.get("isDefault"))));
        let collection_id = default.and_then(id).ok_or("未找到酷安默认收藏单，收藏失败")?;
        Ok(Some((collection_id, String::new())))
    } else if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some((String::new(), selected.join(","))))
    }
}

fn build_product_feeds_query(
    product_id: &str,
    feed_type: &str,
    list_type: &str,
    page: u32,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("url", "/page?url=/product/feedList".to_string()),
        ("id", product_id.to_string()),
        ("type", feed_type.to_string()),
    ];
    if !list_type.trim().is_empty() {
        query.push(("listType", list_type.trim().to_string()));
    }
    query.push(("page", page.to_string()));
    query
}

fn build_product_subtab_query(product_id: &str, sub_id: &str, page: u32) -> Vec<(&'static str, String)> {
    vec![
        ("url", "/page?url=/product/feedList".to_string()),
        ("cacheExpires", "60".to_string()),
        ("type", "subTabFeed".to_string()),
        ("withSortCard", "1".to_string()),
        ("withSubTabFeedCard", "1".to_string()),
        ("ignoreEntityById", "1".to_string()),
        ("id", product_id.to_string()),
        ("subId", sub_id.to_string()),
        ("page", page.max(1).to_string()),
    ]
}

fn topic_hub_cursor(value: &Value) -> String {
    value
        .as_object()
        .and_then(|obj| get_str_by_keys(obj, &["entityId", "id"]))
        .unwrap_or_default()
}

fn topic_hub_tab_target(obj: &serde_json::Map<String, Value>) -> Option<String> {
    let target_keys = [
        "url",
        "link",
        "pageUrl",
        "page_url",
        "pageName",
        "page_name",
        "requestArg",
        "request_arg",
        "requestUrl",
        "request_url",
        "apiUrl",
        "api_url",
    ];
    get_str_by_keys(obj, &target_keys).or_else(|| {
        obj.get("extraData")
            .or_else(|| obj.get("extra_data"))
            .and_then(Value::as_object)
            .and_then(|extra| get_str_by_keys(extra, &target_keys))
    })
}

fn append_topic_hub_tab_entities(value: &Value, output: &mut Vec<Value>) {
    let Some(items) = value.as_array() else {
        return;
    };
    for entity in items {
        let Some(entity_obj) = entity.as_object() else {
            continue;
        };
        let title = get_str_by_keys(entity_obj, &["title", "title_txt", "name", "label"]);
        if title.is_some() && topic_hub_tab_target(entity_obj).is_some() {
            output.push(entity.clone());
        }
    }
}

fn collect_topic_hub_tabs(value: &Value, output: &mut Vec<Value>) {
    let Some(obj) = value.as_object() else {
        return;
    };
    let template = obj
        .get("entityTemplate")
        .or_else(|| obj.get("entity_template"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_tab_card = template.contains("icontablink")
        || template.contains("iconlinkgrid")
        || template.contains("verticalcolumnsfullpagecard")
        || template.contains("selectorlinkgrid");
    if is_tab_card {
        if let Some(entities) = obj.get("entities").and_then(Value::as_array) {
            append_topic_hub_tab_entities(&Value::Array(entities.clone()), output);
        }
        if !output.is_empty() {
            return;
        }
    }
    for key in ["entities", "data", "rows", "card", "category", "tabs", "tabList", "tab_list"] {
        let Some(nested) = obj.get(key) else {
            continue;
        };
        if matches!(key, "tabs" | "tabList" | "tab_list") {
            append_topic_hub_tab_entities(nested, output);
        }
        if !output.is_empty() {
            return;
        }
        if let Some(items) = nested.as_array() {
            for entity in items {
                collect_topic_hub_tabs(entity, output);
                if !output.is_empty() {
                    return;
                }
            }
        } else {
            collect_topic_hub_tabs(nested, output);
            if !output.is_empty() {
                return;
            }
        }
    }
}

fn extract_topic_hub_tabs(raw: &Value) -> Value {
    let mut tabs = Vec::new();
    for key in ["tabs", "tabList", "tab_list", "categories", "category"] {
        if let Some(value) = raw.get(key) {
            append_topic_hub_tab_entities(value, &mut tabs);
            if !tabs.is_empty() {
                return Value::Array(tabs);
            }
        }
    }
    if let Some(data) = raw.get("data").and_then(Value::as_array) {
        for item in data {
            collect_topic_hub_tabs(item, &mut tabs);
            if !tabs.is_empty() {
                break;
            }
        }
    }
    Value::Array(tabs)
}

fn topic_hub_selected_tab(value: &Value) -> Option<String> {
    match value {
        Value::Array(items) => items.iter().find_map(topic_hub_selected_tab),
        Value::Object(obj) => {
            for key in ["extraData", "extra_data"] {
                let Some(extra) = obj.get(key) else {
                    continue;
                };
                if let Some(selected) = topic_hub_selected_tab(extra) {
                    return Some(selected);
                }
            }
            if let Some(selected) = get_str_by_keys(obj, &["selectedTab", "selected_tab"]) {
                return Some(selected);
            }
            obj.values().find_map(topic_hub_selected_tab)
        }
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|parsed| topic_hub_selected_tab(&parsed)),
        _ => None,
    }
}

fn topic_hub_selected_category(raw: &Value) -> Option<(String, String)> {
    let tabs = extract_topic_hub_tabs(raw);
    let items = tabs.as_array()?;
    let selected_title = topic_hub_selected_tab(raw).unwrap_or_default();
    let selected = items
        .iter()
        .find(|item| get_str_by_keys(item.as_object().unwrap_or(&serde_json::Map::new()), &["title", "name", "label"]).as_deref() == Some(selected_title.as_str()))
        .or_else(|| items.first())?;
    let selected_obj = selected.as_object()?;
    let title = get_str_by_keys(selected_obj, &["title", "name", "label"])?;
    let url = topic_hub_tab_target(selected_obj)?;
    Some((title, url))
}

fn topic_hub_result(raw: &Value) -> Value {
    let data = raw.get("data").cloned().unwrap_or_else(|| json!([]));
    let (first_item, last_item) = data
        .as_array()
        .map(|items| {
            (
                items.first().map(topic_hub_cursor).unwrap_or_default(),
                items.last().map(topic_hub_cursor).unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    json!({ "code": 200, "data": data, "firstItem": first_item, "lastItem": last_item, "tabs": extract_topic_hub_tabs(raw) })
}

fn first_value_by_keys<'a>(obj: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| obj.get(*key))
}

fn has_non_empty_json_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(_) | Value::Number(_) => true,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(values) => values.iter().any(has_non_empty_json_value),
        Value::Object(values) => values.values().any(has_non_empty_json_value),
    }
}

fn has_any_non_empty_field(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> bool {
    first_value_by_keys(obj, keys).map_or(false, has_non_empty_json_value)
}

fn copy_first_field(cleaned: &mut Value, obj: &serde_json::Map<String, Value>, output_key: &str, keys: &[&str]) {
    let Some(value) = first_value_by_keys(obj, keys) else {
        return;
    };
    if let Some(cleaned_obj) = cleaned.as_object_mut() {
        cleaned_obj.insert(output_key.to_string(), value.clone());
    }
}

fn normalize_coolapk_image_url(value: &str) -> Option<String> {
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

fn image_source_url(value: &Value) -> Option<String> {
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

/// 构造 HTTP 客户端：Apple 平台改用系统信任库校验 TLS 证书，
/// 使 Rust 侧与 WKWebView 对系统根证书（含本机 MITM 代理、企业 CA）的信任决策一致。
/// 其余平台保持 reqwest 默认的内置 Mozilla 根证书。
pub(crate) fn http_client_builder() -> reqwest::ClientBuilder {
    super::network_proxy::configure(base_http_client_builder())
}

pub(crate) fn base_http_client_builder() -> reqwest::ClientBuilder {
    let builder = Client::builder();
    #[cfg(target_vendor = "apple")]
    let builder = match apple_platform_tls_config() {
        Some(config) => builder.use_preconfigured_tls(config),
        None => builder,
    };
    builder
}

fn build_api_clients(profile: &DeviceProfile) -> Result<(Client, Client), String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, profile.user_agent.as_deref().and_then(|value| HeaderValue::from_str(value).ok()).unwrap_or_else(|| HeaderValue::from_static(super::native_device::COMPATIBILITY_UA)));
    headers.insert("X-Sdk-Int", profile.sdk_int.as_deref().and_then(|value| HeaderValue::from_str(value).ok()).unwrap_or_else(|| HeaderValue::from_static("36")));
    headers.insert("X-Sdk-Locale", HeaderValue::from_static("zh-CN"));
    headers.insert("X-App-Mode", HeaderValue::from_static("universal"));
    headers.insert("X-App-Channel", HeaderValue::from_static("coolapk"));
    headers.insert("X-App-Id", HeaderValue::from_static("com.coolapk.market"));
    headers.insert("X-App-Version", HeaderValue::from_static("16.6.5"));
    headers.insert("X-App-Code", HeaderValue::from(APP_CODE));
    headers.insert("X-Api-Version", HeaderValue::from_static("16"));
    headers.insert("X-App-Supported", HeaderValue::from(APP_CODE));
    headers.insert("X-Dark-Mode", HeaderValue::from_static("0"));
    let client = http_client_builder().default_headers(headers.clone()).build().map_err(|_| "创建酷安 HTTP 客户端失败".to_string())?;
    let redirect_client = http_client_builder().default_headers(headers).redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "创建酷安重定向客户端失败".to_string())?;
    Ok((client, redirect_client))
}

/// Apple 平台经 Security.framework 校验证书；初始化失败时回退内置根证书，
/// 避免客户端直接不可用（此时行为与旧版本一致）。
#[cfg(target_vendor = "apple")]
fn apple_platform_tls_config() -> Option<rustls::ClientConfig> {
    use rustls_platform_verifier::ConfigVerifierExt;
    match rustls::ClientConfig::with_platform_verifier() {
        Ok(config) => Some(config),
        Err(error) => {
            log::warn!("tls.platform_verifier_unavailable reason={error}");
            None
        }
    }
}


// ===== 拆分后的 impl 子模块（每个文件一个 impl CoolapkClient 块） =====
mod core;
mod normalize;
mod feeds;
mod media_plugin;
mod feed_detail;
mod user;
mod collection;
mod social;
mod reads_misc;
mod messages;
mod write_feed_media;
mod follow_actions;
mod publish_auth;
mod discovery;
mod product;
mod dyh_goods;
mod album_apk;
mod goods;


/// 检查设备码是否符合官方结构（Base64 逆序解码后包含设备信息字段分号分隔符）
fn is_valid_device_code(code: &str) -> bool {
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
fn device_code_without_model(code: &str) -> Result<String, String> {
    let mut encoded: String = code.chars().rev().collect();
    while encoded.len() % 4 != 0 { encoded.push('='); }
    let raw = BASE64.decode(encoded).ok().and_then(|bytes| String::from_utf8(bytes).ok()).ok_or("发布设备码格式无效")?;
    let mut fields: Vec<&str> = raw.split(';').map(str::trim).collect();
    if fields.len() != 9 { return Err("发布设备码格式无效".into()); }
    // 保留设备标识/OAID 等身份字段，仅清除厂商、品牌、机型和 Build。
    fields[4..8].fill("");
    Ok(BASE64.encode(fields.join("; ")).chars().rev().filter(|value| *value != '=').collect())
}

fn device_code_with_profile(code: &str, profile: &DeviceProfile) -> String {
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

fn generate_device_code_with_device_id(
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
fn generate_device_code_for_id(uid: &str) -> String {
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
fn generate_random_device_code() -> String {
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

fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .unwrap_or_default()
}

fn value_to_string_opt(value: &Value) -> Option<String> {
    let s = value_to_string(value);
    if s.is_empty() { None } else { Some(s) }
}

/// 从评论详情的酷安 UA 中提取真实设备代号、系统版本与构建号。
/// 接口经常保留 useragent，却把 device_title 等便捷字段返回为空。
fn parse_reply_user_agent(user_agent: &str) -> (String, String, String) {
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

fn rank_feed_url(rank_type: &str) -> Option<&'static str> {
    match rank_type {
        // 实测：月榜 statType=30days 有效（6659/3676/3630），statType=month 返回空
        "month" => Some("#/feed/statList?statType=30days&sortField=likenum"),
        // 实测：收藏榜必须 statType=7days（favnum 排序），statType=all 返回空
        "favorite" => Some("#/feed/statList?statType=7days&sortField=favnum"),
        "index" => Some("#/feed/statList?statType=7days&sortField=detailnum"),
        _ => None,
    }
}

// 评论失败必须保留验证码配置，不能把缺少 data 的错误响应转换成空列表。
fn comment_list_data(response: Value) -> Result<Vec<Value>, String> {
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

fn append_comment_captcha_token<'a>(query: &mut Vec<(&'a str, String)>, token: Option<&str>, field: Option<&'a str>) {
    // APK：captchaField 为空时使用 validate Cookie，不添加查询字段。
    if field.is_some_and(|value| value.trim().is_empty()) { return; }
    if let Some(token) = token.filter(|value| !value.trim().is_empty()) {
        let field = field.filter(|value| value.starts_with('_') && value.len() <= 64 && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
            .unwrap_or("_v2_post_token");
        query.push((field, token.to_string()));
    }
}

fn cookie_with_captcha_validation(cookie: &str, token: Option<&str>) -> String {
    let Some(token) = token.filter(|value| !value.trim().is_empty()) else { return cookie.to_string(); };
    // 与 Android URLEncoder 一致地编码凭证，不输出到日志。
    let mut encoded = reqwest::Url::parse("https://api.coolapk.com").expect("static API URL");
    encoded.query_pairs_mut().append_pair("validate", token);
    let mut cookies: Vec<&str> = cookie.split(';').map(str::trim).filter(|value| !value.is_empty()
        && value.split_once('=').is_none_or(|(name, _)| name.trim() != "validate")).collect();
    cookies.push(encoded.query().unwrap_or_default());
    cookies.join("; ")
}

fn wrap_api_data(response: Value) -> Result<Value, String> {
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
fn wrap_page_data_response(response: Value) -> Result<Value, String> {
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
fn parse_discovery_request_args(raw: &str) -> Vec<(String, String)> {
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

fn is_safe_discovery_page_url(value: &str) -> bool {
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

#[cfg(test)]
mod discovery_url_tests {
    use super::is_safe_discovery_page_url;

    #[test]
    fn accepts_server_page_routes() {
        assert!(is_safe_discovery_page_url("V11_FIND_COOLPIC"));
        assert!(is_safe_discovery_page_url("#/feed/digestList?page=1"));
        assert!(is_safe_discovery_page_url("/page?url=/product/feedList"));
    }

    #[test]
    fn rejects_external_and_script_urls() {
        assert!(!is_safe_discovery_page_url("https://example.com"));
        assert!(!is_safe_discovery_page_url("javascript:alert(1)"));
        assert!(!is_safe_discovery_page_url("file:///C:/secret"));
        assert!(!is_safe_discovery_page_url("bad\nroute"));
    }
}

#[cfg(test)]
mod path_requirements_tests {
    use super::classify_path;

    #[test]
    fn ddi_paths_match_server_config() {
        assert!(classify_path("/v6/feed/like").needs_ddid);
        assert!(classify_path("/v6/feed/likeReply").needs_ddid);
        assert!(!classify_path("/v6/feed/followTag").needs_ddid);
        assert!(!classify_path("/v6/feed/unFollowTag").needs_ddid);
    }
}

/// 传输失败时把错误的 source 链拼成一行，便于定位 DNS、证书、代理等真因。
/// 只记录错误类别文本；该链不含请求 URL、Cookie 或响应体。
fn error_cause_chain(error: &(dyn std::error::Error + 'static)) -> String {
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
struct ApiFailureLogLimiter {
    // 仅保存固定请求路径和错误类别，限制不同键的数量，避免日志设施持有无界状态。
    entries: std::collections::HashMap<String, (Option<std::time::Instant>, u64, u64)>,
}

impl ApiFailureLogLimiter {
    fn record(&mut self, key: String, now: std::time::Instant) -> Option<(u64, u64)> {
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

fn log_api_failure_limited(event: &str, method: &str, origin: &str, path: &str, reason: &str, detail: &str) {
    static LIMITER: std::sync::OnceLock<std::sync::Mutex<ApiFailureLogLimiter>> = std::sync::OnceLock::new();
    let counts = LIMITER.get_or_init(Default::default).lock().ok()
        .and_then(|mut limiter| limiter.record(format!("{event}:{method}:{origin}:{path}:{reason}"), std::time::Instant::now()));
    if let Some((total, suppressed)) = counts {
        log::warn!("{event} method={method} origin={origin} path={path} reason={reason} failures_total={total} suppressed_failures={suppressed} {detail}");
    }
}

fn classify_api_failure(text: &str) -> &'static str {
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

fn application_failure(value: &Value) -> Option<(i64, &'static str)> {
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

async fn response_json(response: reqwest::Response) -> Result<Value, String> {
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

#[cfg(test)]
mod api_failure_log_tests {
    use super::{application_failure, classify_api_failure, ApiFailureLogLimiter};
    use serde_json::json;

    #[test]
    fn classifies_failure_without_exposing_server_message() {
        assert_eq!(application_failure(&json!({"code": 403, "messageStatus": "err_request_captcha_v2"})), Some((403, "captcha")));
        assert_eq!(application_failure(&json!({"code": -415, "message": "访问过于频繁"})), Some((-415, "rate_limit")));
        assert_eq!(application_failure(&json!({"code": 200, "message": "ok"})), None);
        assert_eq!(application_failure(&json!({"status": 1004, "message": "请求验证错误"})), Some((1004, "request_validation")));
        assert_eq!(application_failure(&json!({"code": 200, "status": -3, "message": "请求验证错误"})), Some((-3, "request_validation")));
        assert_eq!(application_failure(&json!({"status": 1, "data": {}})), None);
        assert_eq!(classify_api_failure("Coolapk API returned HTTP 403: empty response body"), "empty_body");
    }

    #[test]
    fn limits_repeated_failures_but_preserves_counts_and_other_endpoints() {
        let mut limiter = ApiFailureLogLimiter::default();
        let now = std::time::Instant::now();
        assert_eq!(limiter.record("guest:space:validation".into(), now), Some((1, 0)));
        for _ in 0..20 { assert_eq!(limiter.record("guest:space:validation".into(), now), None); }
        assert_eq!(limiter.record("notification:dns".into(), now), Some((1, 0)));
        assert_eq!(limiter.record("guest:space:validation".into(), now + std::time::Duration::from_secs(60)), Some((22, 20)));
        for i in 0..200 { limiter.record(format!("bounded:{i}"), now); }
        assert_eq!(limiter.entries.len(), 128);
    }
}

#[cfg(test)]
#[path = "../client_tests.rs"]
mod tests;

#[cfg(test)]
mod download_file_tests {
    use super::CoolapkClient;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn generic_download_stream_keeps_bytes_and_rejects_truncated_response() {
        for truncated in [false, true] {
            // 本地响应仅用于验证流式写入，生产下载仍使用原有地址校验。
            let payload: Vec<u8> = (0..128 * 1024).map(|index| (index % 256) as u8).collect();
            let expected = payload.clone();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                stream.read(&mut request).await.unwrap();
                let declared_size = payload.len() + usize::from(truncated);
                let header = format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=test.custom\r\nContent-Length: {declared_size}\r\nConnection: close\r\n\r\n");
                stream.write_all(header.as_bytes()).await.unwrap();
                for chunk in payload.chunks(4096) { stream.write_all(chunk).await.unwrap(); }
            });
            let path = std::env::temp_dir().join(format!("coolapk-download-{}-{truncated}.part", address.port()));
            let response = reqwest::Client::new().get(format!("http://{address}/test.custom")).send().await.unwrap();
            let result = CoolapkClient::write_download_response(response, &path).await;
            server.await.unwrap();
            if truncated { assert!(result.is_err()); }
            else {
                assert_eq!(result.unwrap().0, "application/octet-stream");
                assert_eq!(tokio::fs::read(&path).await.unwrap(), expected);
            }
            tokio::fs::remove_file(path).await.unwrap();
        }
    }
}

#[cfg(test)]
#[path = "../api_tests.rs"]
mod api_tests;
