use super::http_session::{session, Policy as SessionPolicy, UPDATE_ALLOWED_HOSTS};
use crate::coolapk::client::{CoolapkClient, DeviceProfile};
use crate::download_manager::{DownloadControl, DownloadManager};
use crate::diagnostics::{login_checkpoint, LoginStage};
use base64::{Engine as _, engine::general_purpose::{STANDARD as BASE64, STANDARD_NO_PAD as BASE64_NO_PAD}};
use md5::{Digest, Md5};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tauri::{Emitter, Manager, State};

pub struct AppState {
    pub client: CoolapkClient,
    pub downloads: DownloadManager,
    pub login_session: Mutex<Option<std::sync::Arc<LoginSession>>>,
}

#[derive(Default)]
pub struct LoginSession {
    callback_url: Mutex<Option<String>>,
    verification: tokio::sync::Mutex<LoginVerification>,
}

#[derive(Default)]
struct LoginVerification {
    attempted_access_callback: Option<String>,
    completed: bool,
    #[cfg(any(target_os = "ios", test))]
    ios_cookie_fingerprint: Option<String>,
    #[cfg(any(target_os = "ios", test))]
    ios_cookie_attempts: u8,
    #[cfg(any(target_os = "ios", test))]
    ios_cookie_next_retry: Option<Instant>,
}

impl LoginVerification {
    fn should_exchange(&self, callback: &str, has_session: bool) -> bool {
        has_session && self.attempted_access_callback.as_deref() != Some(callback)
    }

    /// 没有 Cookie 的等待不计次数；同一会话失败四次后暂停十秒，登录后仍可继续同步。
    #[cfg(any(target_os = "ios", test))]
    fn should_verify_ios_cookie(&mut self, cookie: &str, callback: Option<&str>) -> bool {
        let mut pairs = cookie.split(';').map(str::trim).collect::<Vec<_>>();
        pairs.sort_unstable();
        let fingerprint = format!("{:x}", Md5::digest(format!("{}\n{}", pairs.join(";"), callback.unwrap_or_default()).as_bytes()));
        if self.ios_cookie_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            self.ios_cookie_fingerprint = Some(fingerprint);
            self.ios_cookie_attempts = 0;
            self.ios_cookie_next_retry = None;
        }
        if self.ios_cookie_attempts >= 4 {
            if self.ios_cookie_next_retry.is_some_and(|at| Instant::now() < at) { return false; }
            self.ios_cookie_attempts = 0;
        }
        self.ios_cookie_attempts += 1;
        if self.ios_cookie_attempts == 4 { self.ios_cookie_next_retry = Some(Instant::now() + Duration::from_secs(10)); }
        true
    }
}

/// iOS 官方页面可能在原 URL 内完成短信登录，不依赖网页是否发生跳转。
#[cfg(any(target_os = "ios", test))]
fn should_poll_ios_login_cookies(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" { return false; }
    match url.host_str() {
        Some("account.coolapk.com") => matches!(url.path().trim_end_matches('/'), "/auth/login" | "/auth/callback"),
        Some("www.coolapk.com" | "m.coolapk.com" | "coolapk.com") => true,
        _ => false,
    }
}

/// 返回按钮只使用固定导航标记，官方远程页面仍不具备 IPC 权限。
#[cfg(any(target_os = "ios", test))]
fn is_ios_login_return_url(url: &reqwest::Url) -> bool {
    url.scheme() == "coolapk-login" && url.host_str() == Some("return") && matches!(url.path(), "" | "/") && url.username().is_empty() && url.password().is_none() && url.port().is_none() && url.query().is_none() && url.fragment().is_none()
}

/// 按域名优先级合并 Cookie，account 域覆盖其他子域的同名旧值。
#[cfg(any(target_os = "ios", test))]
fn merge_ios_login_cookies(mut cookies: Vec<(String, String, String)>) -> String {
    cookies.retain(|(domain, name, value)| is_coolapk_cookie_domain(domain) && (name != "SESSID" || CoolapkClient::has_valid_session_cookie(&format!("SESSID={value}"))));
    cookies.sort_by_key(|(domain, name, value)| {
        let domain = domain.trim_start_matches('.').to_ascii_lowercase();
        let priority = match domain.as_str() { "account.coolapk.com" => 3, "coolapk.com" => 2, _ => 1 };
        (priority, domain, name.clone(), value.clone())
    });
    let header = cookies.into_iter().map(|(_, name, value)| format!("{name}={value}")).collect::<Vec<_>>().join("; ");
    merge_cookie_headers(None, Some(&header)).unwrap_or_default()
}

static IMAGE_SAVE_LOCK: Mutex<()> = Mutex::new(());
static IMAGE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 登录窗口使用桌面 Chromium UA，避免网易易盾把鼠标事件误判为仅支持触摸事件。
const LOGIN_WEBVIEW_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";

const APK_DOWNLOAD_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;

fn download_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn download_object_string(value: Option<&Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| value.and_then(|item| item.get(*key)).and_then(download_value_to_string))
}

fn decode_extra_analysis_data(value: &str) -> Option<Value> {
    let encoded = value.split('~').next()?.trim();
    if encoded.is_empty() {
        return None;
    }
    let bytes = BASE64
        .decode(encoded)
        .or_else(|_| BASE64_NO_PAD.decode(encoded))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn build_coolapk_download_url(package_name: &str, apk_id: &str, version_code: &str) -> Result<reqwest::Url, String> {
    let package_name = package_name.trim();
    let apk_id = apk_id.trim();
    let version_code = version_code.trim();
    if package_name.is_empty() || apk_id.is_empty() || version_code.is_empty() {
        return Err("应用下载参数不完整，缺少包名、应用 ID 或版本号".to_string());
    }
    let mut url = reqwest::Url::parse("https://api.coolapk.com/v6/apk/download")
        .map_err(|error| format!("构造酷安下载地址失败：{error}"))?;
    url.query_pairs_mut()
        .append_pair("pn", package_name)
        .append_pair("aid", apk_id)
        .append_pair("vc", version_code)
        .append_pair("extra", "");
    Ok(url)
}

fn is_coolapk_download_host(host: &str) -> bool {
    matches!(host.to_ascii_lowercase().as_str(), "api.coolapk.com" | "api-dev.coolapk.com")
}

fn is_windows_reserved_file_name(file_name: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let base_name = file_name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(|character| character == ' ' || character == '.')
        .to_ascii_uppercase();
    if matches!(base_name.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let bytes = base_name.as_bytes();
    (bytes.len() == 4 && (bytes.starts_with(b"COM") || bytes.starts_with(b"LPT")))
        && (b'1'..=b'9').contains(&bytes[3])
}

fn sanitize_apk_file_name(file_name: &str) -> Result<String, String> {
    let mut safe_name = file_name
        .chars()
        .take(160)
        .map(|character| {
            if character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                character
            }
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();
    if safe_name.is_empty() || safe_name == "." || safe_name == ".." || safe_name.contains("..") {
        return Err("下载文件名不合法".to_string());
    }
    let lower_name = safe_name.to_ascii_lowercase();
    if ![".apk", ".xapk", ".apks"].iter().any(|suffix| lower_name.ends_with(suffix)) {
        safe_name.push_str(".apk");
    }
    if is_windows_reserved_file_name(&safe_name) {
        safe_name.insert(0, '_');
    }
    Ok(safe_name)
}

fn constrain_windows_download_file_name(target_dir: &Path, file_name: String) -> Result<String, String> {
    if !cfg!(windows) {
        return Ok(file_name);
    }
    const MAX_WINDOWS_PATH_UNITS: usize = 240;
    let path = Path::new(&file_name);
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or("apk");
    let suffix = format!(".{extension}");
    let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("coolapk");
    let directory_units = target_dir.as_os_str().to_string_lossy().encode_utf16().count();
    let reserved_units = directory_units + 1 + suffix.encode_utf16().count() + ".part".encode_utf16().count();
    let max_stem_units = MAX_WINDOWS_PATH_UNITS.saturating_sub(reserved_units);
    if max_stem_units == 0 {
        return Err("下载目录路径过长，请选择更短的目录".to_string());
    }
    let mut shortened_stem = String::new();
    let mut used_units = 0;
    for character in stem.chars() {
        let units = character.len_utf16();
        if used_units + units > max_stem_units {
            break;
        }
        shortened_stem.push(character);
        used_units += units;
    }
    if shortened_stem.is_empty() {
        return Err("下载文件名过长且无法缩短".to_string());
    }
    Ok(format!("{shortened_stem}{suffix}"))
}

fn partial_download_path(target: &Path) -> Result<PathBuf, String> {
    let file_name = target.file_name().ok_or_else(|| "下载文件路径不合法".to_string())?;
    let mut partial_name = file_name.to_os_string();
    partial_name.push(".part");
    Ok(target.with_file_name(partial_name))
}

fn paths_equivalent(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy().eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn path_is_direct_child_of(path: &Path, target_dir: &Path) -> Result<bool, String> {
    let parent = path.parent().ok_or_else(|| "下载文件路径不合法".to_string())?;
    let canonical_dir = std::fs::canonicalize(target_dir).map_err(|_| "下载目录不存在或无法访问".to_string())?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|_| "下载文件所在目录不存在或无法访问".to_string())?;
    Ok(paths_equivalent(&canonical_dir, &canonical_parent))
}

fn reject_download_symlink(path: &Path) -> Result<(), String> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err("拒绝操作符号链接下载文件".to_string());
        }
    }
    Ok(())
}

fn empty_download_verification(value: &Value) -> bool {
    match value.get("data") {
        None | Some(Value::Null) => true,
        Some(Value::String(data)) => data.trim().is_empty(),
        Some(_) => false,
    }
}

fn download_event_payload(
    task_id: &str,
    status: &str,
    downloaded: u64,
    total: u64,
    speed: u64,
    target_path: &std::path::Path,
    partial_path: &std::path::Path,
    error: Option<&str>,
) -> Value {
    let mut payload = json!({
        "taskId": task_id,
        "status": status,
        "downloaded": downloaded,
        "total": total,
        "speed": speed,
        "path": target_path.to_string_lossy(),
        "partialPath": partial_path.to_string_lossy(),
    });
    if let Some(error) = error {
        payload["error"] = json!(error);
    }
    payload
}

fn download_control(control: &tokio::sync::watch::Receiver<DownloadControl>) -> DownloadControl {
    *control.borrow()
}

async fn run_apk_download(
    app: &tauri::AppHandle,
    client: &CoolapkClient,
    control: &mut tokio::sync::watch::Receiver<DownloadControl>,
    task_id: &str,
    package_name: &str,
    apk_name: &str,
    apk_id: Option<&str>,
    version_code: Option<&str>,
    file_name: &str,
    dir: Option<&str>,
    target_path: Option<&str>,
    extra_analysis_data: Option<&str>,
) -> Result<Value, String> {
    use reqwest::header::{ACCEPT_ENCODING, CONTENT_TYPE, COOKIE, RANGE};
    use tauri::Emitter;
    use tokio::io::AsyncWriteExt;

    let target_dir = user_save_dir(app, dir)?;
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|error| format!("创建下载目录失败：{error}"))?;
    let safe_file_name = constrain_windows_download_file_name(&target_dir, sanitize_apk_file_name(file_name)?)?;
    let target = if let Some(raw_path) = target_path.map(str::trim).filter(|value| !value.is_empty()) {
        let path = validate_download_path_for_file_operation(raw_path)?;
        if !path_is_direct_child_of(&path, &target_dir)? {
            return Err("下载文件必须位于当前下载目录中".to_string());
        }
        reject_download_symlink(&path)?;
        path
    } else {
        next_available_file_path(&target_dir, &safe_file_name)
    };
    let partial = partial_download_path(&target)?;
    reject_download_symlink(&partial)?;
    if target.is_file() && !partial.is_file() {
        return Err(format!("目标文件已经存在：{}", target.display()));
    }
    let existing_length = tokio::fs::metadata(&partial)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if existing_length == 0 && !partial.exists() {
        // 先创建唯一的临时文件占位，避免两个并发任务在网络请求期间选中同一个目标路径。
        let reservation = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .await
            .map_err(|error| format!("创建临时文件失败：{error}"))?;
        drop(reservation);
    }
    let _ = app.emit(
        "apk-download-progress",
        download_event_payload(task_id, "starting", existing_length, 0, 0, &target, &partial, None),
    );

    // 官方客户端先从应用详情取得数字应用 ID 和版本号，再请求 /v6/apk/download。
    // /v6/apk/url 返回的是网页跳转地址，不能当作安装包下载地址。
    let requested_apk_id = apk_id.map(str::trim).filter(|value| !value.is_empty());
    let requested_version_code = version_code.map(str::trim).filter(|value| !value.is_empty());
    let detail = if requested_apk_id.is_none() || requested_version_code.is_none() {
        Some(client.get_app_detail(package_name).await?)
    } else {
        None
    };
    let detail_data = detail.as_ref().and_then(|value| value.get("data"));
    let decoded_extra = extra_analysis_data
        .and_then(decode_extra_analysis_data)
        .or_else(|| detail_data.and_then(|value| value.get("extraAnalysisData")).and_then(Value::as_str).and_then(decode_extra_analysis_data));
    let resolved_apk_id = requested_apk_id
        .map(str::to_string)
        .or_else(|| download_object_string(detail_data, &["aid", "id", "entityId"]))
        .ok_or_else(|| "应用详情未返回数字应用 ID".to_string())?;
    let resolved_version_code = requested_version_code
        .map(str::to_string)
        .or_else(|| download_object_string(detail_data, &["versionCode", "versioncode", "version_code", "apkversioncode", "apkVersionCode", "apk_version_code"]))
        .or_else(|| download_object_string(decoded_extra.as_ref(), &["versionCode", "versioncode", "version_code"]))
        .ok_or_else(|| "应用详情未返回应用版本号".to_string())?;
    let request_url = build_coolapk_download_url(package_name, &resolved_apk_id, &resolved_version_code)?;
    let host = request_url.host_str().unwrap_or_default().to_string();
    let is_coolapk_download = is_coolapk_download_host(&host);

    if download_control(control) == DownloadControl::Cancel {
        let _ = tokio::fs::remove_file(&partial).await;
        let _ = app.emit(
            "apk-download-progress",
            download_event_payload(task_id, "canceled", 0, 0, 0, &target, &partial, None),
        );
        return Ok(json!({ "status": "canceled", "path": target, "partialPath": partial }));
    }

    let http_client = session(request_url.as_str(), SessionPolicy::Download)?;
    let mut request = if is_coolapk_download {
        http_client
            .post(request_url.clone())
            .form(&[("nd", "1"), ("extraAnalysisData", "")])
    } else {
        http_client.get(request_url.clone())
    };
    if is_coolapk_download {
        request = client.apply_download_headers(request)?;
        if let Some(cookie) = client.get_user_cookie().filter(|value| !value.trim().is_empty()) {
            let header = reqwest::header::HeaderValue::from_str(&cookie)
                .map_err(|_| "登录 Cookie 格式无效".to_string())?;
            request = request.header(COOKIE, header);
        }
    }
    // 反编译 APK 的下载器无论是否断点续传都会发送这两个请求头。
    request = request
        .header(RANGE, format!("bytes={existing_length}-"))
        .header(ACCEPT_ENCODING, "identity");
    let mut response = request
        .send()
        .await
        .map_err(|error| format!("下载请求失败：{error}"))?;
    if response.status().as_u16() == 416 && existing_length > 0 {
        drop(response);
        let _ = tokio::fs::remove_file(&partial).await;
        return Err("服务器拒绝断点续传，已清理临时文件，请重试下载".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("下载失败：HTTP {}", response.status()));
    }
    let response_status = response.status();
    let append = existing_length > 0 && response_status.as_u16() == 206;
    let initial_downloaded = if append { existing_length } else { 0 };
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if content_type.starts_with("text/html") || content_type.starts_with("application/xhtml+xml") {
        return Err("下载响应是网页内容，不是应用安装包，已拒绝保存".to_string());
    }
    let final_url = response.url().to_string();
    if is_coolapk_download {
        // 反编译 APK 的 CoolMarketDownloadNetworkExecutor 会在收到响应后调用
        // downloadVerify；接口异常时官方会继续下载，只有明确返回空结果才判定为劫持。
        if let Ok(verification) = client
            .verify_apk_download(apk_name, request_url.as_str(), &final_url)
            .await
        {
            if empty_download_verification(&verification) {
                return Err("酷安下载校验未通过，已拒绝保存安装包".to_string());
            }
        }
    }
    let total = if append {
        response
            .content_length()
            .map(|length| length.saturating_add(existing_length))
            .unwrap_or(0)
    } else {
        response.content_length().unwrap_or(0)
    };
    if total > APK_DOWNLOAD_MAX_BYTES {
        return Err("安装包体积超过 8GB，已拒绝下载".to_string());
    }
    let mut file = if append {
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&partial)
            .await
            .map_err(|error| format!("打开断点文件失败：{error}"))?
    } else {
        tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&partial)
            .await
            .map_err(|error| format!("创建临时文件失败：{error}"))?
    };
    let mut downloaded = initial_downloaded;
    let started_at = Instant::now();
    loop {
        match download_control(control) {
            DownloadControl::Pause => {
                drop(file);
                let _ = app.emit(
                    "apk-download-progress",
                    download_event_payload(task_id, "paused", downloaded, total, 0, &target, &partial, None),
                );
                return Ok(json!({ "status": "paused", "downloaded": downloaded, "total": total, "path": target, "partialPath": partial }));
            }
            DownloadControl::Cancel => {
                drop(file);
                let _ = tokio::fs::remove_file(&partial).await;
                let _ = app.emit(
                    "apk-download-progress",
                    download_event_payload(task_id, "canceled", 0, total, 0, &target, &partial, None),
                );
                return Ok(json!({ "status": "canceled", "downloaded": 0, "total": total, "path": target, "partialPath": partial }));
            }
            DownloadControl::Run => {}
        }
        let chunk = tokio::select! {
            result = response.chunk() => result.map_err(|error| format!("读取下载数据失败：{error}"))?,
            changed = control.changed() => {
                changed.map_err(|_| "下载任务控制器已关闭".to_string())?;
                continue;
            }
        };
        let Some(chunk) = chunk else { break };
        if chunk.is_empty() {
            continue;
        }
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > APK_DOWNLOAD_MAX_BYTES {
            drop(file);
            return Err("安装包体积超过 8GB，已中止下载".to_string());
        }
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("写入临时文件失败：{error}"))?;
        let speed = downloaded / started_at.elapsed().as_secs().max(1);
        let _ = app.emit(
            "apk-download-progress",
            download_event_payload(task_id, "downloading", downloaded, total, speed, &target, &partial, None),
        );
    }
    file.flush().await.map_err(|error| format!("刷新临时文件失败：{error}"))?;
    file.sync_all().await.map_err(|error| format!("同步临时文件失败：{error}"))?;
    drop(file);
    if total > 0 && downloaded != total {
        return Err(format!("下载中断：已下载 {downloaded}/{total} 字节"));
    }
    tokio::fs::rename(&partial, &target)
        .await
        .map_err(|error| format!("保存安装包失败：{error}"))?;
    #[cfg(not(target_os = "android"))]
    let published_path: Option<String> = None;
    #[cfg(target_os = "android")]
    let published_path = match publish_user_file(app, &target).await {
        Ok(location) => Some(location),
        Err(error) => {
        // 原始安装包仍可安装、打开或再次导出，不把已经完成的下载标记为失败。
        log::warn!("安装包已下载，公共目录保存失败：{error}");
        let _ = app.emit("android-file-save-error", &error);
        None
        }
    };
    let speed = downloaded / started_at.elapsed().as_secs().max(1);
    let mut completed = download_event_payload(task_id, "completed", downloaded, total, speed, &target, &partial, None);
    completed["publicPath"] = json!(published_path);
    let _ = app.emit(
        "apk-download-progress",
        completed,
    );
    Ok(json!({ "status": "completed", "downloaded": downloaded, "total": total, "path": target, "partialPath": partial, "publicPath": published_path }))
}

fn validate_download_path_for_file_operation(raw_path: &str) -> Result<PathBuf, String> {
    let path = validate_custom_dir(raw_path.trim(), "下载文件路径")?;
    let name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
    let lower_name = name.to_ascii_lowercase();
    let is_partial = lower_name.ends_with(".part");
    let base_name = if is_partial { &lower_name[..lower_name.len() - 5] } else { &lower_name };
    if name.is_empty()
        || name.contains("..")
        || is_windows_reserved_file_name(base_name)
        || ![".apk", ".xapk", ".apks"].iter().any(|suffix| base_name.ends_with(suffix))
    {
        return Err("下载文件路径不合法".to_string());
    }
    Ok(path)
}

fn validate_custom_dir(value: &str, label: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{label}必须是当前平台的绝对路径：{value}"));
    }
    Ok(path)
}

fn user_save_dir(app: &tauri::AppHandle, custom_dir: Option<&str>) -> Result<PathBuf, String> {
    // Android 先在应用目录落盘，再通过 MediaStore/系统保存器导出；跨设备导入的桌面路径不适用。
    #[cfg(target_os = "android")]
    {
        let _ = custom_dir;
        return app.path().download_dir().map_err(|error| error.to_string());
    }
    #[cfg(not(target_os = "android"))]
    {
    if let Some(custom_dir) = custom_dir.map(str::trim).filter(|value| !value.is_empty()) {
        return validate_custom_dir(custom_dir, "自定义下载目录");
    }
    app.path()
        .download_dir()
        .map_err(|_| "无法获取系统下载目录，请在设置中选择下载目录".to_string())
    }
}

/// 优先采用服务器文件名，未知格式保留原扩展名；无格式信息时使用 bin。
fn build_download_file_name(url: &str, disposition: &str, mime_type: &str, header: &[u8]) -> String {
    // 引号内的分号属于文件名，不能当成参数分隔符。
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in disposition.char_indices() {
        if escaped { escaped = false; continue; }
        if quoted && character == '\\' { escaped = true; continue; }
        if character == '"' { quoted = !quoted; }
        else if character == ';' && !quoted { parts.push(&disposition[start..index]); start = index + 1; }
    }
    parts.push(&disposition[start..]);
    let parameters: Vec<(&str, &str)> = parts.into_iter().filter_map(|part| part.trim().split_once('=')).collect();
    let extended = parameters.iter().find(|(key, _)| key.eq_ignore_ascii_case("filename*")).and_then(|(_, value)| {
        let mut fields = value.trim_matches('"').splitn(3, '\'');
        if !fields.next()?.eq_ignore_ascii_case("utf-8") { return None; }
        fields.next()?;
        Some(decode_download_name(fields.next()?))
    });
    let declared = extended.or_else(|| parameters.iter().find(|(key, _)| key.eq_ignore_ascii_case("filename")).map(|(_, value)| value.trim().trim_matches('"').to_string()));
    let source_name = declared.filter(|name| !name.trim().is_empty()).unwrap_or_else(|| {
        reqwest::Url::parse(url).ok().and_then(|parsed| parsed.path_segments().and_then(|mut segments| segments.next_back()).map(decode_download_name)).unwrap_or_default()
    });
    // 丢弃目录部分和各平台非法字符，同时保留中文名称。
    let leaf = source_name.rsplit(['/', '\\']).next().unwrap_or_default();
    let safe_name: String = leaf.chars().filter(|character| !character.is_control() && !"<>:\"/\\|?*".contains(*character)).take(120).collect();
    let safe_name = safe_name.trim_matches([' ', '.']);
    let mut name = if safe_name.is_empty() { "coolapk_file".to_string() } else { safe_name.to_string() };
    let detected = detect_image_mime(header);
    let mime = detected.unwrap_or_else(|| mime_type.split(';').next().unwrap_or_default().trim());
    let extension = match mime.to_ascii_lowercase().as_str() {
        mime if mime.starts_with("image/") => image_extension(mime),
        "video/mp4" => "mp4", "video/webm" => "webm", "video/quicktime" => "mov", "audio/mpeg" => "mp3", "audio/mp4" => "m4a", "audio/flac" => "flac", "audio/ogg" => "ogg", "audio/wav" | "audio/x-wav" => "wav",
        "application/zip" | "application/x-zip-compressed" => "zip", "application/pdf" => "pdf", "application/x-7z-compressed" => "7z", "application/vnd.rar" | "application/x-rar-compressed" => "rar", "application/gzip" => "gz", "text/plain" => "txt", "text/html" => "html", "application/json" => "json", _ => "",
    };
    let has_extension = Path::new(&name).extension().is_some();
    let named_image = Path::new(&name).extension().and_then(|value| value.to_str()).is_some_and(|value| ["jpg", "jpeg", "png", "gif", "gifv", "webp", "avif", "bmp", "svg", "tif", "tiff", "ico", "heic", "heif"].contains(&value.to_ascii_lowercase().as_str()));
    if detected.is_some() || !has_extension || (named_image && mime.starts_with("image/") && extension != "bin") {
        let stem = Path::new(&name).file_stem().and_then(|value| value.to_str()).unwrap_or("coolapk_file");
        name = format!("{stem}.{}", if extension.is_empty() { "bin" } else { extension });
    }
    // Windows 设备名不能直接作为文件名，包括带扩展名的情况。
    let stem = name.split('.').next().unwrap_or_default().to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"].contains(&stem.as_str()) { name = format!("coolapk_{name}"); }
    name
}

/// 文件名解码保留加号，异常编码维持原名称，避免对中文字符串按错误字节位置切片。
fn decode_download_name(value: &str) -> String {
    decode_percent_bytes(value).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_else(|_| value.to_string())
}

async fn save_image_bytes(
    target_dir: &std::path::Path,
    file_name: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    use tokio::io::AsyncWriteExt;

    let sequence = IMAGE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = target_dir.join(format!(
        ".{file_name}.{}-{nonce}-{sequence}.part",
        std::process::id()
    ));
    let mut temp_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .await
        .map_err(|error| format!("创建图片临时文件失败：{error}"))?;
    if let Err(error) = async {
        temp_file.write_all(bytes).await?;
        temp_file.flush().await?;
        temp_file.sync_all().await
    }
    .await
    {
        drop(temp_file);
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(format!("写入图片临时文件失败：{error}"));
    }
    drop(temp_file);

    let result = {
        let _lock = IMAGE_SAVE_LOCK
            .lock()
            .map_err(|_| "图片保存锁已损坏".to_string())?;
        let target_path = next_available_file_path(target_dir, file_name);
        std::fs::rename(&temp_path, &target_path)
            .map(|_| target_path)
            .map_err(|error| format!("保存图片失败：{error}"))
    };
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temp_path).await;
    }
    result
}

fn decode_image_data_url(data_url: &str) -> Result<(&str, Vec<u8>), String> {
    let (mime_type, bytes) = decode_file_data_url(data_url)?;
    if !mime_type.starts_with("image/") { return Err("下载内容不是图片".to_string()); }
    Ok((mime_type, bytes))
}

/// 已加载的完整文件可直接保存，支持任意 MIME，不经过画布截图或格式转换。
fn decode_file_data_url(data_url: &str) -> Result<(&str, Vec<u8>), String> {
    let (header, payload) = data_url
        .split_once(',')
        .ok_or_else(|| "文件数据格式无效".to_string())?;
    let mime_type = header
        .strip_prefix("data:")
        .and_then(|value| value.split(';').next())
        .ok_or_else(|| "文件数据类型无效".to_string())?;
    let mime_type = if mime_type.is_empty() { "text/plain" } else { mime_type };
    let bytes = if header.ends_with(";base64") {
        BASE64.decode(payload).map_err(|error| format!("文件数据解码失败：{error}"))?
    } else {
        // 普通 Data URL 使用百分号编码，不能通过有损 UTF-8 转换处理二进制字节。
        decode_percent_bytes(payload)?
    };
    // CDN 可能把动图声明为 JPEG；以文件头为准，保留原始字节和正确扩展名。
    let mime_type = detect_image_mime(&bytes).unwrap_or(mime_type);
    Ok((mime_type, bytes))
}

fn decode_percent_bytes(value: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut remaining = value.as_bytes();
    while !remaining.is_empty() {
        if remaining[0] == b'%' {
            let hex = remaining.get(1..3).and_then(|digits| std::str::from_utf8(digits).ok()).and_then(|digits| u8::from_str_radix(digits, 16).ok()).ok_or_else(|| "文件数据包含无效百分号编码".to_string())?;
            bytes.push(hex);
            remaining = &remaining[3..];
        } else {
            bytes.push(remaining[0]);
            remaining = &remaining[1..];
        }
    }
    Ok(bytes)
}

/// 使用文件头识别图片格式，不对原始数据进行转码。
fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { Some("image/gif") }
    else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("image/png") }
    else if bytes.starts_with(b"\xff\xd8\xff") { Some("image/jpeg") }
    else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { Some("image/webp") }
    else if bytes.starts_with(b"BM") { Some("image/bmp") }
    else if bytes.starts_with(b"II\x2a\0") || bytes.starts_with(b"MM\0\x2a") { Some("image/tiff") }
    else if bytes.get(4..8) == Some(b"ftyp") && matches!(bytes.get(8..12), Some(b"avif" | b"avis")) { Some("image/avif") }
    else { None }
}

fn image_extension(mime_type: &str) -> &'static str {
    match mime_type.to_ascii_lowercase().as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "image/tiff" => "tiff",
        "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
        "image/heic" => "heic",
        "image/heif" => "heif",
        _ => "bin",
    }
}

fn build_image_file_name(url: &str, mime_type: &str) -> String {
    let source_name = reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let stem = std::path::Path::new(&source_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let safe_stem: String = stem
        .chars()
        .take(100)
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        .collect();
    let final_stem = if safe_stem.is_empty() || safe_stem.eq_ignore_ascii_case("showimage") {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("coolapk_image_{timestamp}")
    } else {
        safe_stem
    };
    format!("{final_stem}.{}", image_extension(mime_type))
}

fn build_generated_image_file_name(file_name: &str, mime_type: &str) -> String {
    let requested_stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let safe_stem: String = requested_stem
        .chars()
        .take(100)
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        .collect();
    let final_stem = if safe_stem.is_empty() {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!("coolapk_share_{timestamp}")
    } else {
        safe_stem
    };
    format!("{final_stem}.{}", image_extension(mime_type))
}

fn next_available_file_path(dir: &std::path::Path, file_name: &str) -> PathBuf {
    let initial = dir.join(file_name);
    if !initial.exists() {
        return initial;
    }

    let path = std::path::Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("coolapk_image");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("jpg");
    for index in 2..=9999 {
        let candidate = dir.join(format!("{stem}_{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}_{}.{}", std::process::id(), extension))
}

fn equipment_window_error_script(message: &str) -> String {
    let message = serde_json::to_string(message).unwrap();
    format!("document.title = '装备页面打开失败'; const body = document.body || document.documentElement.appendChild(document.createElement('body')); body.style.cssText = 'font:16px system-ui;padding:48px;color:#333;background:#fff'; body.textContent = '装备页面打开失败：' + {message};")
}

fn equipment_webview_cookies(header: &str) -> Vec<tauri::webview::Cookie<'static>> {
    header.split(';').filter_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        let name = name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("ddid")
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)) {
            return None;
        }
        // 凭据只作用于装备网页域，不交给前端脚本或任意外部网址。
        Some(tauri::webview::Cookie::build((name.to_string(), value.trim().to_string()))
            .domain("m.coolapk.com").path("/").secure(true).http_only(true).build())
    }).collect()
}

async fn sync_equipment_webview_cookie(window: &tauri::WebviewWindow, header: Option<&str>) -> Result<(), String> {
    let Some(header) = header.filter(|value| !value.trim().is_empty()) else { return Ok(()); };
    let cookies = equipment_webview_cookies(header);
    #[cfg(windows)]
    super::equipment_cookie_windows::write_equipment_cookies(window, cookies.clone()).await?;
    #[cfg(not(target_os = "android"))]
    {
        // Cookie API 是阻塞调用，放到独立线程才能让外层超时和窗口 UI 正常响应。
        let window = window.clone();
        return tokio::task::spawn_blocking(move || {
        // 覆盖 WebView 中同名的旧账号 Cookie，包括根域 Cookie，避免重复 Cookie 让服务端误认账号。
        #[cfg(not(windows))]
        {
        let target = reqwest::Url::parse("https://m.coolapk.com/mp/do").unwrap();
        let existing = window.cookies_for_url(target).map_err(|_| "读取装备窗口登录态失败".to_string())?;
        for cookie in &cookies {
            for old in existing.iter().filter(|old| old.name() == cookie.name()) {
                let mut updated = old.clone();
                updated.set_value(cookie.value().to_string());
                updated.set_expires(None);
                updated.set_max_age(None);
                window.set_cookie(updated).map_err(|_| "同步装备窗口登录态失败".to_string())?;
            }
            window.set_cookie(cookie.clone()).map_err(|_| "同步装备窗口登录态失败".to_string())?;
        }
        }
        // 只核对登录身份；浏览器会自行更新追踪 Cookie，不能要求整份 Cookie 完全一致。
        // 原生存储写入可能异步落地，短暂等待后再读取，避免首次回读误判失败。
        for attempt in 0..10 {
            let stored = window.cookies_for_url(reqwest::Url::parse("https://m.coolapk.com/mp/do").unwrap())
                .map_err(|_| "核对装备窗口登录态失败".to_string())?;
            if equipment_cookie_store_matches(&cookies, &stored) { return Ok(()); }
            if attempt < 9 { std::thread::sleep(Duration::from_millis(50)); }
        }
        log::warn!("equipment.cookie_identity_mismatch");
        Err("装备窗口登录态未同步成功，请重新打开装备页面".to_string())
        }).await.map_err(|_| "装备窗口登录态同步任务失败".to_string())?;
    }
    #[cfg(target_os = "android")]
    {
        // Android 的通用 set_cookie 是空实现，必须使用系统 CookieManager。
        let (sender, receiver) = tokio::sync::oneshot::channel();
        window.with_webview(move |webview| {
            webview.jni_handle().exec(move |env, _, _| {
                let result: jni::errors::Result<()> = (|| {
                    let manager = env.call_static_method("android/webkit/CookieManager", "getInstance", "()Landroid/webkit/CookieManager;", &[])?.l()?;
                    let address = env.new_string("https://m.coolapk.com/")?;
                    for cookie in cookies {
                        // 同步当前账号的根域与页面域 Cookie，替换共享存储里的旧身份。
                        for domain in [".coolapk.com", "m.coolapk.com"] {
                            let mut scoped = cookie.clone();
                            scoped.set_domain(domain);
                            let value = env.new_string(scoped.to_string())?;
                            env.call_method(&manager, "setCookie", "(Ljava/lang/String;Ljava/lang/String;)V", &[(&address).into(), (&value).into()])?;
                        }
                    }
                    env.call_method(&manager, "flush", "()V", &[])?;
                    Ok(())
                })();
                if result.is_err() && env.exception_check().unwrap_or(false) { let _ = env.exception_clear(); }
                let _ = sender.send(result.map_err(|_| "Android 装备窗口登录态同步失败".to_string()));
            });
        }).map_err(|_| "无法访问 Android 装备窗口".to_string())?;
        tokio::time::timeout(Duration::from_secs(5), receiver).await
            .map_err(|_| "同步 Android 装备窗口登录态超时".to_string())?
            .map_err(|_| "Android 装备窗口已关闭".to_string())?
    }
}

#[cfg(any(not(target_os = "android"), test))]
fn equipment_cookie_store_matches(expected: &[tauri::webview::Cookie<'_>], stored: &[tauri::webview::Cookie<'_>]) -> bool {
    expected.iter().filter(|cookie| matches!(cookie.name(), "SESSID" | "uid" | "token")).all(|cookie| {
        let same_name: Vec<_> = stored.iter().filter(|old| old.name() == cookie.name()).collect();
        !same_name.is_empty() && same_name.iter().all(|old| old.value() == cookie.value())
    })
}

fn is_equipment_action_url(url: &reqwest::Url, method: &str) -> bool {
    url.scheme() == "https" && url.host_str() == Some("m.coolapk.com")
        && url.username().is_empty() && url.password().is_none()
        && url.path() == "/mp/do"
        && url.query_pairs().any(|(key, value)| key == "c" && value == "product")
        && url.query_pairs().any(|(key, value)| key == "m" && value == method)
}

fn equipment_product_callback(product: &Value) -> Result<String, String> {
    let number = |key: &str| product.get(key).and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
    let second = number("second_category_id").unwrap_or(0);
    // 与官方产品选择器一致：指定二级分类直接回填，其余使用一级分类。
    let category = if [1013, 1000, 1001, 1002, 1003, 1005].contains(&second) {
        second
    } else {
        number("category_id").unwrap_or(0)
    };
    let id = number("id").unwrap_or(0);
    let title = product.get("title").and_then(Value::as_str).unwrap_or("");
    let logo = product.get("logo").and_then(Value::as_str).unwrap_or("");
    if category <= 0 || id <= 0 || title.is_empty() {
        return Err("产品缺少装备分类或产品信息".to_string());
    }
    // JSON 编码每个参数，名称中的引号、换行等不能变成可执行脚本。
    let args = [category.to_string(), id.to_string(), title.to_string(), logo.to_string()]
        .iter().map(|value| serde_json::to_string(value).unwrap()).collect::<Vec<_>>().join(",");
    Ok(format!("if (typeof window.addProductOwner === 'function') {{ window.addProductOwner({args}); }} else {{ window.alert('装备编辑页面尚未就绪，请稍后重新添加'); }}"))
}

#[cfg(test)]
mod equipment_bridge_tests {
    use super::*;

    #[test]
    fn equipment_window_errors_are_rendered_as_text() {
        let message = "错误\"内容\\\n<script>alert(1)</script>";
        let script = equipment_window_error_script(message);
        assert!(script.contains("body.textContent"));
        assert!(script.contains(&serde_json::to_string(message).unwrap()));
        assert!(!script.contains("body.innerHTML"));
    }

    #[test]
    fn equipment_cookie_verification_rejects_stale_duplicate_identity() {
        let expected = equipment_webview_cookies("SESSID=current; uid=123");
        assert!(equipment_cookie_store_matches(&expected, &expected));
        assert!(!equipment_cookie_store_matches(&expected, &[]));
        let mut stale = expected.clone();
        stale.push(tauri::webview::Cookie::new("uid", "other-account"));
        assert!(!equipment_cookie_store_matches(&expected, &stale));
    }

    #[test]
    fn equipment_cookie_verification_ignores_browser_managed_cookie_differences() {
        let expected = equipment_webview_cookies("SESSID=current; uid=123; token=auth; ntes_utid=old-tracker; forward=old-page; displayVersion=old-version; username=encoded-name");
        let mut stored = equipment_webview_cookies("SESSID=current; uid=123; token=auth; ntes_utid=new-tracker; username=decoded-name");
        assert!(equipment_cookie_store_matches(&expected, &stored));
        stored.iter_mut().find(|cookie| cookie.name() == "SESSID").unwrap().set_value("old-session");
        assert!(!equipment_cookie_store_matches(&expected, &stored));
    }

    #[test]
    fn equipment_login_cookie_is_scoped_and_not_embedded_in_javascript() {
        let cookies = equipment_webview_cookies(" SESSID=current-session; uid=123; token=a=b; ddid=api-only; malformed; bad name=value");
        assert_eq!(cookies.len(), 3);
        assert_eq!(cookies[0].name(), "SESSID");
        assert_eq!(cookies[0].value(), "current-session");
        assert_eq!(cookies[2].value(), "a=b");
        for cookie in &cookies {
            assert_eq!(cookie.domain(), Some("m.coolapk.com"));
            assert_eq!(cookie.path(), Some("/"));
            assert_eq!(cookie.secure(), Some(true));
            assert_eq!(cookie.http_only(), Some(true));
            assert_eq!(cookie.expires(), None);
        }
        assert!(equipment_webview_cookies("").is_empty());
    }

    #[test]
    fn share_action_preserves_other_users_uid() {
        let url = reqwest::Url::parse("https://m.coolapk.com/mp/do?c=product&m=productOwnerShare&uid=456&from=home").unwrap();
        assert!(is_equipment_action_url(&url, "productOwnerShare"));
        assert!(url.query_pairs().any(|(key, value)| key == "uid" && value == "456"));
    }

    #[test]
    fn intercept_only_official_add_product_action() {
        for (url, expected) in [
            ("https://m.coolapk.com/mp/do?c=product&m=addProductOwner", true),
            ("https://m.coolapk.com/mp/do?c=product&m=editProductOwner", false),
            ("https://m.coolapk.com.evil.com/mp/do?c=product&m=addProductOwner", false),
            ("https://m.coolapk.com/other?c=product&m=addProductOwner", false),
            ("https://m.coolapk.com/mp/do?c=user&m=addProductOwner", false),
        ] {
            assert_eq!(is_equipment_action_url(&reqwest::Url::parse(url).unwrap(), "addProductOwner"), expected);
        }
    }

    #[test]
    fn official_category_mapping_and_safe_callback_arguments() {
        let product = json!({"id": "123", "second_category_id": 1002, "category_id": 9, "title": "产品\"名称\\换行\n", "logo": "https://image.coolapk.com/123.png"});
        let script = equipment_product_callback(&product).unwrap();
        assert!(script.contains("window.addProductOwner(\"1002\",\"123\","));
        assert!(script.contains(&serde_json::to_string(product["title"].as_str().unwrap()).unwrap()));
        let fallback = equipment_product_callback(&json!({"id": 123, "second_category_id": 999, "category_id": "9", "title": "设备", "logo": ""})).unwrap();
        assert!(fallback.contains("window.addProductOwner(\"9\",\"123\","));
        assert!(equipment_product_callback(&json!({"id": 123, "title": "设备"})).is_err());
    }
}

/// 从主窗口当前 URL 推导应用自身源地址（dev 为 http://127.0.0.1:17520，打包后为 tauri 自定义协议源），
/// 用于登录回跳 forward 与关窗判定，避免 dev/生产环境不一致。
///
/// 安全约束：只允许应用自身的固定源。登录回跳会把 Cookie 拼进 URL 带回本地，
/// 若主窗口被导航到外部域名，绝不能把凭据回跳到该域。
const ALLOWED_APP_ORIGINS: &[&str] = &[
    "http://127.0.0.1:17520",
    "http://tauri.localhost",
    "tauri://localhost",
];

fn get_app_origin(app: &tauri::AppHandle) -> String {
    use tauri::Manager;
    if let Some(main) = app.get_webview_window("main") {
        if let Ok(url) = main.url() {
            if let Some(host) = url.host_str() {
                let origin = match url.port() {
                    Some(port) => format!("{}://{}:{}", url.scheme(), host, port),
                    None => format!("{}://{}", url.scheme(), host),
                };
                if ALLOWED_APP_ORIGINS.contains(&origin.as_str()) {
                    return origin;
                }
            }
        }
    }
    "http://127.0.0.1:17520".to_string()
}

fn login_callback_kind(url: &reqwest::Url, app_origin: &str) -> Option<&'static str> {
    if url.scheme() == "https"
        && url.host_str() == Some("account.coolapk.com")
        && url.path() == "/auth/callback"
    {
        return Some("official");
    }
    if url.as_str().starts_with(&format!("{app_origin}/"))
        && url
            .fragment()
            .is_some_and(|fragment| fragment.split('?').next() == Some("/auth_callback"))
    {
        return Some("app-origin");
    }
    None
}

/// 从回跳 URL 中提取 ck 参数（完整 cookie 字符串），例如
/// `http://127.0.0.1:17520/#/auth_callback?ck=uid%3D...%3BSESSID%3D...`
fn extract_callback_param(url: &str, key: &str) -> Option<String> {
    let queries = [
        url.split_once('?')
            .map(|(_, value)| value.split('#').next().unwrap_or(value)),
        url.split_once('#')
            .and_then(|(_, value)| value.split_once('?').map(|(_, query)| query)),
    ];
    for query in queries.into_iter().flatten() {
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let name = percent_decode(parts.next().unwrap_or_default());
            if name == key {
                return Some(percent_decode(parts.next().unwrap_or_default()));
            }
        }
    }
    None
}

/// 从回跳 URL 中提取完整 Cookie 字符串。
fn extract_ck_from_url(url: &str) -> Option<String> {
    extract_callback_param(url, "ck")
}

/// 合并回调参数和 WebView2 Cookie 存储中的 Cookie，后者覆盖同名旧值。
fn merge_cookie_headers(first: Option<&str>, second: Option<&str>) -> Option<String> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for source in [first.unwrap_or_default(), second.unwrap_or_default()] {
        for item in source.split(';') {
            let mut parts = item.trim().splitn(2, '=');
            let name = parts.next().unwrap_or_default().trim();
            let value = parts.next().unwrap_or_default().trim();
            if name.is_empty() {
                continue;
            }
            if let Some(existing) = pairs.iter_mut().find(|(key, _)| key == name) {
                existing.1 = value.to_string();
            } else {
                pairs.push((name.to_string(), value.to_string()));
            }
        }
    }
    if pairs.is_empty() {
        None
    } else {
        Some(pairs.into_iter().map(|(name, value)| format!("{name}={value}")).collect::<Vec<_>>().join("; "))
    }
}

/// 从登录 WebView 的 Cookie 存储读取酷安会话，包含 HttpOnly Cookie。
async fn get_login_webview_cookie<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>) -> Result<String, String> {
    login_checkpoint(LoginStage::CookieReadStarted);
    // 在 WebView 线程直接访问系统 CookieManager，不依赖 Activity 或生成类中的自定义方法。
    #[cfg(target_os = "android")]
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        win.with_webview(move |webview| {
            webview.jni_handle().exec(move |env, _, _| {
                let result: jni::errors::Result<String> = (|| {
                    let manager = env.call_static_method(
                        "android/webkit/CookieManager",
                        "getInstance",
                        "()Landroid/webkit/CookieManager;",
                        &[],
                    )?.l()?;
                    let mut combined: Option<String> = None;
                    for address in [
                        "https://account.coolapk.com/",
                        "https://www.coolapk.com/",
                        "https://m.coolapk.com/",
                        "https://api.coolapk.com/",
                    ] {
                        let address = env.new_string(address)?;
                        let value = env.call_method(
                            &manager,
                            "getCookie",
                            "(Ljava/lang/String;)Ljava/lang/String;",
                            &[(&address).into()],
                        )?.l()?;
                        if value.is_null() {
                            continue;
                        }
                        let value = jni::objects::JString::from(value);
                        let header: String = env.get_string(&value)?.into();
                        // account 域优先；其他子域只补充它没有的 Cookie。
                        combined = merge_cookie_headers(Some(&header), combined.as_deref());
                    }
                    Ok(combined.unwrap_or_default())
                })();
                if result.is_err() && env.exception_check().unwrap_or(false) {
                    let _ = env.exception_clear();
                }
                // JNI 错误只使用固定类别，不把可能包含敏感参数的异常写入日志。
                let _ = sender.send(result.map_err(|_| "Android CookieManager 读取失败".to_string()));
            });
        }).map_err(|_| "无法访问 Android 登录 WebView".to_string())?;
        return tokio::time::timeout(Duration::from_secs(5), receiver)
            .await
            .map_err(|_| "读取 Android 登录 Cookie 超时".to_string())?
            .map_err(|_| "Android 登录窗口已关闭".to_string())?;
    }

    #[cfg(target_os = "ios")]
    {
        use objc2::{msg_send, runtime::AnyObject};
        use objc2_foundation::{NSArray, NSHTTPCookie};
        let (sender, receiver) = tokio::sync::oneshot::channel();
        win.with_webview(move |webview| unsafe {
            // WKHTTPCookieStore 是异步接口。让主线程返回后再接收回调，避免同步等待超时。
            let view = &*(webview.inner() as *const AnyObject);
            let configuration: *mut AnyObject = msg_send![view, configuration];
            let store: *mut AnyObject = msg_send![configuration, websiteDataStore];
            let cookies: *mut AnyObject = msg_send![store, httpCookieStore];
            let sender = Mutex::new(Some(sender));
            let completion = block2::RcBlock::new(move |values: std::ptr::NonNull<NSArray<NSHTTPCookie>>| {
                let header = merge_ios_login_cookies(values.as_ref().iter()
                    .map(|cookie| (cookie.domain().to_string(), cookie.name().to_string(), cookie.value().to_string()))
                    .collect());
                if let Some(sender) = sender.lock().ok().and_then(|mut value| value.take()) {
                    login_checkpoint(LoginStage::CookieReadCompleted);
                    let _ = sender.send(header);
                }
            });
            let _: () = msg_send![cookies, getAllCookies: &*completion];
        }).map_err(|_| "无法访问 iOS 登录 WebView".to_string())?;
        return tokio::time::timeout(Duration::from_secs(5), receiver)
            .await
            .map_err(|_| "读取 iOS 登录 Cookie 超时".to_string())
            .and_then(|result| result.map_err(|_| "iOS 登录窗口已关闭".to_string()));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let cookies = win.cookies().map_err(|e| e.to_string())?;
        Ok(cookies
            .into_iter()
            .filter(|cookie| {
                is_coolapk_cookie_domain(cookie.domain().unwrap_or_default())
            })
            .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
            .collect::<Vec<_>>()
            .join("; "))
    }
}

#[cfg(any(not(target_os = "android"), test))]
fn is_coolapk_cookie_domain(domain: &str) -> bool {
    let domain = domain.trim_start_matches('.').to_ascii_lowercase();
    domain == "coolapk.com" || domain.ends_with(".coolapk.com")
}

/// 诊断日志只记录预定义类别，避免把服务器响应、Cookie 或授权码写入日志。
fn login_failure_kind(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("captcha") || error.contains("验证码") {
        "captcha"
    } else if lower.contains("http 401") {
        "http_401"
    } else if lower.contains("http 403") {
        "http_403"
    } else if lower.contains("http 429") || lower.contains("rate_limit") {
        "rate_limit"
    } else if lower.contains("http 5") {
        "http_5xx"
    } else if lower.contains("http 4") {
        "http_4xx"
    } else if lower.contains("timed out") || lower.contains("timeout") || error.contains("超时") {
        "timeout"
    } else if lower.contains("error sending request")
        || lower.contains("failed to send")
        || lower.contains("connection")
        || lower.contains("connect error")
    {
        "transport"
    } else if lower.contains("sessid") || error.contains("会话") {
        "invalid_session"
    } else if error.contains("没有登录凭据") || error.contains("没有带回登录 Cookie") {
        "missing_cookie"
    } else if error.contains("未返回完整登录信息") || error.contains("未通过账号校验") {
        "invalid_account_response"
    } else if lower.contains("invalid coolapk json")
        || lower.contains("failed to read coolapk response")
    {
        "invalid_response"
    } else if error.contains("登录信息有误") || error.contains("未登录") {
        "authentication"
    } else {
        "other"
    }
}

/// APK 只在 ac=access_token 时把 code 交给 /account/accessToken。
fn extract_access_code_from_url(url: &str) -> Option<String> {
    if extract_callback_param(url, "ac").as_deref() != Some("access_token") {
        return None;
    }
    extract_callback_param(url, "code").filter(|code| !code.trim().is_empty())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 自动监控和手动同步共享兑换状态，避免并发消费一次性授权码。
async fn verify_login_webview(
    app: &tauri::AppHandle,
    win: &tauri::WebviewWindow,
    session: &LoginSession,
    automatic: bool,
) -> Result<bool, String> {
    #[cfg(not(target_os = "ios"))]
    let _ = automatic;
    let mut verification = session.verification.lock().await;
    if verification.completed {
        return Ok(true);
    }
    let callback = session.callback_url.lock().map_err(|_| "登录状态不可用".to_string())?.clone();
    let callback_cookie = callback.as_deref().and_then(extract_ck_from_url);
    let webview_cookie = match get_login_webview_cookie(win).await {
        Ok(cookie) => {
            log::info!("login.webview_cookie_read has_cookie={} has_session={}",
                !cookie.is_empty(), CoolapkClient::has_valid_session_cookie(&cookie));
            Some(cookie)
        }
        Err(error) => {
            log::warn!("login.webview_cookie_read_failed platform={} reason={}",
                std::env::consts::OS,
                if error.contains("超时") { "timeout" } else { "native_read" });
            if callback_cookie.is_none() {
                return Err("无法读取登录窗口凭据，请重试同步或重新打开登录窗口".to_string());
            }
            None
        }
    };
    let cookie = merge_cookie_headers(callback_cookie.as_deref(), webview_cookie.as_deref());
    let has_session = cookie.as_deref().is_some_and(CoolapkClient::has_valid_session_cookie);
    if !has_session {
        log::info!("login.cookie_not_ready");
        return Ok(false);
    }
    #[cfg(target_os = "ios")]
    if automatic && !verification.should_verify_ios_cookie(cookie.as_deref().unwrap_or_default(), callback.as_deref()) {
        return Ok(false);
    }
    let state = app.state::<AppState>();
    let code = callback.as_deref().and_then(extract_access_code_from_url);
    let mut valid = false;
    if let (Some(callback), Some(code)) = (callback.as_deref(), code) {
        if verification.should_exchange(callback, has_session) {
            // 只有真正发起兑换才标记使用；Cookie 读取失败不会消耗兑换机会。
            verification.attempted_access_callback = Some(callback.to_string());
            match state.client.login_by_access_code(&code, cookie.as_deref()).await {
                Ok(_) => {
                    log::info!("login.access_code_succeeded");
                    valid = true;
                }
                Err(error) => log::warn!("login.access_code_failed reason={}", login_failure_kind(&error)),
            }
        }
    }
    if !valid {
        match state.client.login_by_webview_cookie(cookie.as_deref().unwrap_or_default()).await {
            Ok(_) => {
                log::info!("login.webview_cookie_succeeded");
                valid = true;
            }
            Err(error) => log::warn!("login.webview_cookie_failed reason={}", login_failure_kind(&error)),
        }
    }
    verification.completed = valid;
    drop(verification);
    if valid && close_login_window(app.clone()).is_err() {
        log::warn!("login.window_close_failed");
    }
    Ok(valid)
}

#[cfg(target_os = "ios")]
fn apply_login_safe_area(win: &tauri::WebviewWindow) -> Result<(), String> {
    use objc2::{msg_send, rc::Retained, runtime::AnyObject};
    win.with_webview(|webview| unsafe {
        login_checkpoint(LoginStage::SafeAreaStarted);
        // Wry 使用 frame 和自动缩放管理 WebView，保持它的布局方式，避免额外约束影响窗口创建。
        let view = &*(webview.inner() as *const AnyObject);
        let scroll_view: Retained<AnyObject> = msg_send![view, scrollView];
        // UIScrollViewContentInsetAdjustmentAlways = 3，由 UIKit 随安全区变化调整正文边距。
        let _: () = msg_send![&scroll_view, setContentInsetAdjustmentBehavior: 3isize];
        log::info!("login.safe_area_applied platform=ios mode=scroll_insets");
        login_checkpoint(LoginStage::SafeAreaCompleted);
    }).map_err(|_| "设置 iOS 登录安全区失败".to_string())
}

#[cfg(test)]
mod login_callback_tests {
    use super::{
        extract_access_code_from_url, extract_callback_param, extract_ck_from_url,
        login_callback_kind, login_failure_kind, LOGIN_WEBVIEW_USER_AGENT,
    };

    #[test]
    fn ios_waits_for_sms_login_on_account_page_but_not_logout_or_untrusted_pages() {
        for url in ["https://account.coolapk.com/auth/login", "https://account.coolapk.com/auth/login/?type=coolapk", "https://account.coolapk.com/auth/callback", "https://www.coolapk.com/"] {
            assert!(super::should_poll_ios_login_cookies(&reqwest::Url::parse(url).unwrap()));
        }
        for url in ["https://account.coolapk.com/auth/logout", "https://account.coolapk.com/other", "http://account.coolapk.com/auth/login", "https://account.coolapk.com.evil.test/auth/login"] {
            assert!(!super::should_poll_ios_login_cookies(&reqwest::Url::parse(url).unwrap()));
        }
    }

    #[test]
    fn ios_limits_same_cookie_verification_but_retries_new_sessions_and_callbacks() {
        let mut verification = super::LoginVerification::default();
        for _ in 0..4 { assert!(verification.should_verify_ios_cookie("SESSID=one; uid=12", None)); }
        assert!(!verification.should_verify_ios_cookie("uid=12; SESSID=one", None));
        assert!(verification.should_verify_ios_cookie("SESSID=two; uid=12", None));
        for _ in 0..3 { assert!(verification.should_verify_ios_cookie("SESSID=two; uid=12", None)); }
        assert!(!verification.should_verify_ios_cookie("SESSID=two; uid=12", None));
        assert!(verification.should_verify_ios_cookie("SESSID=two; uid=12", Some("new-callback")));
    }

    #[test]
    fn ios_resumes_verification_after_cooldown_even_if_login_keeps_the_same_cookie() {
        let mut verification = super::LoginVerification::default();
        for _ in 0..4 { assert!(verification.should_verify_ios_cookie("SESSID=unchanged", None)); }
        assert!(!verification.should_verify_ios_cookie("SESSID=unchanged", None));
        // 短信登录完成后 Cookie 可能不变，冷却结束必须重新校验，不能永久停掉自动同步。
        verification.ios_cookie_next_retry = Some(std::time::Instant::now());
        assert!(verification.should_verify_ios_cookie("SESSID=unchanged", None));
        assert_eq!(verification.ios_cookie_attempts, 1);
    }

    #[test]
    fn ios_cookie_merge_prefers_account_session_and_ignores_expired_or_unrelated_values() {
        let cookies = vec![
            ("account.coolapk.com", "SESSID", "account"),
            ("www.coolapk.com", "SESSID", "web-old"),
            (".coolapk.com", "SESSID", "root-old"),
            (".coolapk.com", "uid", "12"),
            ("account.coolapk.com", "SESSID", "deleted"),
            ("evil.test", "token", "unrelated"),
        ].into_iter().map(|(domain, name, value)| (domain.to_string(), name.to_string(), value.to_string())).collect();
        let header = super::merge_ios_login_cookies(cookies);
        assert_eq!(header.matches("SESSID=").count(), 1);
        assert!(header.contains("SESSID=account"));
        assert!(header.contains("uid=12"));
        assert!(!header.contains("unrelated"));
    }

    #[test]
    fn ios_return_navigation_only_accepts_the_fixed_marker() {
        assert!(super::is_ios_login_return_url(&reqwest::Url::parse("coolapk-login://return").unwrap()));
        for url in ["https://return/", "coolapk-login://return/other", "coolapk-login://return?cookie=secret", "coolapk-login://other", "coolapk-login://user@return"] {
            assert!(!super::is_ios_login_return_url(&reqwest::Url::parse(url).unwrap()));
        }
    }

    #[test]
    fn missing_cookie_does_not_consume_access_code_exchange() {
        let mut state = super::LoginVerification::default();
        let callback = "https://account.coolapk.com/auth/callback?ac=access_token&code=first";
        assert!(!state.should_exchange(callback, false));
        // 原生 Cookie 延迟到达后，同一授权回调仍然可以兑换。
        assert!(state.should_exchange(callback, true));
        state.attempted_access_callback = Some(callback.to_string());
        assert!(!state.should_exchange(callback, true));
        assert!(state.should_exchange("https://account.coolapk.com/auth/callback?ac=access_token&code=second", true));
    }

    #[test]
    fn cookie_domains_exclude_unrelated_and_lookalike_sites() {
        assert!(super::is_coolapk_cookie_domain(".coolapk.com"));
        assert!(super::is_coolapk_cookie_domain("account.coolapk.com"));
        assert!(!super::is_coolapk_cookie_domain("evilcoolapk.com"));
        assert!(!super::is_coolapk_cookie_domain("coolapk.com.evil.test"));
    }

    #[test]
    fn account_cookie_has_priority_over_other_subdomains() {
        assert_eq!(super::merge_cookie_headers(
            Some("SESSID=other; uid=12"), Some("SESSID=account")
        ).as_deref(), Some("SESSID=account; uid=12"));
    }

    #[test]
    fn login_webview_ua_keeps_desktop_mouse_events() {
        assert!(LOGIN_WEBVIEW_USER_AGENT.contains("Windows NT"));
        assert!(!LOGIN_WEBVIEW_USER_AGENT.contains("Mobile"));
    }

    #[test]
    fn extracts_access_code_and_cookie_from_hash_callback() {
        let url = "http://127.0.0.1:17520/#/auth_callback?ac=access_token&code=one%2Btime&ck=SESSID%3Dsession%3B%20uid%3D0";
        assert_eq!(extract_callback_param(url, "ac").as_deref(), Some("access_token"));
        assert_eq!(extract_access_code_from_url(url).as_deref(), Some("one+time"));
        assert_eq!(
            extract_ck_from_url(url).as_deref(),
            Some("SESSID=session; uid=0")
        );
    }

    #[test]
    fn extracts_access_code_from_account_query_callback() {
        let url = "https://account.coolapk.com/auth/callback?ac=access_token&code=server-code";
        assert_eq!(extract_access_code_from_url(url).as_deref(), Some("server-code"));
    }

    #[test]
    fn rejects_non_access_token_callback() {
        let url = "https://account.coolapk.com/auth/callback?ac=login&code=server-code";
        assert_eq!(extract_access_code_from_url(url), None);
    }

    #[test]
    fn classifies_transient_login_callbacks() {
        let official = reqwest::Url::parse(
            "https://account.coolapk.com/auth/callback?ac=access_token&code=server-code",
        )
        .unwrap();
        assert_eq!(
            login_callback_kind(&official, "http://127.0.0.1:17520"),
            Some("official")
        );

        let app = reqwest::Url::parse(
            "http://127.0.0.1:17520/#/auth_callback?ac=access_token&code=server-code",
        )
        .unwrap();
        assert_eq!(
            login_callback_kind(&app, "http://127.0.0.1:17520"),
            Some("app-origin")
        );

        let landing = reqwest::Url::parse("https://www.coolapk.com/").unwrap();
        assert_eq!(
            login_callback_kind(&landing, "http://127.0.0.1:17520"),
            None
        );
    }

    #[test]
    fn login_failure_logs_only_safe_categories() {
        assert_eq!(login_failure_kind("Coolapk API returned HTTP 403: secret response"), "http_403");
        assert_eq!(login_failure_kind("error sending request for url containing a token"), "transport");
        assert_eq!(login_failure_kind("授权回调没有带回有效的 SESSID"), "invalid_session");
        assert_eq!(login_failure_kind("unknown response with cookie=secret"), "other");
    }
}

/// 后台静默下载更新安装包，实时向前端广播下载进度；
/// 支持限速（speed_limit_kbps，0 为不限速），网络请求使用全局代理设置。
///
/// 安全约束：仅允许 https + GitHub 官方域名白名单（含 release 资源重定向目标），
/// 文件名净化 + 体积上限，防止前端被注入时被利用下载并执行任意文件。
const UPDATE_MAX_BYTES: u64 = 500 * 1024 * 1024;

fn dir_total_size(dir: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    total += dir_total_size(&entry.path());
                } else {
                    total += meta.len();
                }
            }
        }
    }
    total
}

const IMAGE_CACHE_CONTAINER: &str = "CoolapkDesktopCache";
const IMAGE_CACHE_MAGIC: &str = "COOLAPK_IMAGE_CACHE_V1";

fn image_cache_root(app: &tauri::AppHandle, custom_dir: Option<&str>) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    let custom_dir = { let _ = custom_dir; None::<&str> };
    let base = custom_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| validate_custom_dir(value, "自定义缓存目录"))
        .unwrap_or_else(|| app.path().app_cache_dir().map_err(|e| e.to_string()))?;
    Ok(base.join(IMAGE_CACHE_CONTAINER).join("images"))
}

fn image_cache_file(
    app: &tauri::AppHandle,
    custom_dir: Option<&str>,
    url: &str,
) -> Result<PathBuf, String> {
    let mut hasher = Md5::new();
    hasher.update(url.as_bytes());
    let key = hex::encode(hasher.finalize());
    Ok(image_cache_root(app, custom_dir)?.join(format!("{key}.bin")))
}

async fn read_image_cache(path: &std::path::Path, ttl_days: u64) -> Option<String> {
    let metadata = tokio::fs::metadata(path).await.ok()?;
    if ttl_days > 0 {
        let max_age = Duration::from_secs(ttl_days.saturating_mul(24 * 60 * 60));
        let modified = metadata.modified().ok()?;
        if SystemTime::now().duration_since(modified).ok()? > max_age {
            let _ = tokio::fs::remove_file(path).await;
            return None;
        }
    }

    let bytes = tokio::fs::read(path).await.ok()?;
    let first_break = bytes.iter().position(|byte| *byte == b'\n')?;
    let second_break = bytes[first_break + 1..]
        .iter()
        .position(|byte| *byte == b'\n')?
        + first_break
        + 1;
    let magic = std::str::from_utf8(&bytes[..first_break]).ok()?;
    let mime = std::str::from_utf8(&bytes[first_break + 1..second_break]).ok()?;
    if magic != IMAGE_CACHE_MAGIC || !mime.starts_with("image/") {
        let _ = tokio::fs::remove_file(path).await;
        return None;
    }
    Some(format!(
        "data:{mime};base64,{}",
        BASE64.encode(&bytes[second_break + 1..])
    ))
}

async fn write_image_cache(path: &std::path::Path, data_url: &str) -> Result<(), String> {
    let (meta, encoded) = data_url
        .split_once(',')
        .ok_or_else(|| "图片数据格式不正确".to_string())?;
    let mime = meta
        .strip_prefix("data:")
        .and_then(|value| value.strip_suffix(";base64"))
        .filter(|value| value.starts_with("image/"))
        .ok_or_else(|| "图片类型不正确".to_string())?;
    let image = BASE64
        .decode(encoded)
        .map_err(|e| format!("图片缓存解码失败：{e}"))?;
    let parent = path.parent().ok_or_else(|| "缓存目录不正确".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| format!("创建缓存目录失败：{e}"))?;

    let mut content = format!("{IMAGE_CACHE_MAGIC}\n{mime}\n").into_bytes();
    content.extend_from_slice(&image);
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    tokio::fs::write(&temp, content)
        .await
        .map_err(|e| format!("写入图片缓存失败：{e}"))?;
    if tokio::fs::rename(&temp, path).await.is_err() {
        let _ = tokio::fs::remove_file(path).await;
        tokio::fs::rename(&temp, path)
            .await
            .map_err(|e| format!("保存图片缓存失败：{e}"))?;
    }
    Ok(())
}

fn cache_locations(
    app: &tauri::AppHandle,
    custom_dir: Option<&str>,
) -> Result<(PathBuf, PathBuf), String> {
    let image = image_cache_root(app, custom_dir)?;
    let update = update_cache_dir(app)?;
    Ok((image, update))
}

pub(super) fn update_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        // Tauri 的 Android app_data_dir 是应用数据根目录；files 子目录可由 FileProvider 安全共享。
        Ok(app.path().app_data_dir().map_err(|e| e.to_string())?
            .join("files")
            .join("coolapk-desktop-update"))
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        Ok(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("updates"))
    }
    #[cfg(not(any(target_os = "android", target_os = "macos", target_os = "linux")))]
    {
        let _ = app;
        Ok(std::env::temp_dir().join("coolapk-desktop-update"))
    }
}

fn is_update_package_extension(path: &std::path::Path) -> bool {
    let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default();
    #[cfg(target_os = "android")]
    { extension.eq_ignore_ascii_case("apk") }
    #[cfg(target_os = "macos")]
    { extension.eq_ignore_ascii_case("dmg") }
    #[cfg(target_os = "linux")]
    { extension.eq_ignore_ascii_case("AppImage") || extension.eq_ignore_ascii_case("deb") || extension.eq_ignore_ascii_case("rpm") }
    #[cfg(not(any(target_os = "android", target_os = "macos", target_os = "linux")))]
    { extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("msi") }
}

async fn publish_user_file(app: &tauri::AppHandle, path: &Path) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        static SAVE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _save_guard = SAVE_LOCK.lock().await;
        let mut status = call_android_update_method(app, "publishSavedFile", path.to_string_lossy().to_string()).await?;
        let started = Instant::now();
        while status == "pending" {
            if started.elapsed() > Duration::from_secs(300) { return Err("等待保存位置超时，文件仍保留在应用目录".to_string()); }
            tokio::time::sleep(Duration::from_millis(300)).await;
            status = call_android_update_method(app, "takeSavedFileResult", String::new()).await?;
        }
        if !status.starts_with("content://") { return Err(status.trim_start_matches("error:").to_string()); }
        if !path.extension().is_some_and(|extension| ["apk", "xapk", "apks"].iter().any(|allowed| extension.eq_ignore_ascii_case(allowed))) {
            let _ = tokio::fs::remove_file(path).await;
        }
        Ok(status)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(path.to_string_lossy().to_string())
    }
}

async fn open_local_path(app: &tauri::AppHandle, path: &Path) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let status = call_android_update_method(app, "openLocalPath", path.to_string_lossy().to_string()).await?;
        if status == "opened" { Ok(()) } else { Err(status.trim_start_matches("error:").to_string()) }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        opener::open(path).map_err(|error| format!("打开文件位置失败：{error}"))
    }
}

#[cfg(target_os = "android")]
async fn install_update_android(app: tauri::AppHandle, installer_path: String) -> Result<String, String> {
    let location = if installer_path.starts_with("content://") {
        installer_path
    } else {
        let canonical = std::fs::canonicalize(&installer_path)
            .map_err(|_| "更新安装包不存在，可能已被清理，请重新下载".to_string())?;
        let expected_dir = update_cache_dir(&app)?
            .canonicalize()
            .map_err(|_| "更新目录不存在，请重新下载 APK".to_string())?;
        if !canonical.starts_with(&expected_dir) || !is_update_package_extension(&canonical) {
            return Err("拒绝安装不在更新目录内的 APK".to_string());
        }
        canonical.to_string_lossy().to_string()
    };
    let status = call_android_update_method(&app, "launchUpdateInstaller", location).await?;
    match status.as_str() {
        "started" | "permission_required" => Ok(status),
        error if error.starts_with("error:") => Err(error.trim_start_matches("error:").to_string()),
        _ => Err(format!("Android 安装器返回未知结果：{status}")),
    }
}

#[cfg(target_os = "android")]
pub(crate) async fn call_android_update_method(
    app: &tauri::AppHandle,
    method: &'static str,
    argument: String,
) -> Result<String, String> {
    let window = app.get_webview_window("main")
        .ok_or_else(|| "Android 主窗口不可用".to_string())?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window.with_webview(move |webview| {
        webview.jni_handle().exec(move |env, activity, _| {
            let result: jni::errors::Result<String> = (|| {
                let argument = env.new_string(argument)?;
                let value = env.call_method(
                    activity,
                    method,
                    "(Ljava/lang/String;)Ljava/lang/String;",
                    &[(&argument).into()],
                )?.l()?;
                let value = jni::objects::JString::from(value);
                Ok(env.get_string(&value)?.into())
            })();
            let result = result.map_err(|error| {
                // JNI 默认只返回 JavaException；清除挂起异常后读取真实原因，便于定位混淆和系统安装器错误。
                let exception = env.exception_occurred().ok();
                let _ = env.exception_clear();
                let detail = exception.filter(|value| !value.is_null()).and_then(|exception| {
                    let value = env.call_method(exception, "toString", "()Ljava/lang/String;", &[]).ok()?.l().ok()?;
                    let value = jni::objects::JString::from(value);
                    let detail: String = env.get_string(&value).ok()?.into();
                    Some(detail)
                });
                // 读取异常详情本身也可能抛出异常，不能污染后续 JNI 调用。
                let _ = env.exception_clear();
                format!("Android {method} 调用失败：{}", detail.unwrap_or_else(|| error.to_string()))
            });
            let _ = sender.send(result);
        });
    }).map_err(|error| format!("调用 Android 安装器失败：{error}"))?;
    tokio::time::timeout(Duration::from_secs(120), receiver)
        .await
        .map_err(|_| "等待 Android 安装器响应超时".to_string())?
        .map_err(|_| "Android 安装器未返回结果".to_string())?
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn install_update_windows(app: &tauri::AppHandle, installer_path: String, portable: bool) -> Result<(), String> {
    // 只允许执行更新目录内的 .exe/.msi 安装包：
    // 路径必须真实存在于下载目录（canonicalize 解析 .. / 符号链接后再前缀校验），
    // 防止前端被注入时借助该命令执行任意文件。
    let canonical = std::fs::canonicalize(&installer_path)
        .map_err(|_| "更新安装包不存在，可能已被清理，请重新下载".to_string())?;
    let expected_dir = update_cache_dir(app)?;
    let expected_dir = expected_dir.canonicalize().unwrap_or(expected_dir);
    if !canonical.starts_with(&expected_dir) {
        return Err("拒绝安装不在更新目录内的文件".to_string());
    }
    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "exe" && ext != "msi" {
        return Err("拒绝安装非安装包文件".to_string());
    }

    if portable {
        if ext != "exe" {
            return Err("便携版更新包必须是 EXE 文件".to_string());
        }
        let current_exe = std::env::current_exe()
            .and_then(std::fs::canonicalize)
            .map_err(|error| format!("无法定位当前程序：{error}"))?;
        let current_dir = current_exe
            .parent()
            .ok_or_else(|| "无法定位便携版所在目录".to_string())?;
        let write_probe = current_dir.join(format!(
            ".coolapk-update-write-test-{}",
            std::process::id()
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&write_probe)
            .map_err(|error| format!("便携版所在目录不可写，无法自动更新：{error}"))?;
        let _ = std::fs::remove_file(write_probe);
        // 更新助手是从更新目录直接启动的新版本本体，加载期就需要 WebView2Loader.dll。
        // 便携版所在的目录带这个 DLL，必须同步到更新目录，否则助手会以 0xC0000135 静默退出，更新看起来毫无反应。
        stage_portable_update_loader(&current_dir, &expected_dir)?;
        let mut child = std::process::Command::new(&canonical)
            .arg("--coolapk-apply-portable-update")
            .arg(std::process::id().to_string())
            .arg(current_exe)
            .spawn()
            .map_err(|error| format!("启动便携版更新助手失败：{error}"))?;
        // 助手启动后应进入“等待旧进程退出”的状态；立刻退出说明依赖仍然缺失。
        std::thread::sleep(std::time::Duration::from_millis(400));
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "更新助手启动后立即退出（退出码 {}），可能是更新包依赖缺失，请手动下载新版",
                status.code().unwrap_or(-1)
            ));
        }
        return Ok(());
    }

    std::process::Command::new(&canonical)
        .args(["/S", "/UPDATE", "/R"])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 便携版更新包是单个 EXE，但它在加载期就依赖 WebView2Loader.dll。
/// 更新助手从更新目录直接启动，因此必须把便携版目录里的 DLL 一起放到更新包旁边。
#[cfg(target_os = "windows")]
fn stage_portable_update_loader(
    app_dir: &std::path::Path,
    update_dir: &std::path::Path,
) -> Result<(), String> {
    let source = app_dir.join("WebView2Loader.dll");
    if !source.is_file() {
        return Ok(());
    }
    let target = update_dir.join("WebView2Loader.dll");
    let same_size = match (std::fs::metadata(&target), std::fs::metadata(&source)) {
        (Ok(staged), Ok(origin)) => staged.len() == origin.len(),
        _ => false,
    };
    if same_size {
        return Ok(());
    }
    if let Err(error) = std::fs::copy(&source, &target) {
        // 目标 DLL 已被上一次的助手进程加载时无法覆盖，但文件本身已经可用。
        if target.is_file() {
            return Ok(());
        }
        return Err(format!(
            "准备更新助手依赖失败：{error}，请手动下载新版安装包"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod cache_tests {
    use super::{
        build_download_file_name, build_generated_image_file_name, build_image_file_name, decode_file_data_url, decode_image_data_url, next_available_file_path, read_image_cache,
        save_image_bytes, validate_custom_dir, write_image_cache,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn image_cache_round_trip_keeps_binary_data() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("coolapk-image-cache-test-{unique}"));
        let path = root.join("sample.bin");
        let expected = "data:image/png;base64,Y2FjaGUtdGVzdA==";

        write_image_cache(&path, expected).await.unwrap();
        let actual = read_image_cache(&path, 7).await;

        assert_eq!(actual.as_deref(), Some(expected));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn invalid_image_cache_is_ignored() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("coolapk-image-cache-invalid-{unique}"));
        let path = root.join("sample.bin");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&path, b"broken-cache").unwrap();

        assert!(read_image_cache(&path, 7).await.is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn image_save_helpers_keep_original_format_and_avoid_overwrite() {
        let (mime_type, bytes) = decode_image_data_url("data:image/png;base64,YWJj").unwrap();
        assert_eq!(mime_type, "image/png");
        assert_eq!(bytes, b"abc");
        assert_eq!(
            build_image_file_name("https://image.coolapk.com/feed/2026/abc123.jpg", mime_type),
            "abc123.png"
        );

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("coolapk-image-save-test-{unique}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("abc123.png"), b"existing").unwrap();
        assert_eq!(
            next_available_file_path(&root, "abc123.png"),
            root.join("abc123_2.png")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn image_save_detects_gif_bytes_despite_jpeg_content_type() {
        // 模拟 CDN 错报 JPEG，确认下载内容保持 GIF 字节且文件名使用 GIF 扩展名。
        for payload in ["R0lGODlh", "R0lGODdh"] {
            let data_url = format!("data:image/jpeg;charset=utf-8;base64,{payload}");
            let (mime_type, bytes) = decode_image_data_url(&data_url).unwrap();
            assert_eq!(mime_type, "image/gif");
            assert!(bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a"));
            assert_eq!(build_image_file_name("https://image.coolapk.com/feed/demo.jpg", mime_type), "demo.gif");
        }
    }

    #[test]
    fn generic_download_names_preserve_any_format_and_server_names() {
        assert_eq!(build_download_file_name("https://example.com/demo.jpg", "", "image/jpeg", b"GIF89a"), "demo.gif");
        assert_eq!(build_download_file_name("https://example.com/demo.jpg", "", "image/jpeg", b"RIFFxxxxWEBP"), "demo.webp");
        assert_eq!(build_download_file_name("https://example.com/demo.jpg", "", "image/jpeg", b"\0\0\0\x18ftypavif"), "demo.avif");
        assert_eq!(build_download_file_name("https://example.com/demo.jpg", "", "image/heic", b"\0\0\0\x18ftypheic"), "demo.heic");
        assert_eq!(build_download_file_name("https://example.com/demo.jpg", "", "image/svg+xml", b"<svg/>"), "demo.svg");
        for extension in ["mp4", "zip", "7z", "apk", "custom"] {
            let expected = format!("demo.{extension}");
            assert_eq!(build_download_file_name(&format!("https://example.com/{expected}?token=1"), "", "application/octet-stream", b"binary"), expected);
        }
        assert_eq!(build_download_file_name("https://example.com/download", "attachment; filename=backup.zip", "application/zip", b"PK\x03\x04"), "backup.zip");
        assert_eq!(build_download_file_name("https://example.com/download", "attachment; filename=\"报告;2026.custom\"", "application/octet-stream", b""), "报告;2026.custom");
        assert_eq!(build_download_file_name("https://example.com/download", "attachment; filename*=UTF-8''%中文.custom", "application/octet-stream", b""), "%中文.custom");
        assert_eq!(build_download_file_name("https://example.com/download", "attachment; filename=fallback.zip; filename*=UTF-8''%E6%B5%8B%E8%AF%95%2B.zip", "application/zip", b"PK\x03\x04"), "测试+.zip");
        assert_eq!(build_download_file_name("https://example.com/download", "attachment; filename=../../CON.zip", "application/zip", b""), "coolapk_CON.zip");
        assert_eq!(build_download_file_name("https://example.com/", "", "application/octet-stream", b"unknown"), "coolapk_file.bin");
        assert_eq!(build_download_file_name("https://example.com/", "", "video/mp4", b""), "coolapk_file.mp4");
    }

    #[test]
    fn loaded_file_data_is_decoded_without_changing_binary_bytes() {
        let (mime, bytes) = decode_file_data_url("data:application/octet-stream;base64,AP/+AQ==").unwrap();
        assert_eq!(mime, "application/octet-stream");
        assert_eq!(bytes, [0, 255, 254, 1]);
        assert_eq!(decode_file_data_url("data:application/zip,%00%FF%FE%01").unwrap().1, bytes);
        assert_eq!(decode_file_data_url("data:,a+b%20c").unwrap(), ("text/plain", b"a+b c".to_vec()));
        assert!(decode_file_data_url("data:application/zip,%GG").is_err());
    }

    #[test]
    fn generated_image_file_name_is_safe_and_uses_payload_format() {
        assert_eq!(
            build_generated_image_file_name("coolapk-feed-42.png", "image/jpeg"),
            "coolapk-feed-42.jpg"
        );
        assert_eq!(
            build_generated_image_file_name("../unsafe/name.png", "image/png"),
            "name.png"
        );
    }

    #[test]
    fn json_export_path_avoids_overwrite() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("coolapk-json-export-test-{unique}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("history.json"), b"existing").unwrap();

        assert_eq!(
            next_available_file_path(&root, "history.json"),
            root.join("history_2.json")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn concurrent_image_saves_do_not_overwrite_each_other() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("coolapk-image-concurrent-{unique}"));
        std::fs::create_dir_all(&root).unwrap();

        let (first, second) = tokio::join!(
            save_image_bytes(&root, "same.png", b"first"),
            save_image_bytes(&root, "same.png", b"second")
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(first).unwrap(), b"first");
        assert_eq!(std::fs::read(second).unwrap(), b"second");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_custom_directory_that_is_not_absolute_on_current_platform() {
        #[cfg(target_os = "windows")]
        let incompatible = "/home/user/Downloads";
        #[cfg(not(target_os = "windows"))]
        let incompatible = r"D:\Downloads";

        assert!(validate_custom_dir(incompatible, "自定义目录").is_err());
        assert!(validate_custom_dir("relative/downloads", "自定义目录").is_err());
    }
}

#[cfg(test)]
mod download_tests {
    use super::{
        build_coolapk_download_url, constrain_windows_download_file_name,
        empty_download_verification, partial_download_path, sanitize_apk_file_name,
        validate_download_path_for_file_operation,
    };
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn download_file_name_keeps_supported_extension_and_removes_path_separators() {
        assert_eq!(sanitize_apk_file_name("酷安/测试.apk").unwrap(), "酷安_测试.apk");
        assert_eq!(sanitize_apk_file_name("demo").unwrap(), "demo.apk");
        assert!(sanitize_apk_file_name("..").is_err());
        #[cfg(windows)]
        assert_eq!(sanitize_apk_file_name("CON.apk").unwrap(), "_CON.apk");
    }

    #[test]
    fn download_partial_path_keeps_platform_path_separators() {
        let target = std::env::temp_dir().join("coolapk").join("demo.apk");
        assert_eq!(partial_download_path(&target).unwrap(), target.with_file_name("demo.apk.part"));
    }

    #[test]
    fn long_download_file_name_is_shortened_only_for_windows() {
        let root = std::env::temp_dir().join("coolapk-download-path");
        let file_name = format!("{}.apk", "a".repeat(400));
        let result = constrain_windows_download_file_name(&root, file_name.clone()).unwrap();
        #[cfg(windows)]
        assert!(root.join(&result).to_string_lossy().encode_utf16().count() <= 240);
        #[cfg(not(windows))]
        assert_eq!(result, file_name);
    }

    #[test]
    fn official_download_url_uses_the_v6_download_endpoint_and_required_fields() {
        let url = build_coolapk_download_url("com.demo.app", "943417", "275").unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("api.coolapk.com"));
        assert_eq!(url.path(), "/v6/apk/download");
        assert_eq!(url.query(), Some("pn=com.demo.app&aid=943417&vc=275&extra="));
    }

    #[test]
    fn app_detail_version_accepts_coolapk_apkversioncode_field() {
        let detail = json!({ "apkversioncode": 7245864 });
        assert_eq!(super::download_object_string(Some(&detail), &["versionCode", "versioncode", "version_code", "apkversioncode"]), Some("7245864".to_string()));
    }

    #[test]
    fn only_official_api_hosts_use_the_coolapk_post_download_protocol() {
        assert!(super::is_coolapk_download_host("api.coolapk.com"));
        assert!(super::is_coolapk_download_host("api-dev.coolapk.com"));
        assert!(!super::is_coolapk_download_host("download.coolapk.com"));
        assert!(!super::is_coolapk_download_host("cdn.coolapk.com"));
    }

    #[test]
    fn download_verification_only_rejects_an_explicit_empty_result() {
        assert!(empty_download_verification(&json!({ "data": "" })));
        assert!(!empty_download_verification(&json!({ "data": "verified" })));
        assert!(!empty_download_verification(&json!({ "data": { "ok": true } })));
    }

    #[test]
    fn download_file_operations_accept_apk_and_partial_paths() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("coolapk-download-path-test-{unique}"));
        std::fs::create_dir_all(&root).unwrap();
        let apk = root.join("demo.apk").to_string_lossy().to_string();
        let partial = root.join("demo.apk.part").to_string_lossy().to_string();
        let text = root.join("demo.txt").to_string_lossy().to_string();
        assert!(validate_download_path_for_file_operation(&apk).is_ok());
        assert!(validate_download_path_for_file_operation(&partial).is_ok());
        assert!(validate_download_path_for_file_operation(&text).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}


// ===== 按域拆分的命令子模块 =====

mod notification_message;
mod system_settings;
mod update_download;
mod auth;
mod device_network;
mod topic_dyh;
mod product;
mod app_album;
mod search;
mod media;
mod user;
mod home_discovery;
mod feed;
mod misc;

pub use notification_message::*;
pub use system_settings::*;
pub use update_download::*;
pub use auth::*;
pub use device_network::*;
pub use topic_dyh::*;
pub use product::*;
pub use app_album::*;
pub use search::*;
pub use media::*;
pub use user::*;
pub use home_discovery::*;
pub use feed::*;
pub use misc::*;