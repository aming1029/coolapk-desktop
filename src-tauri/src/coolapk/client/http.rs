use super::*;

pub(super) fn build_api_clients(profile: &DeviceProfile) -> Result<(Client, Client), String> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, profile.user_agent.as_deref().and_then(|value| HeaderValue::from_str(value).ok()).unwrap_or_else(|| HeaderValue::from_static(crate::coolapk::native_device::COMPATIBILITY_UA)));
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
pub(super) fn apple_platform_tls_config() -> Option<rustls::ClientConfig> {
    use rustls_platform_verifier::ConfigVerifierExt;
    match rustls::ClientConfig::with_platform_verifier() {
        Ok(config) => Some(config),
        Err(error) => {
            log::warn!("tls.platform_verifier_unavailable reason={error}");
            None
        }
    }
}
