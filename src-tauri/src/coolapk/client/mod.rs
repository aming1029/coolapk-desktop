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


// ===== 拆分的自由函数子模块 =====
mod publish;
mod cookies;
mod device;
mod media;
mod html;
mod query;
mod topic_hub;
mod jsonx;
mod response;
mod http;
use publish::*;
use cookies::*;
use device::*;
use media::*;
use html::*;
use query::*;
use topic_hub::*;
use jsonx::*;
use response::*;
use http::*;