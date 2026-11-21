use super::*;

#[tauri::command]
pub async fn get_user_qr_image(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.get_user_qr_image(&uid).await
}

#[tauri::command]
pub async fn send_private_image(
    state: State<'_, AppState>,
    uid: String,
    message_pic: String,
) -> Result<Value, String> {
    state.client.send_private_image(&uid, &message_pic).await
}

#[tauri::command]
pub async fn upload_image(
    state: State<'_, AppState>,
    image_bytes: Vec<u8>,
    file_name: String,
    content_type: String,
    dir: String,
    to_uid: Option<String>,
    live_video_bytes: Option<Vec<u8>>,
    hdr: Option<u32>,
) -> Result<Value, String> {
    state
        .client
        .upload_image_with_live(
            &image_bytes,
            &file_name,
            &content_type,
            &dir,
            to_uid.as_deref(),
            live_video_bytes.as_deref(),
            hdr.unwrap_or(0),
        )
        .await
}

#[tauri::command]
pub async fn get_image_data_url(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    url: String,
    cache_dir: Option<String>,
    cache_ttl_days: Option<u64>,
) -> Result<String, String> {
    let cache_file = image_cache_file(&app, cache_dir.as_deref(), &url)?;
    let ttl_days = cache_ttl_days.unwrap_or(7);

    if let Some(cached) = read_image_cache(&cache_file, ttl_days).await {
        return Ok(cached);
    }

    let data_url = state.client.get_image_data_url(&url).await?;
    // 写缓存失败不能影响图片显示，网络请求成功后始终优先返回图片。
    let _ = write_image_cache(&cache_file, &data_url).await;
    Ok(data_url)
}

/// 通用下载不限制文件格式，完成前仅写临时文件，失败时清理半成品。
#[tauri::command]
pub async fn save_file(state: State<'_, AppState>, app: tauri::AppHandle, url: String, dir: Option<String>, cached_data_url: Option<String>, cache_dir: Option<String>, cache_ttl_days: Option<u64>) -> Result<String, String> {
    use tokio::io::AsyncReadExt;
    let target_dir = user_save_dir(&app, dir.as_deref())?;
    tokio::fs::create_dir_all(&target_dir).await.map_err(|error| format!("创建下载目录失败：{error}"))?;
    // 精确匹配原资源缓存，绝不拿静态缩略图代替动图原文件。
    let cached = if cached_data_url.is_some() { cached_data_url }
        else if url.starts_with("data:") { Some(url.clone()) }
        else if let Ok(cache_file) = image_cache_file(&app, cache_dir.as_deref(), &url) { read_image_cache(&cache_file, cache_ttl_days.unwrap_or(7)).await }
        else { None };
    if let Some(data_url) = cached {
        let (mime_type, bytes) = decode_file_data_url(&data_url)?;
        let file_name = build_download_file_name(&url, "", mime_type, &bytes);
        let target_path = save_image_bytes(&target_dir, &file_name, &bytes).await?;
        return publish_user_file(&app, &target_path).await;
    }
    let sequence = IMAGE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let temp_path = target_dir.join(format!(".coolapk-download-{}-{nonce}-{sequence}.part", std::process::id()));
    let result = async {
        let (mime_type, disposition, final_url) = state.client.download_file_to(&url, &temp_path).await?;
        // 只读取文件头判断格式，大文件下载无需整份载入内存。
        let mut file = tokio::fs::File::open(&temp_path).await.map_err(|error| error.to_string())?;
        let mut header = [0u8; 64];
        let length = file.read(&mut header).await.map_err(|error| error.to_string())?;
        drop(file);
        let file_name = build_download_file_name(&final_url, &disposition, &mime_type, &header[..length]);
        let target_path = {
            let _lock = IMAGE_SAVE_LOCK.lock().map_err(|_| "文件保存锁已损坏".to_string())?;
            let target_path = next_available_file_path(&target_dir, &file_name);
            std::fs::rename(&temp_path, &target_path).map_err(|error| format!("保存下载文件失败：{error}"))?;
            target_path
        };
        publish_user_file(&app, &target_path).await
    }.await;
    if result.is_err() { let _ = tokio::fs::remove_file(&temp_path).await; }
    result
}

#[tauri::command]
pub async fn save_image(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    url: String,
    dir: Option<String>,
) -> Result<String, String> {
    let (file_name, bytes) = if url.starts_with("data:image/") {
        let (mime_type, bytes) = decode_image_data_url(&url)?;
        let file_name = build_generated_image_file_name("coolapk_image", mime_type);
        (file_name, bytes)
    } else {
        let data_url = state.client.get_image_data_url(&url).await?;
        let (mime_type, bytes) = decode_image_data_url(&data_url)?;
        let file_name = build_image_file_name(&url, mime_type);
        (file_name, bytes)
    };
    let target_dir = user_save_dir(&app, dir.as_deref())?;

    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|error| format!("创建图片保存目录失败：{error}"))?;
    let target_path = save_image_bytes(&target_dir, &file_name, &bytes).await?;

    publish_user_file(&app, &target_path).await
}

/// 保存前端生成的 Base64 分享图，目录为空时使用系统下载目录。
#[tauri::command]
pub async fn save_image_data_url(
    app: tauri::AppHandle,
    data_url: String,
    file_name: String,
    dir: Option<String>,
) -> Result<String, String> {
    if data_url.len() > 64 * 1024 * 1024 {
        return Err("分享图数据过大（超过 64MB）".to_string());
    }
    let (mime_type, bytes) = decode_image_data_url(&data_url)?;
    if bytes.len() > 48 * 1024 * 1024 {
        return Err("分享图文件过大（超过 48MB）".to_string());
    }
    let file_name = build_generated_image_file_name(&file_name, mime_type);
    let target_dir = user_save_dir(&app, dir.as_deref())?;
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|error| format!("创建分享图保存目录失败：{error}"))?;
    let target_path = save_image_bytes(&target_dir, &file_name, &bytes).await?;
    publish_user_file(&app, &target_path).await
}

/// 下载图片到应用缓存后交给系统默认图片查看器，避免把 HTTPS 地址交给浏览器。
#[tauri::command]
pub async fn open_image_in_system_viewer(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    url: String,
    cache_dir: Option<String>,
) -> Result<String, String> {
    let (file_name, bytes) = if url.starts_with("data:image/") {
        let (mime_type, bytes) = decode_image_data_url(&url)?;
        let mut hasher = Md5::new();
        hasher.update(url.as_bytes());
        let file_name = format!(
            "system-{}.{}",
            hex::encode(hasher.finalize()),
            image_extension(mime_type)
        );
        (file_name, bytes)
    } else {
        let data_url = state.client.get_image_data_url(&url).await?;
        let (mime_type, bytes) = decode_image_data_url(&data_url)?;
        let mut hasher = Md5::new();
        hasher.update(url.as_bytes());
        let file_name = format!(
            "system-{}.{}",
            hex::encode(hasher.finalize()),
            image_extension(mime_type)
        );
        (file_name, bytes)
    };
    let target_dir = image_cache_root(&app, cache_dir.as_deref())?;
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|error| format!("创建图片缓存目录失败：{error}"))?;
    let preferred_path = target_dir.join(&file_name);
    let target_path = if preferred_path.is_file() {
        preferred_path
    } else {
        save_image_bytes(&target_dir, &file_name, &bytes).await?
    };
    open_local_path(&app, &target_path).await?;
    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn upload_publish_video(state: State<'_, AppState>, video_bytes: Vec<u8>, file_name: String, cover_bytes: Vec<u8>, duration: u64) -> Result<Value, String> {
    state.client.upload_publish_video(&video_bytes, &file_name, &cover_bytes, duration).await
}
