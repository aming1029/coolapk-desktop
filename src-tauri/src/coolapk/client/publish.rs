use super::*;

pub(super) fn apply_publish_options(form: &mut Vec<(&'static str, String)>, options: &PublishOptions) -> Result<(), String> {
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
pub(super) fn build_publish_upload_files(image: &[u8], name: &str, live_video: Option<&[u8]>, hdr: u32) -> (String, Option<String>) {
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

pub(super) fn build_create_feed_form(
    message: &str,
    pic: Option<&str>,
    post_token: Option<&str>,
) -> Vec<(&'static str, String)> {
    build_create_feed_form_for_type(message, pic, post_token, "feed", "")
}

pub(super) fn build_create_feed_form_for_type(
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
pub(super) fn build_forward_form(message: &str, pic: Option<&str>, forward_id: &str) -> Vec<(&'static str, String)> {
    let mut form = build_create_feed_form(message, pic, None);
    if let Some((_, value)) = form.iter_mut().find(|(name, _)| *name == "forwardid") {
        *value = forward_id.to_string();
    }
    form
}
