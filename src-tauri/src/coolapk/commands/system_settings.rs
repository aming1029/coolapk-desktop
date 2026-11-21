use super::*;

/// 为包含 HTML 与图片资源的导出包创建独立目录。
#[tauri::command]
pub fn create_export_directory(
    app: tauri::AppHandle,
    directory_name: String,
    dir: Option<String>,
) -> Result<String, String> {
    let safe_name: String = directory_name
        .chars()
        .take(100)
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .collect();
    if safe_name.is_empty() || safe_name == "." || safe_name == ".." {
        return Err("导出目录名不合法".to_string());
    }

    let parent = user_save_dir(&app, dir.as_deref())?;
    std::fs::create_dir_all(&parent).map_err(|e| format!("创建导出目录失败：{e}"))?;
    let mut target = parent.join(&safe_name);
    if target.exists() {
        for index in 2..=9999 {
            let candidate = parent.join(format!("{safe_name}_{index}"));
            if !candidate.exists() {
                target = candidate;
                break;
            }
        }
    }
    std::fs::create_dir(&target).map_err(|e| format!("创建导出目录失败：{e}"))?;
    Ok(target.to_string_lossy().to_string())
}

/// 将文本内容以 JSON 形式导出到指定目录（dir 为空时使用系统下载目录），返回完整保存路径
#[tauri::command]
pub async fn export_json_file(
    app: tauri::AppHandle,
    file_name: String,
    content: String,
    dir: Option<String>,
) -> Result<String, String> {
    // 文件名净化：只保留安全字符，拒绝 .. 路径穿越与空名，并限制长度与内容体积
    let safe_name: String = file_name
        .chars()
        .take(100)
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    if safe_name.is_empty() || safe_name == "." || safe_name == ".." || safe_name.contains("..") {
        return Err("导出文件名不合法".to_string());
    }
    // 保留较高上限以兼容大型 JSON/TXT/HTML 文本导出；HTML 图片资源单独保存。
    if content.len() > 256 * 1024 * 1024 {
        return Err("导出内容过大（超过 256MB）".to_string());
    }

    let dir = user_save_dir(&app, dir.as_deref())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建导出目录失败：{e}"))?;
    let path = next_available_file_path(&dir, &safe_name);
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    publish_user_file(&app, &path).await
}

/// 统计应用自己管理的图片缓存和更新包临时文件，并返回实际图片缓存目录。
#[tauri::command]
pub fn get_cache_info(
    app: tauri::AppHandle,
    cache_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let (image, update) = cache_locations(&app, cache_dir.as_deref())?;
    let _ = std::fs::create_dir_all(&image);
    let image_bytes = dir_total_size(&image);
    let update_bytes = dir_total_size(&update);
    Ok(serde_json::json!({
        "bytes": image_bytes + update_bytes,
        "imageBytes": image_bytes,
        "webviewBytes": 0,
        "updateBytes": update_bytes,
        "path": image.to_string_lossy(),
    }))
}

/// 只删除应用自己管理的图片缓存，不触碰 WebView profile 和更新包。
#[tauri::command]
pub fn clear_app_cache(
    app: tauri::AppHandle,
    cache_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let (image, _update) = cache_locations(&app, cache_dir.as_deref())?;
    let _ = std::fs::remove_dir_all(&image);
    let _ = std::fs::create_dir_all(&image);
    // 更新包由独立的待安装流程管理，清理普通缓存时必须保留，
    // 否则用户下载后暂不安装，重启或手动清理缓存就会丢失安装包。
    get_cache_info(app, cache_dir)
}

/// 删除超过设置天数的原生图片缓存，启动和修改过期时间时调用。
#[tauri::command]
pub fn clean_expired_cache(
    app: tauri::AppHandle,
    cache_dir: Option<String>,
    cache_ttl_days: u64,
) -> Result<serde_json::Value, String> {
    let image = image_cache_root(&app, cache_dir.as_deref())?;
    if cache_ttl_days > 0 {
        let max_age = Duration::from_secs(cache_ttl_days.saturating_mul(24 * 60 * 60));
        if let Ok(entries) = std::fs::read_dir(&image) {
            for entry in entries.flatten() {
                let path = entry.path();
                let expired = entry
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                    .is_some_and(|age| age > max_age);
                if expired && path.is_file() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
    get_cache_info(app, cache_dir)
}

/// 打开当前图片缓存目录，方便用户查看实际落盘文件。
#[tauri::command]
pub async fn open_cache_directory(
    app: tauri::AppHandle,
    cache_dir: Option<String>,
) -> Result<String, String> {
    let image = image_cache_root(&app, cache_dir.as_deref())?;
    std::fs::create_dir_all(&image).map_err(|e| format!("创建缓存目录失败：{e}"))?;
    open_local_path(&app, &image).await?;
    Ok(image.to_string_lossy().to_string())
}

/// 退出整个应用（用于更新前关闭窗口）
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    crate::persist_current_window_geometry(&app);
    app.exit(0);
}
