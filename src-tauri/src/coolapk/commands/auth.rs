use super::*;

#[tauri::command]
pub async fn check_login_info(state: State<'_, AppState>) -> Result<Value, String> {
    let result = state.client.check_login_info().await;
    match &result {
        Ok(_) => log::info!("login.info_check_succeeded"),
        Err(error) => log::warn!(
            "login.info_check_failed reason={}",
            login_failure_kind(error)
        ),
    }
    result
}

#[tauri::command]
pub async fn save_cookie_securely(
    state: State<'_, AppState>,
    cookie_str: String,
) -> Result<String, String> {
    log::info!(
        "login.cookie_received has_session={}",
        CoolapkClient::has_valid_session_cookie(&cookie_str)
    );
    state.client.set_user_cookie(cookie_str)?;
    // 保存和验证分开：回调页必须先确认服务端返回真实账号，再关登录窗口。
    log::info!("login.cookie_staged");
    Ok("登录 Cookie 已载入，正在验证会话".to_string())
}

#[tauri::command]
pub async fn check_login_status(state: State<'_, AppState>) -> Result<Value, String> {
    log::info!("login.status_check");
    let result = state.client.check_login_status().await;
    match &result {
        Ok(_) => log::info!("login.status_check_succeeded"),
        Err(error) => log::warn!(
            "login.status_check_failed reason={}",
            login_failure_kind(error)
        ),
    }
    result
}

#[tauri::command]
pub fn clear_user_cookie(state: State<'_, AppState>) -> Result<String, String> {
    state.client.clear_user_cookie()?;
    Ok("登录状态已清除".to_string())
}

#[tauri::command]
pub fn get_user_cookie(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.client.get_user_cookie())
}

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.list_accounts().await
}

#[tauri::command]
pub async fn login_as(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.login_as(&uid).await
}

#[tauri::command]
pub async fn save_account(
    state: State<'_, AppState>,
    uid: String,
    username: String,
    user_avatar: String,
    cookie: String,
) -> Result<Value, String> {
    state
        .client
        .save_account(&uid, &username, &user_avatar, &cookie)
        .await
}

#[tauri::command]
pub async fn persist_current_account(
    state: State<'_, AppState>,
    uid: String,
    username: String,
    user_avatar: String,
) -> Result<Value, String> {
    state
        .client
        .persist_current_account(&uid, &username, &user_avatar)
        .await
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.remove_account(&uid).await
}

#[tauri::command]
pub async fn login_by_account(
    state: State<'_, AppState>,
    account: String,
    password: String,
) -> Result<Value, String> {
    state.client.login_by_account(&account, &password).await
}

#[tauri::command]
pub async fn send_sms_vcode(state: State<'_, AppState>, mobile: String) -> Result<Value, String> {
    state.client.send_sms_vcode(&mobile).await
}

#[tauri::command]
pub async fn login_by_mobile(
    state: State<'_, AppState>,
    mobile: String,
    vcode: String,
) -> Result<Value, String> {
    state.client.login_by_mobile(&mobile, &vcode).await
}

#[tauri::command]
pub fn close_login_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Emitter;
    use tauri::Manager;
    login_checkpoint(LoginStage::CloseRequested);
    let close_result = if let Some(win) = app.get_webview_window("login_window") {
        #[cfg(target_os = "android")]
        {
            // Android 登录页是独立 Activity。Window::close 只移除 Rust 窗口句柄，
            // 不能保证结束前台 Activity；明确调用 finish() 才会返回主界面。
            win.with_webview(|webview| {
                webview.jni_handle().exec(|env, activity, _| {
                    if env.call_method(activity, "finish", "()V", &[]).is_err() {
                        log::warn!("login.activity_finish_failed");
                    }
                });
            })
            .map_err(|error| error.to_string())
        }
        #[cfg(target_os = "ios")]
        {
            // iOS 没有桌面关窗动画，先隐藏登录窗口，再恢复主窗口为前台窗口。
            let result = win.hide().and_then(|_| win.close()).map_err(|error| error.to_string());
            if result.is_ok() {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
            result
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            win.close().map_err(|error| error.to_string())
        }
    } else {
        Ok(())
    };
    // 即使关窗请求失败，也通知主窗口检查已保存的登录态。
    log::info!("login.window_closed close_ok={}", close_result.is_ok());
    if close_result.is_ok() { login_checkpoint(LoginStage::CloseCompleted); }
    let _ = app.emit("login-window-closed", ());
    close_result
}

#[tauri::command]
pub async fn sync_login_webview(app: tauri::AppHandle) -> Result<bool, String> {
    let Some(win) = app.get_webview_window("login_window") else {
        return Ok(false);
    };
    let session = app.state::<AppState>().login_session.lock()
        .map_err(|_| "登录状态不可用".to_string())?.clone();
    let Some(session) = session else {
        return Ok(false);
    };
    log::info!("login.manual_sync_started");
    verify_login_webview(&app, &win, &session, false).await
}

#[tauri::command]
pub async fn open_login_webview(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    login_checkpoint(LoginStage::OpenRequested);

    if let Some(win) = app.get_webview_window("login_window") {
        let _ = win.set_focus();
        return Ok(());
    }

    let app_origin = get_app_origin(&app);
    let callback_url = format!("{}/#/auth_callback", app_origin);
    let target_login = reqwest::Url::parse_with_params(
        "https://account.coolapk.com/auth/login",
        &[("type", "coolapk"), ("forward", callback_url.as_str())],
    )
    .map_err(|e| e.to_string())?
    .to_string();
    // 先发起 logout 清理网页底层 Cookie 旧会话，防止服务端自动 302 静默跳回旧账号，强制弹出全新登录框
    let login_url = reqwest::Url::parse_with_params(
        "https://account.coolapk.com/auth/logout",
        &[("forward", target_login)],
    )
    .map_err(|e| e.to_string())?;

    log::info!("login.webview_opened");

    // 登录完成后由 Rust monitor 读取 WebView2 Cookie 存储，避免 document.cookie 丢失 HttpOnly 和跨域 Cookie。
    let js_script = r#"
        (function() {
            var APP_ORIGIN = "__APP_ORIGIN__";

            function isLogoutPage() {
                var href = window.location.href || "";
                return href.indexOf('auth/logout') !== -1;
            }

            function clearCoolapkCookies() {
                var expires = "Thu, 01 Jan 1970 00:00:00 GMT";
                var names = (document.cookie || "").split(';');
                for (var i = 0; i < names.length; i++) {
                    var name = (names[i].split('=')[0] || "").trim();
                    if (!name) continue;
                    document.cookie = name + "=; expires=" + expires + "; path=/; domain=.coolapk.com";
                    document.cookie = name + "=; expires=" + expires + "; path=/";
                }
            }

            function checkLogoutPage() {
                var text = (document.body && document.body.innerText) || "";
                // 必须等待退出页面真正加载完成，不能只看到 auth/logout URL 就跳转，
                // 否则会取消服务端清理 Cookie 的请求，旧账号会被登录页再次自动识别。
                if (text.indexOf('已经退出登录') !== -1) {
                    clearCoolapkCookies();
                    window.location.replace("https://account.coolapk.com/auth/login?type=coolapk&forward=" + encodeURIComponent(APP_ORIGIN + "/#/auth_callback"));
                    return true;
                }
                return isLogoutPage();
            }

            if (checkLogoutPage() && !isLogoutPage()) return;

            document.addEventListener('DOMContentLoaded', function() {
                checkLogoutPage();
            });
        })();
    "#
    .replace("__APP_ORIGIN__", &app_origin);
    #[cfg(target_os = "ios")]
    let js_script = format!("{js_script}\n{}", include_str!("ios-login-controls.js"));

    // 授权回调通常会立即 302 到 forward 页面，定时读取 win.url() 可能完全看不到它。
    // 在 WebView 导航发生时同步捕获回调 URL，再交给异步任务兑换授权码。
    let session = std::sync::Arc::new(LoginSession::default());
    let navigation_session = session.clone();
    let navigation_app_origin = app_origin.clone();
    #[cfg(target_os = "ios")]
    let navigation_app = app.clone();
    login_checkpoint(LoginStage::WindowBuildStarted);
    let login_window = tauri::WebviewWindowBuilder::new(
        &app,
        "login_window",
        tauri::WebviewUrl::External(login_url),
    )
    .title("酷安官方授权登录")
    .inner_size(440.0, 620.0)
    .on_navigation(move |url| {
        #[cfg(target_os = "ios")]
        if is_ios_login_return_url(url) {
            let app = navigation_app.clone();
            let session = navigation_session.clone();
            tauri::async_runtime::spawn(async move {
                let current = app.state::<AppState>().login_session.lock().ok().and_then(|value| value.clone());
                if !current.as_ref().is_some_and(|value| std::sync::Arc::ptr_eq(value, &session)) { return; }
                if let Some(win) = app.get_webview_window("login_window") {
                    log::info!("login.ios_return_requested");
                    // 退出登录页不恢复旧会话；其他页面返回前先同步，用户无需杀掉应用。
                    let can_sync = win.url().ok().is_some_and(|url| should_poll_ios_login_cookies(&url));
                    if can_sync { let _ = verify_login_webview(&app, &win, &session, false).await; }
                    // 同步期间可能重新打开登录窗口，旧返回任务不能关闭新会话。
                    let current = app.state::<AppState>().login_session.lock().ok().and_then(|value| value.clone());
                    if current.as_ref().is_some_and(|value| std::sync::Arc::ptr_eq(value, &session)) && app.get_webview_window("login_window").is_some() {
                        let _ = close_login_window(app);
                    }
                }
            });
            return false;
        }
        if login_callback_kind(url, &navigation_app_origin).is_some()
            && extract_access_code_from_url(url.as_str()).is_some()
        {
            if let Ok(mut captured) = navigation_session.callback_url.lock() {
                *captured = Some(url.to_string());
            }
            log::info!("login.callback_navigation_captured");
            // 与官方 APK LoginFragment.shouldOverrideUrlLoading 一致：
            // 授权回调由客户端消费，不再放行到官网宣传落地页。
            return false;
        }
        true
    })
    .user_agent(LOGIN_WEBVIEW_USER_AGENT);
    #[cfg(target_os = "android")]
    let login_window = login_window.activity_name("LoginActivity");
    #[cfg(desktop)]
    let login_window = login_window.center();
    let window = login_window
    .initialization_script(js_script)
    .build()
    .map_err(|e| e.to_string())?;
    login_checkpoint(LoginStage::WindowBuildCompleted);

    #[cfg(target_os = "ios")]
    if apply_login_safe_area(&window).is_err() {
        log::warn!("login.safe_area_failed platform=ios");
    }
    let _ = window;
    *app.state::<AppState>().login_session.lock().map_err(|_| "登录状态不可用".to_string())? = Some(session.clone());

    // 监控授权回调；网页上的“已经登录”提示不能代替身份校验。
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_url = None;
        let mut last_callback = None;
        let mut attempts = 0;
        let mut last_attempt: Option<Instant> = None;
        loop {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let Some(win) = app_handle.get_webview_window("login_window") else { break };
            // 新窗口使用独立会话，旧监控任务不能处理新窗口的回调。
            let current = app_handle.state::<AppState>().login_session.lock()
                .ok().and_then(|value| value.clone());
            if !current.as_ref().is_some_and(|current| std::sync::Arc::ptr_eq(current, &session)) {
                break;
            }
            if session.verification.lock().await.completed {
                break;
            }
            let Ok(url) = win.url() else { continue };
            if last_url.as_deref() != Some(url.as_str()) {
                log::info!("login.navigation_changed");
                last_url = Some(url.to_string());
            }
            // 同时兼容包含 ck 但没有授权码的回调。
            if login_callback_kind(&url, &get_app_origin(&app_handle)).is_some() {
                if let Ok(mut captured) = session.callback_url.lock() {
                    if captured.is_none() {
                        *captured = Some(url.to_string());
                    }
                }
            }
            let callback = session.callback_url.lock().ok().and_then(|value| value.clone());
            if callback != last_callback {
                attempts = 0;
                last_attempt = None;
                last_callback = callback.clone();
            }
            let on_landing = url.scheme() == "https"
                && matches!(url.host_str(), Some("www.coolapk.com" | "m.coolapk.com" | "coolapk.com"));
            #[cfg(target_os = "ios")]
            let ios_cookie_poll = callback.is_some() || should_poll_ios_login_cookies(&url);
            #[cfg(not(target_os = "ios"))]
            let ios_cookie_poll = false;
            // iOS 等待会话写入期间持续轮询，账号校验由会话指纹和失败后的冷却时间限频。
            if (callback.is_some() || on_landing || ios_cookie_poll) && (attempts < 4 || ios_cookie_poll)
                && last_attempt.is_none_or(|at| at.elapsed() >= Duration::from_secs(2))
            {
                attempts += 1;
                last_attempt = Some(Instant::now());
                log::info!("login.callback_attempt number={attempts}");
                match verify_login_webview(&app_handle, &win, &session, true).await {
                    Ok(true) => break,
                    _ => log::warn!("login.callback_unverified"),
                }
                if attempts == 4 && !ios_cookie_poll {
                    log::warn!("login.callback_retry_exhausted");
                }
            }
        }
    });

    Ok(())
}
