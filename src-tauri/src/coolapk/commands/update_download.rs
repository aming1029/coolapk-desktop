use super::*;

#[tauri::command]
pub async fn update_home_tab_config(
    state: State<'_, AppState>,
    config_json: String,
) -> Result<Value, String> {
    state.client.update_home_tab_config(&config_json).await
}

#[tauri::command]
pub async fn update_collection_item(
    state: State<'_, AppState>,
    collection_ids: String,
    cancel_ids: String,
    target_id: String,
    feed_type: String,
    trace: String,
) -> Result<Value, String> {
    state
        .client
        .update_collection_item(
            &collection_ids,
            &cancel_ids,
            &target_id,
            &feed_type,
            &trace,
        )
        .await
}

#[tauri::command]
pub async fn update_collection(
    state: State<'_, AppState>,
    id: String,
    title: String,
    description: String,
    cover: String,
    is_open: i32,
) -> Result<Value, String> {
    state
        .client
        .update_collection(&id, &title, &description, &cover, is_open)
        .await
}

#[tauri::command]
pub async fn update_my_card_config(state: State<'_, AppState>, config_json: String) -> Result<Value, String> {
    state.client.update_my_card_config(&config_json).await
}

#[tauri::command]
pub async fn update_feed(
    state: State<'_, AppState>,
    feed_id: String,
    message: String,
    pic: String,
    post_token: Option<String>,
    options: Option<crate::coolapk::client::PublishOptions>,
) -> Result<Value, String> {
    state.client.update_feed(&feed_id, &message, &pic, post_token.as_deref(), options.as_ref()).await
}

#[tauri::command]
pub async fn update_user_profile(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<Value, String> {
    state.client.update_user_profile(&key, &value).await
}

#[tauri::command]
pub async fn update_user_cover(
    state: State<'_, AppState>,
    url: String,
) -> Result<Value, String> {
    state.client.update_user_cover(&url).await
}

#[tauri::command]
pub async fn start_apk_download(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    task_id: String,
    package_name: String,
    apk_name: String,
    apk_id: Option<String>,
    version_code: Option<String>,
    file_name: String,
    dir: Option<String>,
    target_path: Option<String>,
    extra_analysis_data: Option<String>,
) -> Result<Value, String> {
    if task_id.trim().is_empty() || package_name.trim().is_empty() {
        return Err("下载任务参数不完整".to_string());
    }
    let mut control = state.downloads.register(&task_id)?;
    let result = run_apk_download(
        &app,
        &state.client,
        &mut control,
        &task_id,
        &package_name,
        if apk_name.trim().is_empty() { &package_name } else { &apk_name },
        apk_id.as_deref(),
        version_code.as_deref(),
        &file_name,
        dir.as_deref(),
        target_path.as_deref(),
        extra_analysis_data.as_deref(),
    )
    .await;
    state.downloads.finish(&task_id);
    if let Err(error) = &result {
        let _ = app.emit(
            "apk-download-progress",
            json!({ "taskId": task_id, "status": "failed", "error": error }),
        );
    }
    result
}

#[tauri::command]
pub fn pause_apk_download(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    state.downloads.request(&task_id, DownloadControl::Pause)
}

#[tauri::command]
pub fn cancel_apk_download(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    state.downloads.request(&task_id, DownloadControl::Cancel)
}

#[tauri::command]
pub async fn delete_apk_download_file(
    app: tauri::AppHandle,
    target_path: Option<String>,
    partial_path: Option<String>,
    dir: Option<String>,
    public_path: Option<String>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    if let Some(location) = public_path.filter(|value| !value.is_empty()) {
        let result = call_android_update_method(&app, "deletePublishedApk", location).await?;
        if result != "deleted" { return Err(result.trim_start_matches("error:").to_string()); }
    }
    #[cfg(not(target_os = "android"))]
    let _ = public_path;
    let target_dir = user_save_dir(&app, dir.as_deref())?;
    for raw_path in [target_path, partial_path]
        .into_iter()
        .flatten()
        .filter(|path| !path.trim().is_empty())
    {
        let path = validate_download_path_for_file_operation(&raw_path)?;
        if !path_is_direct_child_of(&path, &target_dir)? {
            return Err("下载文件必须位于当前下载目录中".to_string());
        }
        reject_download_symlink(&path)?;
        if path.is_file() {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|error| format!("删除下载文件失败：{error}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn install_apk_download(app: tauri::AppHandle, path: String) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        let file = std::fs::canonicalize(&path).map_err(|_| "安装包不存在，请重新下载".to_string())?;
        let directory = user_save_dir(&app, None)?.canonicalize().map_err(|error| error.to_string())?;
        if file.parent() != Some(directory.as_path()) || !file.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("apk")) {
            return Err("只能安装应用下载目录内的 APK；拆分安装包请使用对应安装工具".to_string());
        }
        let result = call_android_update_method(&app, "installDownloadedApk", file.to_string_lossy().to_string()).await?;
        match result.as_str() {
            "started" | "permission_required" => Ok(result),
            _ => Err(result.trim_start_matches("error:").to_string()),
        }
    }
    #[cfg(not(target_os = "android"))]
    { let _ = (app, path); Err("此安装入口仅支持 Android".to_string()) }
}

#[tauri::command]
pub async fn open_apk_download_directory(
    app: tauri::AppHandle,
    dir: Option<String>,
) -> Result<(), String> {
    let target_dir = user_save_dir(&app, dir.as_deref())?;
    std::fs::create_dir_all(&target_dir).map_err(|error| format!("创建下载目录失败：{error}"))?;
    open_local_path(&app, &target_dir).await?;
    Ok(())
}

#[tauri::command]
pub async fn update_user_remark(
    state: State<'_, AppState>,
    uid: String,
    name: String,
) -> Result<Value, String> {
    state.client.update_user_remark(&uid, &name).await
}

#[tauri::command]
pub fn update_device_profile(
    state: State<'_, AppState>,
    profile: DeviceProfile,
) -> Result<Value, String> {
    state.client.update_device_profile(profile);
    Ok(json!({ "code": 200, "data": true }))
}

/// 返回当前平台实际使用的下载目录，便于设置页展示真实路径。
#[tauri::command]
pub fn get_download_directory(app: tauri::AppHandle, dir: Option<String>) -> Result<String, String> {
    Ok(user_save_dir(&app, dir.as_deref())?.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn download_update(
    app: tauri::AppHandle,
    url: String,
    speed_limit_kbps: Option<u64>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tokio::io::AsyncWriteExt;

    let parsed_url = reqwest::Url::parse(&url).map_err(|e| format!("更新链接无效: {e}"))?;
    if parsed_url.scheme() != "https" {
        return Err("更新链接必须为 HTTPS".to_string());
    }
    let host = parsed_url
        .host_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !UPDATE_ALLOWED_HOSTS.contains(&host.as_str()) {
        return Err(format!("更新链接域名不在允许列表内: {host}"));
    }

    let dir = update_cache_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // 文件名净化：只保留安全字符，防路径穿越，并按平台限制扩展名。
    let raw_name = url.rsplit('/').next().unwrap_or("").trim();
    let safe_name: String = raw_name
        .chars()
        .take(128)
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    if safe_name.is_empty() || !is_update_package_extension(Path::new(&safe_name)) {
        return Err("更新包文件名不合法".to_string());
    }
    // 每次下载使用独立文件名，避免旧任务或另一个应用实例仍持有同名安装包时互相锁定。
    let extension = safe_name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    let stem = safe_name
        .get(..safe_name.len().saturating_sub(extension.len() + 1))
        .filter(|value| !value.is_empty())
        .unwrap_or("coolapk-desktop-update");
    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let unique_name = format!("{stem}-{}-{nonce}.{extension}", std::process::id());
    let path = dir.join(unique_name);
    let partial_path = path.with_extension(format!("{extension}.part"));

    let client = session(&url, SessionPolicy::Updater)?;
    let mut response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("下载失败：HTTP {}", response.status()));
    }

    let total = response.content_length().unwrap_or(0);
    if total > UPDATE_MAX_BYTES {
        return Err("更新包体积异常（超过 500MB），已拒绝下载".to_string());
    }
    let mut file = tokio::fs::File::create(&partial_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    // 限速：按 1 秒滑动窗口累积字节数，超出配额后补眠
    let limit_bytes_per_sec = speed_limit_kbps.unwrap_or(0).saturating_mul(1024);
    let mut window_bytes: u64 = 0;
    let mut window_start = tokio::time::Instant::now();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        downloaded += chunk.len() as u64;
        if downloaded > UPDATE_MAX_BYTES {
            drop(file);
            let _ = tokio::fs::remove_file(&partial_path).await;
            return Err("更新包体积异常（超过 500MB），已中止下载".to_string());
        }
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        if limit_bytes_per_sec > 0 {
            window_bytes += chunk.len() as u64;
            let elapsed = window_start.elapsed().as_secs_f64();
            if elapsed >= 1.0 {
                window_bytes = 0;
                window_start = tokio::time::Instant::now();
            } else {
                let budget = window_bytes as f64 / limit_bytes_per_sec as f64;
                if budget > elapsed {
                    tokio::time::sleep(std::time::Duration::from_secs_f64(budget - elapsed)).await;
                }
            }
        }
        if total > 0 {
            let _ = app.emit(
                "update-download-progress",
                serde_json::json!({ "downloaded": downloaded, "total": total }),
            );
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    file.sync_all().await.map_err(|e| e.to_string())?;
    // Windows 下启动安装程序前必须释放下载文件句柄，否则 CreateProcess 可能返回 os error 32。
    drop(file);
    // 服务端声明了文件大小时校验完整性，避免保存半成品安装包
    if total > 0 && downloaded != total {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(format!("下载中断：已下载 {downloaded}/{total} 字节"));
    }
    tokio::fs::rename(&partial_path, &path)
        .await
        .map_err(|e| format!("保存更新包失败：{e}"))?;
    #[cfg(target_os = "android")]
    {
        let published = call_android_update_method(
            &app,
            "publishUpdateApk",
            path.to_string_lossy().to_string(),
        ).await;
        // 下载已完成；导出到公共下载目录失败时保留私有 APK，仍可通过 FileProvider 安装。
        match published {
            Ok(location) if location.starts_with("content://") => {
                let _ = tokio::fs::remove_file(&path).await;
                return Ok(location);
            }
            Ok(status) if status == "private_fallback" => {}
            Ok(status) => {
                log::warn!("APK 已下载，导出到下载目录失败，使用私有更新包：{status}");
            }
            Err(error) => {
                log::warn!("APK 已下载，调用下载目录导出失败，使用私有更新包：{error}");
            }
        }
    }
    Ok(path.to_string_lossy().to_string())
}

/// 启动时校验待安装包是否仍存在且位于应用更新目录内。
#[tauri::command]
pub async fn is_update_package_available(app: tauri::AppHandle, installer_path: String) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    if installer_path.starts_with("content://") {
        return Ok(call_android_update_method(&app, "isUpdatePackageAvailable", installer_path).await? == "available");
    }
    let expected_dir = update_cache_dir(&app)?;
    let expected_dir = match expected_dir.canonicalize() {
        Ok(path) => path,
        Err(_) => return Ok(false),
    };
    let canonical = match std::fs::canonicalize(installer_path) {
        Ok(path) => path,
        Err(_) => return Ok(false),
    };
    if !canonical.starts_with(&expected_dir) || !is_update_package_extension(&canonical) {
        return Ok(false);
    }
    let metadata = match std::fs::metadata(canonical) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(false),
    };
    Ok(metadata.is_file() && metadata.len() > 0)
}

/// 清理没有被待安装记录引用的旧安装包和未完成下载文件。
/// 更新包不属于普通图片/WebView缓存，不能由 clear_app_cache 直接删除。
#[tauri::command]
pub fn cleanup_update_packages(app: tauri::AppHandle, keep_path: Option<String>) -> Result<(), String> {
    let update_dir = update_cache_dir(&app)?;
    if !update_dir.exists() {
        return Ok(());
    }
    let canonical_keep = keep_path.and_then(|path| {
        let canonical = std::fs::canonicalize(path).ok()?;
        let expected_dir = update_dir.canonicalize().ok()?;
        if canonical.starts_with(expected_dir) && is_update_package_extension(&canonical) {
            Some(canonical)
        } else {
            None
        }
    });
    for entry in std::fs::read_dir(&update_dir)
        .map_err(|error| format!("读取更新目录失败：{error}"))?
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_download_artifact = is_update_package_extension(&path)
            || path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("part"));
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        if is_download_artifact && canonical_keep.as_ref() != Some(&canonical_path) {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

/// 返回当前 Windows 发行方式。安装版目录带有 NSIS 的 uninstall.exe；否则视为单文件便携版。
#[tauri::command]
pub fn get_update_distribution() -> String {
    #[cfg(target_os = "windows")]
    {
        let is_installed = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("uninstall.exe")))
            .is_some_and(|path| path.is_file());
        if is_installed {
            "installer".to_string()
        } else {
            "portable".to_string()
        }
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        crate::coolapk::desktop_update::distribution().to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "installer".to_string()
    }
}

/// 按当前发行方式启动更新，保留各平台的安装和授权流程。
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle, installer_path: String, portable: bool) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        install_update_windows(&app, installer_path, portable).map(|_| "started".to_string())
    }
    #[cfg(target_os = "android")]
    {
        if portable {
            return Err("Android 更新包必须是 APK".to_string());
        }
        install_update_android(app, installer_path).await
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        tauri::async_runtime::spawn_blocking(move || {
            crate::coolapk::desktop_update::install(&app, &installer_path, portable)
        }).await.map_err(|error| error.to_string())?.map(|_| "started".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "android", target_os = "macos", target_os = "linux")))]
    {
        let _ = app;
        let _ = installer_path;
        let _ = portable;
        Err("当前平台暂不支持应用内自动安装，请前往发布页面手动下载安装".to_string())
    }
}

#[tauri::command]
pub fn take_update_install_error(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let path = update_cache_dir(&app)?.join("install-error.txt");
    match std::fs::read_to_string(&path) {
        Ok(error) => {
            let _ = std::fs::remove_file(path);
            Ok(Some(error))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn get_update_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_update_list(page).await
}

#[tauri::command]
pub async fn get_download_version_list(
    state: State<'_, AppState>,
    package_name: String,
) -> Result<Value, String> {
    state.client.get_download_version_list(&package_name).await
}
