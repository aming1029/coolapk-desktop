use super::*;

#[tauri::command]
pub fn set_close_to_tray(enabled: bool) {
    CLOSE_TO_TRAY.store(enabled, Ordering::SeqCst);
}

#[tauri::command]
pub fn set_window_theme(app: tauri::AppHandle, theme: Option<String>) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        let tauri_theme = match theme.as_deref() {
            Some("dark") => Some(tauri::Theme::Dark),
            Some("light") => Some(tauri::Theme::Light),
            _ => None,
        };
        w.set_theme(tauri_theme).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_android_system_bar_theme(app: tauri::AppHandle, dark: bool, profile: Option<bool>, dimmed: Option<bool>) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let theme = match (dark, profile.unwrap_or(false)) { (true, true) => "dark-profile", (false, true) => "light-profile", (true, false) => "dark", (false, false) => "light" };
        let theme = if dimmed.unwrap_or(false) { format!("{theme}-dimmed") } else { theme.to_string() };
        coolapk::commands::call_android_update_method(&app, "setSystemBarTheme", theme).await?;
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, dark, profile, dimmed);
    Ok(())
}

#[tauri::command]
pub async fn get_android_safe_area_top(app: tauri::AppHandle) -> Result<f64, String> {
    #[cfg(target_os = "android")]
    {
        // 返回物理像素，由网页按 WebView 的像素比例换算，避免字号或显示缩放造成偏移。
        let raw = coolapk::commands::call_android_update_method(&app, "getSafeAreaTop", String::new()).await?;
        raw.parse::<f64>().map_err(|_| "Android 顶部安全区格式错误".to_string())
    }
    #[cfg(not(target_os = "android"))]
    { let _ = app; Ok(0.0) }
}

#[tauri::command]
pub async fn log_android_runtime_info(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let raw = coolapk::commands::call_android_update_method(&app, "getRuntimeInfo", String::new()).await?;
        let info: serde_json::Value = serde_json::from_str(&raw).map_err(|_| "Android 环境信息格式错误")?;
        let field = |key: &str| info.get(key).and_then(serde_json::Value::as_str).unwrap_or("unavailable");
        tracing::info!("runtime.android android_release={} android_sdk={} manufacturer={} model={} webview_package={} webview_version={}",
            field("android_release"), info.get("android_sdk").and_then(serde_json::Value::as_u64).unwrap_or(0),
            field("manufacturer"), field("model"), field("webview_package"), field("webview_version"));
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
    Ok(())
}

#[cfg(windows)]
pub fn choose_windows_font_family(
    owner: windows::Win32::Foundation::HWND,
    current_font: Option<&str>,
) -> Result<Option<String>, String> {
    use std::mem::size_of;
    use windows::Win32::Graphics::Gdi::LOGFONTW;
    use windows::Win32::UI::Controls::Dialogs::{
        CF_INITTOLOGFONTSTRUCT, CF_SCREENFONTS, CHOOSEFONTW, ChooseFontW,
    };

    let mut log_font = LOGFONTW::default();
    let mut flags = CF_SCREENFONTS;
    if let Some(font) = current_font.map(str::trim).filter(|font| !font.is_empty()) {
        let encoded: Vec<u16> = font
            .encode_utf16()
            .take(log_font.lfFaceName.len().saturating_sub(1))
            .collect();
        log_font.lfFaceName[..encoded.len()].copy_from_slice(&encoded);
        flags = flags | CF_INITTOLOGFONTSTRUCT;
    }

    let mut choose_font = CHOOSEFONTW {
        lStructSize: size_of::<CHOOSEFONTW>() as u32,
        hwndOwner: owner,
        lpLogFont: &mut log_font,
        Flags: flags,
        ..Default::default()
    };

    if !unsafe { ChooseFontW(&mut choose_font).as_bool() } {
        // 取消选择器不是错误，前端保留当前字体。
        return Ok(None);
    }

    let length = log_font
        .lfFaceName
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(log_font.lfFaceName.len());
    let font = String::from_utf16(&log_font.lfFaceName[..length])
        .map_err(|error| format!("读取系统字体名称失败：{error}"))?;
    let font = font.trim().to_string();
    if font.is_empty() {
        return Err("系统字体选择器没有返回字体名称".to_string());
    }
    Ok(Some(font))
}

#[tauri::command]
pub async fn pick_font_family(
    window: tauri::WebviewWindow,
    current_font: Option<String>,
) -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        let owner = window
            .hwnd()
            .map_err(|error| format!("获取主窗口句柄失败：{error}"))?
            .0 as isize;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        window
            .run_on_main_thread(move || {
                let owner = windows::Win32::Foundation::HWND(owner as *mut std::ffi::c_void);
                let result = choose_windows_font_family(owner, current_font.as_deref());
                let _ = sender.send(result);
            })
            .map_err(|error| format!("调度系统字体选择器失败：{error}"))?;
        receiver
            .await
            .map_err(|_| "系统字体选择器未返回结果".to_string())?
    }

    #[cfg(not(windows))]
    {
        let _ = (window, current_font);
        Err("系统字体选择器目前仅支持 Windows".to_string())
    }
}

#[tauri::command]
pub async fn set_live_tile_enabled(
    app: tauri::AppHandle,
    enabled: bool,
    source: Option<String>,
) -> Result<(), String> {
    crate::windows_tile::set_live_tile_enabled_flag(enabled);
    if !enabled {
        // 先停止周期任务，再清空队列并让在途刷新失效。
        crate::windows_tile::stop_tile_refresh_task()?;
        return crate::windows_tile::clear_tile();
    }
    if !crate::windows_tile::supports_live_tile() {
        return Ok(());
    }
    // 定时器仅在用户启用时存在；开关切换的立即刷新仍直接使用 UI 传入的数据源。
    crate::windows_tile::start_tile_refresh_task(app.clone(), false)?;
    let override_source = source
        .as_deref()
        .map(|raw| crate::windows_tile::LiveTileSource::from_setting(Some(raw)));
    crate::windows_tile::refresh_tile(&app, override_source).await
}

#[tauri::command]
pub async fn update_desktop_tile(
    app: tauri::AppHandle,
    source: Option<String>,
) -> Result<(), String> {
    if !crate::windows_tile::is_live_tile_enabled() {
        return Ok(());
    }
    let override_source = source
        .as_deref()
        .map(|raw| crate::windows_tile::LiveTileSource::from_setting(Some(raw)));
    crate::windows_tile::refresh_tile(&app, override_source).await
}

#[tauri::command]
pub fn get_platform_info() -> serde_json::Value {
    // 只查一次系统，避免两个字段各查一遍。
    let tile_status = crate::windows_tile::live_tile_status();
    serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        // 是否为 Windows 10（内部版本号 < 22000），前端据此在设置中显示动态磁贴开关与说明。
        "isWindows10": crate::windows_tile::is_windows_10(),
        // 本机当前进程是否具备动态磁贴运行条件（Windows && Windows 10 && 有包标识）。
        "supportsLiveTile": tile_status == crate::windows_tile::LiveTileStatus::Ready,
        // 同一判定的**带原因**形式，供诊断使用。前端目前不消费它 ——
        // 保留 `supportsLiveTile` 是为了不破坏既有契约。
        // 用户报「没有磁贴」时，这两项能一句话定位是版本、身份还是平台问题。
        "liveTileStatus": tile_status.as_str(),
        "liveTileStatusReason": tile_status.reason(),
    })
}

#[cfg(windows)]
pub fn windows_notification_icon_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let directory = app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = directory.join("coolapk-notification-icon.png");
    let icon = include_bytes!("../icons/icon.png");
    if std::fs::read(&path).ok().as_deref() != Some(icon.as_slice()) {
        std::fs::write(&path, icon).map_err(|error| error.to_string())?;
    }
    Ok(path)
}

#[cfg(windows)]
pub fn register_windows_notification_identity(app: &tauri::AppHandle) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let identifier = &app.config().identifier;
    let display_name = app
        .config()
        .product_name
        .clone()
        .unwrap_or_else(|| "酷安".to_string());
    let icon_path = windows_notification_icon_path(app)?;
    let registry = RegKey::predef(HKEY_CURRENT_USER);
    let path = format!("Software\\Classes\\AppUserModelId\\{identifier}");
    let (key, _) = registry
        .create_subkey(path)
        .map_err(|error| error.to_string())?;
    key.set_value("DisplayName", &display_name)
        .map_err(|error| error.to_string())?;
    key.set_value("IconUri", &icon_path.to_string_lossy().to_string())
        .map_err(|error| error.to_string())?;
    key.set_value("IconBackgroundColor", &"00B578")
        .map_err(|error| error.to_string())?;
    key.set_value("ShowInSettings", &1_u32)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(any(windows, test))]
pub fn escape_notification_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[tauri::command]
pub async fn send_desktop_notification(
    app: tauri::AppHandle,
    title: String,
    body: Option<String>,
    android: Option<serde_json::Value>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let mut payload = android.unwrap_or_else(|| serde_json::json!({}));
        payload["title"] = serde_json::json!(title);
        payload["body"] = serde_json::json!(body.unwrap_or_default());
        let result = coolapk::commands::call_android_update_method(&app, "showCoolNotification", payload.to_string()).await?;
        return if let Some(error) = result.strip_prefix("error:") { Err(error.into()) } else { Ok(()) };
    }
    #[cfg(not(target_os = "android"))]
    let _ = android;
    #[cfg(target_os = "ios")]
    {
        use tauri_plugin_notification::NotificationExt;

        let mut notification = app.notification().builder().title(&title);
        if let Some(body) = body.as_deref().filter(|value| !value.trim().is_empty()) {
            notification = notification.body(body);
        }
        return notification.show().map_err(|error| error.to_string());
    }

    #[cfg(desktop)]
    {
    // AUMID 必须与当前形态匹配：有包标识时用 `<PackageFamilyName>!<ApplicationId>`，
    // 未打包时用注册在 HKCU 的 BareId。混用会让通知挂到一个清单里不存在的身份上。
    #[cfg(windows)]
    let identifier = crate::windows_tile::notification_aumid(&app.config().identifier);
    #[cfg(windows)]
    let icon_path = windows_notification_icon_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        {
            use windows::Data::Xml::Dom::XmlDocument;
            use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
            use windows::core::HSTRING;

            let title = escape_notification_xml(&title);
            let body = escape_notification_xml(body.as_deref().unwrap_or_default());
            let icon_uri = format!(
                "file:///{}",
                icon_path.to_string_lossy().replace('\\', "/")
            );
            let xml = format!(
                r#"<toast duration="short"><visual><binding template="ToastGeneric"><image placement="appLogoOverride" hint-crop="circle" src="{icon_uri}" alt="酷安"/><text>{title}</text><text>{body}</text></binding></visual></toast>"#
            );
            let document = XmlDocument::new().map_err(|error| error.to_string())?;
            document
                .LoadXml(&HSTRING::from(xml))
                .map_err(|error| error.to_string())?;
            let toast = ToastNotification::CreateToastNotification(&document)
                .map_err(|error| error.to_string())?;
            let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
                identifier,
            ))
            .map_err(|error| error.to_string())?;
            notifier.Show(&toast).map_err(|error| error.to_string())?;
            std::thread::sleep(std::time::Duration::from_secs(3));
            notifier.Hide(&toast).map_err(|error| error.to_string())?;
            return Ok(());
        }

        #[cfg(not(windows))]
        {
            let mut notification = notify_rust::Notification::new();
            notification.summary(&title).auto_icon();
            if let Some(body) = body.filter(|value| !value.trim().is_empty()) {
                notification.body(&body);
            }
            notification
                .show()
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
    })
    .await
    .map_err(|error| error.to_string())?
    }
}

/// 前端保存启动参数（静默启动 / 记忆窗口状态 / 置顶），重启后由 setup 读取生效
#[tauri::command]
pub fn set_startup_flags(
    app: tauri::AppHandle,
    start_minimized: bool,
    remember_window_state: bool,
    always_on_top: bool,
) -> Result<(), String> {
    START_MINIMIZED.store(start_minimized, Ordering::SeqCst);
    REMEMBER_WINDOW_STATE.store(remember_window_state, Ordering::SeqCst);
    ALWAYS_ON_TOP.store(always_on_top, Ordering::SeqCst);

    let path = startup_state_path(&app).ok_or("无法获取应用数据目录")?;
    update_startup_state_file(&path, serde_json::json!({}), |flags| {
        flags["start_minimized"] = serde_json::json!(start_minimized);
        flags["remember_window_state"] = serde_json::json!(remember_window_state);
        flags["always_on_top"] = serde_json::json!(always_on_top);
    })
}

/// 主窗口导航白名单：只允许应用自身源（dev 固定端口 / 打包后 tauri 自定义协议源）。
/// 主窗口一旦导航到外部域名，外部页面会接管整个窗口（钓鱼/UI 混淆风险），
/// 且登录回跳等依赖主窗口 URL 的逻辑会失去正确来源。
/// 外部链接一律由页面级 handleAnchorClick 在新窗口打开，这里仅作兜底防线。
pub fn is_main_window_navigation_allowed(url: &tauri::Url) -> bool {
    let scheme = url.scheme().to_ascii_lowercase();
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    match scheme.as_str() {
        "http" | "https" => {
            host == "tauri.localhost" || (host == "127.0.0.1" && url.port() == Some(17520))
        }
        "tauri" => host == "localhost",
        _ => false,
    }
}
