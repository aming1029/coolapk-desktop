use super::*;

#[tauri::command]
pub async fn get_hidden_replies(
    state: State<'_, AppState>,
    feed_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_hidden_replies(&feed_id, page).await
}

#[tauri::command]
pub async fn resolve_video_url(
    state: State<'_, AppState>,
    request_params: String,
) -> Result<Value, String> {
    state.client.resolve_video_url(&request_params).await
}

#[tauri::command]
pub async fn get_sub_replies(
    state: State<'_, AppState>,
    feed_id: String,
    reply_id: String,
    page: u32,
    last_item: Option<String>,
    post_token: Option<String>,
    post_token_field: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_sub_replies_with_captcha(
            &feed_id,
            &reply_id,
            page,
            last_item.as_deref().unwrap_or(""),
            post_token.as_deref(),
            post_token_field.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn open_url(app: tauri::AppHandle, url: String, mode: Option<String>) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static BROWSER_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

    // 协议白名单：仅允许 http/https/mailto/tel。
    // 拒绝 file:、ms-msdt:、smb:、javascript: 等可被系统协议处理器滥用的 scheme，
    // 防止来自动态/评论里的恶意链接触发本地程序。
    let parsed = reqwest::Url::parse(&url).map_err(|e| format!("无效链接: {e}"))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https" | "mailto" | "tel") {
        return Err(format!("不支持的链接协议: {scheme}"));
    }

    // 非酷安域名直接交给系统浏览器，即使调用方传了 internal 也不创建应用内窗口。
    let system_mode = mode.as_deref() == Some("system");
    let coolapk_host = parsed.host_str().is_some_and(|host| {
        host == "coolapk.com" || host.ends_with(".coolapk.com")
    }) && parsed.username().is_empty() && parsed.password().is_none();
    if system_mode || !coolapk_host || (scheme != "http" && scheme != "https") {
        use tauri_plugin_opener::OpenerExt;
        // 使用插件实例才能在移动端调用 Android Intent / iOS 原生接口。
        return app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string());
    }

    // 应用本身即 WebView 浏览器：外部链接在新开窗口内浏览，不调起系统浏览器
    let label = format!(
        "browser_window_{}",
        BROWSER_WINDOW_ID.fetch_add(1, Ordering::Relaxed)
    );
    let title = parsed.host_str().unwrap_or("链接").to_string();

    // 移动端 UA：酷安网页（如账号安全页）在桌面 UA 下会白屏，与登录窗口同一套已验证可用的 UA
    let equipment_editor = is_equipment_action_url(&parsed, "editProductOwner");
    let equipment_page = equipment_editor || is_equipment_action_url(&parsed, "productOwnerShare");
    // 先创建空白窗口并同步登录 Cookie，首次请求就必须携带当前账号身份。
    let initial_url = if equipment_page { reqwest::Url::parse("about:blank").unwrap() } else { parsed.clone() };
    let navigation_app = app.clone();
    let editor_label = label.clone();
    let browser = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(initial_url))
        .initialization_script(r#"
            if (location.href === 'about:blank') {
                const showLoading = () => {
                    document.title = '正在打开装备页面';
                    const body = document.body || document.documentElement.appendChild(document.createElement('body'));
                    body.style.cssText = 'font:16px system-ui;padding:48px;color:#333;background:#fff';
                    body.textContent = '正在同步登录状态并打开装备页面…';
                };
                if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', showLoading, { once: true });
                else showLoading();
            }
        "#)
        .on_navigation(move |url| {
            if equipment_editor && is_equipment_action_url(url, "addProductOwner") {
                let app = navigation_app.clone();
                let window_label = editor_label.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(main) = app.get_webview_window("main") {
                        if let Err(error) = main.emit("equipment-product-picker", json!({ "windowLabel": window_label })) {
                            log::warn!("装备产品选择器打开失败: {error}");
                            return;
                        }
                        let _ = main.set_focus();
                    }
                });
                return false;
            }
            true
        })
        .title(title)
        .user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Mobile/15E148 Safari/604.1")
        .inner_size(1100.0, 780.0);
    #[cfg(desktop)]
    let browser = browser.center();
    #[cfg(desktop)]
    let browser = browser.decorations(true);
    let window = browser
        .visible(true)
        .build()
        .map_err(|e| e.to_string())?;
    if equipment_page {
        log::info!("equipment.window_created");
        // 焦点请求可能被系统拒绝，但不应因此关闭已经创建的窗口。
        let _ = window.set_focus();
        let cookie = app.state::<AppState>().client.get_user_cookie();
        let result = async {
            tokio::time::timeout(Duration::from_secs(10), sync_equipment_webview_cookie(&window, cookie.as_deref()))
                .await.map_err(|_| "装备窗口登录态同步超时，请关闭该窗口后重新尝试".to_string())??;
            window.navigate(parsed).map_err(|_| "无法加载装备页面".to_string())?;
            let _ = window.set_focus();
            Ok::<(), String>(())
        }.await;
        if let Err(message) = &result {
            log::warn!("equipment.window_open_failed reason={message}");
            let _ = window.eval(&equipment_window_error_script(message));
        }
        result?;
    }
    Ok(())
}

#[tauri::command]
pub async fn fetch_external_page(state: State<'_, AppState>, url: String) -> Result<Value, String> {
    state.client.fetch_external_page(&url).await
}
