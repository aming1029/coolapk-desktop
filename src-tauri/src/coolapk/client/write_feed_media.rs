use super::*;

impl CoolapkClient {
    /// 收藏/取消收藏动态（需登录，酷安 v6 写接口使用 GET）
    pub(super) async fn favorite_action(&self, path: &str, id: &str) -> Result<Value, String> {
        wrap_api_data(self.api_get(path, &[("id", id.to_string())]).await?)
    }

    pub async fn favorite_feed(&self, feed_id: &str) -> Result<Value, String> {
        self.favorite_action("/v6/feed/favorite", feed_id).await
    }

    pub async fn unfavorite_feed(&self, feed_id: &str) -> Result<Value, String> {
        self.favorite_action("/v6/feed/unFavorite", feed_id).await
    }

    /// 应用收藏沿用官方 AppViewV8Activity → CollectionSelectActivity：数字应用 ID、type=apk。
    /// 对外保留包名参数，供现有页面和调用方兼容使用。
    pub async fn favorite_apk(&self, package_name: &str) -> Result<Value, String> {
        self.set_apk_favorite(package_name, true).await
    }

    pub async fn unfavorite_apk(&self, package_name: &str) -> Result<Value, String> {
        self.set_apk_favorite(package_name, false).await
    }

    /// 读取全部收藏单及所属状态，保留分页游标，避免遗漏后续分页中的已收藏项目。
    pub(super) async fn get_apk_collections(&self, apk_id: &str) -> Result<Vec<Value>, String> {
        let mut collections = Vec::new();
        let mut first_item = String::new();
        let mut last_item = String::new();
        for page in 1..=100 {
            let mut query = build_collection_list_query("", page, &first_item, &last_item);
            query.extend([("id", apk_id.to_string()), ("type", "apk".to_string())]);
            let response = wrap_api_data(self.api_get("/v6/collection/list", &query).await?)?;
            let rows = response["data"].as_array().ok_or("应用收藏单返回格式不正确")?;
            if rows.is_empty() { return Ok(collections); }
            let cursor = |row: &Value| row.as_object().and_then(|o| get_str_by_keys(o, &["entityId", "id"])).unwrap_or_default();
            if first_item.is_empty() { first_item = cursor(&rows[0]); }
            last_item = cursor(rows.last().unwrap());
            let previous_len = collections.len();
            for row in rows {
                if row["entityType"].as_str() != Some("collection") { continue; }
                if !collections.iter().any(|existing: &Value| existing["id"] == row["id"]) { collections.push(row.clone()); }
            }
            // 默认收藏单可能在各页重复出现；没有新项目时终止，避免无效循环。
            if collections.len() == previous_len { return Ok(collections); }
        }
        Err("应用收藏单分页过多，请稍后重试".to_string())
    }

    /// 收藏单归属才是应用收藏的真实状态；旧 userAction.follow 字段可能仍为零。
    pub async fn get_apk_collection_status(&self, apk_id: &str) -> Result<Value, String> {
        if apk_id.is_empty() || !apk_id.bytes().all(|ch| ch.is_ascii_digit()) || apk_id == "0" { return Err("应用收藏状态需要有效数字 ID".to_string()); }
        Ok(json!({ "code": 200, "data": self.get_apk_collections(apk_id).await? }))
    }

    pub(super) async fn set_apk_favorite(&self, package_name: &str, favorited: bool) -> Result<Value, String> {
        if package_name.trim().is_empty() { return Err("应用包名不能为空".to_string()); }
        let detail = self.get_app_detail(package_name.trim()).await?;
        let apk_id = detail["data"].as_object().and_then(|o| get_str_by_keys(o, &["id", "apkId", "aid"])).filter(|id| id.bytes().all(|ch| ch.is_ascii_digit()) && id != "0").ok_or("应用详情缺少有效数字 ID，无法操作收藏")?;
        let collections = self.get_apk_collections(&apk_id).await?;
        match apk_collection_update_plan(&collections, favorited)? {
            Some((ids, cancel_ids)) => self.update_collection_item(&ids, &cancel_ids, &apk_id, "apk", "").await,
            None => Ok(json!({ "code": 200, "data": { "collect": if favorited { 1 } else { 0 } } })),
        }
    }

    /// APK 个人主页置顶：member 节点，不使用编辑推荐置顶接口。
    pub async fn set_user_feed_top(&self, uid: &str, feed_id: &str, top: bool) -> Result<Value, String> {
        let path = if top { "/v6/feed/addTopToNode" } else { "/v6/feed/cancelTopFromNode" };
        wrap_api_data(self.api_post(path, &[], &[
            ("nodeType", "member".to_string()),
            ("nodeId", uid.to_string()),
            ("feedId", feed_id.to_string()),
        ]).await?)
    }

    /// 删除自己发布的动态（需登录）
    /// 实测：必须 POST + id 放 URL query + X-Requested-With: XMLHttpRequest
    /// （GET 返回 status=-1"请求方式错误"）
    pub async fn delete_feed(&self, feed_id: &str) -> Result<Value, String> {
        self.delete_action("/v6/feed/deleteFeed", feed_id).await
    }

    /// 删除自己的评论/回复（需登录）
    /// 实测：必须 POST + id 放 URL query + X-Requested-With: XMLHttpRequest
    pub async fn delete_reply(&self, reply_id: &str) -> Result<Value, String> {
        self.delete_action("/v6/feed/deleteReply", reply_id).await
    }

    /// 删除类接口通用实现：POST + query + XMLHttpRequest（与实测成功组合一致）
    pub(super) async fn delete_action(&self, path: &str, id: &str) -> Result<Value, String> {
        let token = self.get_token()?;
        let url = format!("https://api.coolapk.com{path}?id={}", id);
        let mut request = self.apply_device_profile(
            self.api_client()?
                .request(reqwest::Method::POST, url)
                .header("X-App-Token", token)
                .header("X-Requested-With", "XMLHttpRequest"),
        )?;

        let cookie = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .clone();
        if let Some(cookie) = cookie {
            let full_cookie = self.cookie_for_path(&cookie, path);
            if let Ok(header_val) = reqwest::header::HeaderValue::from_str(&full_cookie) {
                request = request.header(COOKIE, header_val);
            }
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        wrap_api_data(response_json(response).await?)
    }

    /// 上传图片（发动态/发私信配图），返回图片 URL（需登录）
    /// 旧接口 /v6/feed/uploadImage 已被酷安服务端下线（旧版本不再支持图片上传），
    /// 改走新版 OSS 直传链路：ossUploadPrepare 获取凭证 → 直传阿里云 OSS → 返回图片地址。
    /// to_uid：私信场景需传对方 uid（dir=message），发动态（dir=feed）可不传。
    /// 官方先向酷安申请腾讯 UGC 签名，再上传普通视频及封面。
    pub async fn upload_publish_video(&self, video: &[u8], name: &str, cover: &[u8], duration: u64) -> Result<Value, String> {
        let response = wrap_api_data(self.api_post("/v6/upload/TXUgcUploadPrepare", &[], &[]).await?)?;
        let signature = response.get("data").and_then(Value::as_str).filter(|value| !value.is_empty()).ok_or("未获得视频上传签名")?;
        crate::coolapk::video_upload::upload(signature, video, name, cover, duration).await
    }

    pub async fn upload_image(
        &self,
        image_bytes: &[u8],
        file_name: &str,
        content_type: &str,
        dir: &str,
        to_uid: Option<&str>,
    ) -> Result<Value, String> {
        self.upload_image_with_live(image_bytes, file_name, content_type, dir, to_uid, None, 0).await
    }

    /// 实况照片按官方文件清单同时登记封面和视频，HDR 标记由调用方传入。
    pub async fn upload_image_with_live(&self, image_bytes: &[u8], file_name: &str, content_type: &str, dir: &str, to_uid: Option<&str>, live_video: Option<&[u8]>, hdr: u32) -> Result<Value, String> {
        if ![0, 1].contains(&hdr) { return Err("HDR 标记无效".to_string()); }
        if let Some(video) = live_video {
            if video.len() < 12 || &video[4..8] != b"ftyp" { return Err("实况视频必须是标准 MP4 文件".to_string()); }
        }
        let my_uid = self
            .user_cookie
            .read()
            .ok()
            .and_then(|g| g.clone())
            .and_then(|c| {
                c.split(';').find_map(|kv| {
                    let mut parts = kv.trim().splitn(2, '=');
                    match (parts.next(), parts.next()) {
                        (Some("uid"), Some(v)) => Some(v.trim().to_string()),
                        _ => None,
                    }
                })
            })
            .ok_or_else(|| "未登录，无法上传图片".to_string())?;
        let target_uid = match to_uid {
            Some(u) => u.to_string(),
            None => my_uid,
        };

        let (file_list, video_name) = build_publish_upload_files(image_bytes, file_name, live_video, hdr);

        // 发动态/评论配图用 image/feed，私信图片用 message/message
        let upload_bucket = if matches!(dir, "feed" | "feed_report" | "feed_emoji_picture") { "image" } else { dir }.to_string();
        let feed_type = if dir == "feed" { "feed" } else { "" }.to_string();

        let prepare_params = [
            ("uploadBucket", upload_bucket),
            ("uploadDir", dir.to_string()),
            ("is_anonymous", "0".to_string()),
            ("uploadFileList", file_list),
            ("toUid", target_uid),
            ("feed_type", feed_type),
        ];

        let prepare_json = self
            .api_post("/v6/upload/ossUploadPrepare", &[], &prepare_params)
            .await?;

        if let Some(msg) = prepare_json
            .get("message")
            .or_else(|| prepare_json.get("error"))
            .and_then(Value::as_str)
        {
            if !msg.is_empty()
                && (prepare_json.get("data").is_none()
                    || prepare_json.get("data") == Some(&Value::Null))
            {
                return Err(format!("上传凭证获取失败（{msg}）"));
            }
        }

        let data = prepare_json
            .get("data")
            .filter(|d| !d.is_null())
            .ok_or_else(|| {
                let msg = prepare_json
                    .get("message")
                    .or_else(|| prepare_json.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("服务端未返回凭证数据");
                format!("上传凭证获取失败（{msg}）")
            })?;

        let file_info = data
            .get("fileInfo")
            .and_then(|f| f.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| {
                let msg = prepare_json
                    .get("message")
                    .or_else(|| prepare_json.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("fileInfo 缺失");
                format!("上传凭证获取失败（{msg}）")
            })?;
        let prepare_info = data
            .get("uploadPrepareInfo")
            .ok_or_else(|| {
                let msg = prepare_json
                    .get("message")
                    .or_else(|| prepare_json.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("uploadPrepareInfo 缺失");
                format!("上传凭证获取失败（{msg}）")
            })?;

        let upload_file_name = file_info
            .get("uploadFileName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let existing_file_url = file_info
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|url| !url.trim().is_empty())
            .map(str::to_string);
        let bucket = prepare_info
            .get("bucket")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        // 官方 APK 用 uploadImagePrefix + uploadFileName 作为提交给 createFeed
        // 的图片地址，而不是使用 OSS 回调正文中的地址。
        let upload_image_prefix = prepare_info
            .get("uploadImagePrefix")
            .and_then(|v| v.as_str())
            .filter(|prefix| !prefix.trim().is_empty())
            // APK 的默认值是 http://image.coolapk.com；服务端的旧图地址也使用该主机。
            .unwrap_or("http://image.coolapk.com")
            .trim_end_matches('/')
            .to_string();
        let end_point = prepare_info
            .get("endPoint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let access_key_id = prepare_info
            .get("accessKeyId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let access_key_secret = prepare_info
            .get("accessKeySecret")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let security_token = prepare_info
            .get("securityToken")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        // 视频无需图片回调；先上传成功才返回实况封面，避免静默退化为静态图片。
        if let (Some(video), Some(name)) = (live_video, video_name.as_ref()) {
            let info = data.get("fileInfo").and_then(Value::as_array).and_then(|files| files.iter().find(|file| file.get("name").and_then(Value::as_str) == Some(name.as_str()))).ok_or("服务端未返回实况视频凭证")?;
            if !info.get("url").and_then(Value::as_str).is_some_and(|url| !url.is_empty()) {
                let key = info.get("uploadFileName").and_then(Value::as_str).filter(|key| !key.is_empty()).ok_or("实况视频上传路径缺失")?;
                self.put_live_photo_video(video, key, prepare_info).await?;
            }
        }

        // 相同 MD5 的文件可能由服务端直接返回已有地址，官方客户端会跳过直传。
        if let Some(url) = existing_file_url {
            return Ok(json!({ "code": 200, "data": url }));
        }

        if upload_file_name.is_empty()
            || bucket.is_empty()
            || end_point.is_empty()
            || access_key_id.is_empty()
            || access_key_secret.is_empty()
        {
            return Err("上传凭证不完整".to_string());
        }

        // 2. 直传 OSS（PUT Object，OSS V1 签名）
        let content_md5_b64 = {
            use base64::Engine;
            use md5::{Digest, Md5};
            let mut hasher = Md5::new();
            hasher.update(image_bytes);
            base64::engine::general_purpose::STANDARD.encode(hasher.finalize())
        };
        let now = chrono::Utc::now()
            .format("%a, %d %b %Y %H:%M:%S GMT")
            .to_string();

        // 上传成功回调（与官方客户端一致）
        let callback = "eyJjYWxsYmFja0JvZHlUeXBlIjoiYXBwbGljYXRpb25cL2pzb24iLCJjYWxsYmFja0hvc3QiOiJhcGkuY29vbGFway5jb20iLCJjYWxsYmFja1VybCI6Imh0dHBzOlwvXC9hcGkuY29vbGFway5jb21cL3Y2XC9jYWxsYmFja1wvbW9iaWxlT3NzVXBsb2FkU3VjY2Vzc0NhbGxiYWNrP2NoZWNrQXJ0aWNsZUNvdmVyUmVzb2x1dGlvbj0wJnZlcnNpb25Db2RlPTIxMDIwMzEiLCJjYWxsYmFja0JvZHkiOiJ7XCJidWNrZXRcIjoke2J1Y2tldH0sXCJvYmplY3RcIjoke29iamVjdH0sXCJoYXNQcm9jZXNzXCI6JHt4OnZhcjF9fSJ9";
        let callback_var = "eyJ4OnZhcjEiOiJmYWxzZSJ9";

        let resource = format!("/{}/{}", bucket, upload_file_name);
        let string_to_sign = format!(
            "PUT\n{}\n{}\n{}\nx-oss-callback:{}\nx-oss-callback-var:{}\nx-oss-security-token:{}\n{}",
            content_md5_b64, content_type, now, callback, callback_var, security_token, resource
        );

        use base64::Engine;
        use hmac::{Hmac, Mac};
        use sha1::Sha1;
        type HmacSha1 = Hmac<Sha1>;
        let mut mac =
            HmacSha1::new_from_slice(access_key_secret.as_bytes()).map_err(|e| e.to_string())?;
        mac.update(string_to_sign.as_bytes());
        let signature =
            base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        let authorization = format!("OSS {}:{}", access_key_id, signature);

        let oss_host = if end_point.starts_with("http") {
            end_point
        } else {
            format!("https://{}", end_point)
        };
        let oss_host = oss_host.replace("https://", "").replace("http://", "");
        let oss_url = format!("https://{}.{}/{}", bucket, oss_host, upload_file_name);

        let mut oss_request = self
            .api_client()?
            .request(reqwest::Method::PUT, &oss_url)
            .header("Authorization", &authorization)
            .header("Content-MD5", &content_md5_b64)
            .header("Content-Type", content_type)
            .header("Date", &now)
            .header("x-oss-callback", callback)
            .header("x-oss-callback-var", callback_var)
            .header("x-oss-security-token", &security_token)
            .body(image_bytes.to_vec());

        let _ = &mut oss_request;

        let oss_res = oss_request.send().await.map_err(|e| e.to_string())?;
        let oss_status = oss_res.status();
        let oss_body = oss_res.text().await.unwrap_or_default();

        if !oss_status.is_success() {
            return Err(format!("OSS 直传失败 (HTTP {}): {}", oss_status, &oss_body));
        }

        // 3. 按官方客户端的方式生成最终图片地址。OSS 回调只负责让酷安
        // 服务端登记文件，回调 JSON 不是 createFeed 的 pic 字段。
        if let Some(image_url) = build_oss_image_url(&upload_image_prefix, &upload_file_name) {
            return Ok(json!({ "code": 200, "data": image_url }));
        }
        Err(format!("OSS 直传响应异常: {}", &oss_body))
    }

    /// 视频使用与封面同一组 OSS 凭证，签名中不包含图片专用回调。
    pub(super) async fn put_live_photo_video(&self, bytes: &[u8], key: &str, credentials: &Value) -> Result<(), String> {
        use base64::Engine;
        use md5::{Digest, Md5};
        use hmac::{Hmac, Mac};
        use sha1::Sha1;
        let field = |name: &str| credentials.get(name).and_then(Value::as_str).filter(|value| !value.is_empty()).ok_or_else(|| format!("上传凭证缺少 {name}"));
        let bucket = field("bucket")?;
        let endpoint = field("endPoint")?;
        let id = field("accessKeyId")?;
        let secret = field("accessKeySecret")?;
        let token = field("securityToken")?;
        let now = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let digest = BASE64.encode(Md5::digest(bytes));
        let resource = format!("/{bucket}/{key}");
        let content_type = if bytes.get(8..12) == Some(b"qt  ") { "video/quicktime" } else { "video/mp4" };
        let canonical = format!("PUT\n{digest}\n{content_type}\n{now}\nx-oss-security-token:{token}\n{resource}");
        let mut mac = Hmac::<Sha1>::new_from_slice(secret.as_bytes()).map_err(|error| error.to_string())?;
        mac.update(canonical.as_bytes());
        let signature = BASE64.encode(mac.finalize().into_bytes());
        let host = endpoint.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
        let url = format!("https://{bucket}.{host}/{key}");
        let response = self.api_client()?.put(url).header("Authorization", format!("OSS {id}:{signature}")).header("Content-MD5", digest).header("Content-Type", content_type).header("Date", now).header("x-oss-security-token", token).body(bytes.to_vec()).send().await.map_err(|error| error.to_string())?;
        if !response.status().is_success() { return Err(format!("实况视频上传失败（HTTP {}）", response.status())); }
        Ok(())
    }
}
