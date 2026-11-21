use super::*;

impl CoolapkClient {
    pub async fn get_feed_detail(&self, feed_id: &str, post_token: Option<&str>, post_token_field: Option<&str>) -> Result<Value, String> {
        let mut query = vec![("id", feed_id.to_string())];
        if let Some(token) = post_token.filter(|value| !value.trim().is_empty()) {
            append_comment_captcha_token(&mut query, Some(token), post_token_field);
        }
        let validation = post_token.filter(|_| post_token_field.is_some_and(|field| field.trim().is_empty()));
        let primary_error = match self.request_api_from_with_validation("https://api.coolapk.com", Method::GET, "/v6/feed/detail", &query, None, validation).await {
            Ok(value) if matches!(application_failure(&value), Some((403, "captcha"))) => return Err(value.to_string()),
            Ok(value) => match wrap_api_data(value) {
                Ok(detail) => return Ok(detail),
                Err(error) => error,
            },
            Err(error) => error,
        };

        // 动态属于公开内容：登录请求不可用时改用独立游客设备码，并且明确不携带账号 Cookie。
        let mut public_errors = Vec::new();
        for api_origin in ["https://api.coolapk.com", "https://api2.coolapk.com"] {
            match self
                .public_api_get_from(api_origin, "/v6/feed/detail", &query)
                .await
            {
                Ok(value) if matches!(application_failure(&value), Some((403, "captcha"))) => return Err(value.to_string()),
                Ok(value) => match wrap_api_data(value) {
                    Ok(detail) => return Ok(detail),
                    Err(error) => public_errors.push(format!("{api_origin}: {error}")),
                },
                Err(error) => public_errors.push(format!("{api_origin}: {error}")),
            }
        }

        Err(format!(
            "动态详情加载失败：{primary_error}；游客接口：{}",
            public_errors.join("；")
        ))
    }

    /// 后台索引与导出使用游客设备读取公开动态，避免将未打开的帖子记入当前账号的浏览历史。
    pub async fn get_public_feed_detail(&self, feed_id: &str) -> Result<Value, String> {
        let query = [("id", feed_id.to_string())];
        let mut errors = Vec::new();
        for api_origin in ["https://api.coolapk.com", "https://api2.coolapk.com"] {
            match self.public_api_get_from(api_origin, "/v6/feed/detail", &query).await {
                Ok(value) => match wrap_api_data(value) {
                    Ok(detail) => return Ok(detail),
                    Err(error) => errors.push(format!("{api_origin}: {error}")),
                },
                Err(error) => errors.push(format!("{api_origin}: {error}")),
            }
        }
        Err(format!("公开动态详情加载失败：{}", errors.join("；")))
    }

    /// APK 二次编辑先读取 changeDetail，服务端在这里返回可编辑状态和原始内容。
    pub async fn get_editable_feed(&self, feed_id: &str) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/feed/changeDetail", &[("id", feed_id.to_string()), ("rid", String::new()), ("noticeId", String::new()), ("fromApi", String::new())]).await?)
    }

    /// 与 APK 的 FeedMultiPart 一样保留原动态字段；图文标题和题图由正文模型单独提交。
    pub async fn update_feed(&self, feed_id: &str, message: &str, pic: &str, post_token: Option<&str>, options: Option<&PublishOptions>) -> Result<Value, String> {
        let detail = self.get_editable_feed(feed_id).await?;
        let original = detail.get("data").ok_or_else(|| "获取可编辑动态失败".to_string())?;
        let original_id = original.get("id").map(value_to_string).unwrap_or_default();
        if original_id != feed_id { return Err("可编辑动态 ID 不匹配".to_string()); }
        if let Some(allowed) = original.get("enableModify").or_else(|| original.get("enable_modify")) {
            if parse_u64_val(allowed) != Some(1) { return Err("此动态当前不允许编辑或编辑次数已用尽".to_string()); }
        }
        let feed_type = original.get("feedType").or_else(|| original.get("feed_type")).and_then(Value::as_str).unwrap_or("feed");
        let original_obj = original.as_object().ok_or_else(|| "动态格式无效".to_string())?;
        let original_article_type = get_u64_by_keys(original_obj, &["isHtmlArticle", "is_html_article"]);
        if feed_type != "feed" && original_article_type != 1 { return Err("目前只支持重新编辑普通动态和图文".to_string()); }
        if get_u64_by_keys(original_obj, &["mediaType", "media_type"]) > 0 || original.get("mediaUrl").or_else(|| original.get("media_url")).and_then(Value::as_str).is_some_and(|value| !value.is_empty()) { return Err("暂不支持重新编辑视频动态".to_string()); }
        let is_article_update = options.is_some_and(|value| value.html_article == Some(true));
        if (original_article_type == 1) != is_article_update { return Err("重新编辑时不能改变动态类型".to_string()); }
        if original_article_type != 0 && original_article_type != 1 { return Err("暂不支持重新编辑大封面动态".to_string()); }
        if is_article_update {
            let article_options = options.expect("article update options checked above");
            if pic.trim().len() > 0 { return Err("图文正文图片必须保存在正文模型中".to_string()); }
            if !article_options.message_title.as_deref().is_some_and(|title| !title.trim().is_empty()) { return Err("图文标题不能为空".to_string()); }
            let models: Value = serde_json::from_str(message).map_err(|_| "图文正文格式无效".to_string())?;
            let models = models.as_array().ok_or_else(|| "图文正文格式无效".to_string())?;
            let mut body_text_length = 0usize;
            let mut has_text = false;
            for model in models {
                match model.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        let text = model.get("message").and_then(Value::as_str).ok_or_else(|| "图文文字段落格式无效".to_string())?;
                        has_text |= !text.trim().is_empty();
                        body_text_length += text.chars().count();
                    }
                    Some("image") => {
                        if !model.get("url").and_then(Value::as_str).is_some_and(|url| !url.trim().is_empty()) { return Err("图文正文图片地址缺失".to_string()); }
                    }
                    Some("card") | Some("shareUrl") | Some("else") => {},
                    _ => return Err("图文包含不支持的内容模型，为避免保存时丢失，已停止修改".to_string()),
                }
            }
            if !has_text { return Err("图文正文不能为空".to_string()); }
            if body_text_length > 12_000 { return Err("图文正文超过 12000 字".to_string()); }
        } else {
            if message.trim().is_empty() && pic.trim().is_empty() { return Err("动态内容不能为空".to_string()); }
            if message.chars().count() > 1000 { return Err("动态内容超过 1000 字".to_string()); }
        }

        let mut form = build_create_feed_form(message, Some(pic), None);
        for (key, value) in &mut form {
            if *key == "id" { *value = feed_id.to_string(); continue; }
            let source_keys: &[&str] = match *key {
                "publish_status" => &["publish_status", "publishStatus"], "location" => &["location"], "long_location" => &["long_location", "longLocation"],
                "latitude" => &["latitude"], "longitude" => &["longitude"], "media_url" => &["media_url", "mediaUrl"], "media_type" => &["media_type", "mediaType"],
                "media_pic" => &["media_pic", "mediaPic"], "message_title" => &["message_title", "messageTitle"], "message_brief" => &["message_brief", "messageBrief"],
                "extra_title" => &["extra_title", "extraTitle"], "extra_url" => &["extra_url", "extraUrl"], "extra_key" => &["extra_key", "extraKey"],
                "extra_pic" => &["extra_pic", "extraPic"], "extra_info" => &["extra_info", "extraInfo"], "message_cover" => &["message_cover", "messageCover"],
                "original_type" => &["original_type", "originalType"], "is_editInDyh" => &["is_editInDyh", "isEditInDyh"], "forwardid" => &["forwardid", "forwardId"],
                "fid" => &["fid"], "dyhId" => &["dyhId", "dyh_id"], "targetType" => &["targetType", "target_type"], "productId" => &["productId", "product_id"],
                "targetId" => &["targetId", "target_id"], "location_city" => &["location_city", "locationCity"], "location_country" => &["location_country", "locationCountry"],
                "disallow_reply" => &["disallow_reply", "disallowReply"], "vote_score" => &["vote_score", "voteScore"], "replyWithForward" => &["replyWithForward", "reply_with_forward"],
                "media_info" => &["media_info", "mediaInfo"], "insert_product_media" => &["insert_product_media", "insertProductMedia"], "is_ks_doc" => &["is_ks_doc", "isKsDoc"],
                "goods_list_id" => &["goods_list_id", "goodsListId"], "is_html_article" => &["is_html_article", "isHtmlArticle"], _ => &[],
            };
            if let Some(raw) = source_keys.iter().find_map(|source_key| original.get(*source_key).filter(|value| !value.is_null())) {
                *value = if let Some(flag) = raw.as_bool() { if flag { "1" } else { "0" }.to_string() } else { value_to_string(raw) };
            }
        }
        if is_article_update {
            let article_options = options.expect("article update options checked above");
            for (key, value) in &mut form {
                match *key {
                    "pic" => value.clear(),
                    "is_html_article" => *value = "1".to_string(),
                    "message_title" => *value = article_options.message_title.clone().unwrap_or_default(),
                    "message_cover" => *value = article_options.message_cover.clone().unwrap_or_default(),
                    _ => {},
                }
            }
        }
        for (key, source_keys) in [("province", &["province"][..]), ("city_code", &["city_code", "cityCode"][..])] {
            if let Some(raw) = source_keys.iter().find_map(|source_key| original.get(*source_key).filter(|value| !value.is_null())) { form.push((key, value_to_string(raw))); }
        }
        if let Some(token) = post_token.filter(|token| !token.trim().is_empty()) { form.push(("_v2_post_token", token.to_string())); }
        let updated = wrap_api_data(self.api_post("/v6/feed/changeFeed", &[], &form).await?)?;
        if updated.get("data").and_then(|data| data.get("id")).map(value_to_string).as_deref() != Some(feed_id) { return Err("服务端未返回修改后的动态，请刷新确认".to_string()); }
        Ok(updated)
    }

    /// 按 APK 的回退链路，把 Video.requestParams 交给酷安播放器接口解析。
    ///
    /// APK 的 `CoolApkDataProvider` 先尝试本地 videoParser；解析失败时调用
    /// `POST /v6/player/getUrl`，唯一表单字段为 `params`，值是 provider-specific
    /// requestParams，而不是整个 Feed、mediaInfo 或 mediaUrl。
    pub async fn resolve_video_url(&self, request_params: &str) -> Result<Value, String> {
        let params = request_params.trim();
        if params.is_empty() {
            return Err("视频解析参数为空".to_string());
        }

        let raw = self
            .api_post(
                "/v6/player/getUrl",
                &[],
                &[("params", params.to_string())],
            )
            .await?;
        wrap_api_data(raw)
    }

    /// 为 WebView2 代理微博视频的 Range 请求。
    ///
    /// 微博 CDN 返回的跨域媒体会被 WebView2 的 ORB 拦截，原生播放器则可以正常读取。
    /// 这里只允许微博 CDN，并且不携带酷安 Cookie 或 App 指纹头。
    pub async fn proxy_weibo_video(
        &self,
        video_url: &str,
        range: Option<&str>,
    ) -> Result<ProxiedVideoResponse, String> {
        let parsed = reqwest::Url::parse(video_url.trim())
            .map_err(|error| format!("微博视频地址无效：{error}"))?;
        let host = parsed.host_str().unwrap_or_default();
        if parsed.scheme() != "https" || !is_weibo_video_host(host) {
            return Err("仅允许代理微博 HTTPS 视频地址".to_string());
        }

        let client = session(parsed.as_str(), SessionPolicy::Follow)?;
        let mut request = client
            .get(parsed)
            .timeout(std::time::Duration::from_secs(90))
            .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept", "video/mp4,video/*;q=0.9,*/*;q=0.8")
            .header("Referer", "https://weibo.com/");
        if let Some(range) = range.filter(|value| !value.trim().is_empty()) {
            request = request.header("Range", range);
        }

        let response = request
            .send()
            .await
            .map_err(|error| format!("读取微博视频失败：{error}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("微博视频返回 HTTP {status}"));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.starts_with("video/"))
            .unwrap_or("video/mp4")
            .to_string();
        let content_length = response.content_length();
        let content_range = response
            .headers()
            .get("content-range")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response
            .bytes()
            .await
            .map_err(|error| format!("读取微博视频内容失败：{error}"))?
            .to_vec();

        Ok(ProxiedVideoResponse {
            status: status.as_u16(),
            content_type,
            content_length,
            content_range,
            body,
        })
    }

    /// 按 APK 的 Live Photo 链路获取实况视频最终地址。
    ///
    /// 酷安返回的图片是静态封面，`/v6/livePhoto/showVideo` 会将
    /// `picUrl` 与 `feed_<id>`/`reply_<id>` 解析为一个可播放的视频重定向。
    pub async fn resolve_live_photo_video(
        &self,
        image_url: &str,
        content_id: &str,
        content_type: &str,
    ) -> Result<Value, String> {
        let image_url = image_url.trim();
        let parsed_image = reqwest::Url::parse(image_url)
            .map_err(|e| format!("invalid Live Photo image URL: {e}"))?;
        let image_host = parsed_image.host_str().unwrap_or_default();
        if (parsed_image.scheme() != "http" && parsed_image.scheme() != "https")
            || !is_coolapk_host(image_host)
        {
            return Err("Live Photo 图片地址必须来自酷安官方域名".to_string());
        }

        // APK 将接口返回的原始 image.coolapk.com HTTP 地址传给 showVideo。
        // 图片渲染层可能已将它规范化为 HTTPS，但解析接口会校验该参数的
        // 原始格式；对酷安 CDN 仅恢复协议，不改变路径或查询参数。
        let resolver_image_url = if parsed_image.scheme() == "https"
            && image_host.eq_ignore_ascii_case("image.coolapk.com")
        {
            let mut url = parsed_image.clone();
            let _ = url.set_scheme("http");
            url.to_string()
        } else {
            image_url.to_string()
        };

        let content_id = content_id.trim();
        if content_id.is_empty() || content_id.len() > 128 || content_id.chars().any(|ch| ch.is_control()) {
            return Err("Live Photo 内容 ID 无效".to_string());
        }
        let content_type = match content_type.trim() {
            "reply" => "reply",
            "article" => "article",
            _ => "feed",
        };

        let mut wrapper_url = reqwest::Url::parse("https://api.coolapk.com/v6/livePhoto/showVideo")
            .map_err(|e| format!("invalid Live Photo resolver URL: {e}"))?;
        wrapper_url
            .query_pairs_mut()
            .append_pair("picUrl", &resolver_image_url)
            .append_pair("id", &format!("{content_type}_{content_id}"));
        let wrapper_url_text = wrapper_url.to_string();

        let token = self.get_token()?;
        let mut request = self.apply_device_profile(
            self.api_redirect_client()?
                .get(wrapper_url)
                .header("X-App-Token", token)
                .header("X-Requested-With", "XMLHttpRequest"),
        )?;
        let cookie = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .clone();
        if let Some(cookie) = cookie {
            let cookie = cookie_without_ddid(&cookie);
            if let Ok(header_value) = HeaderValue::from_str(&cookie) {
                request = request.header(COOKIE, header_value);
            }
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        let status = response.status();
        let redirect_url = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|location| {
                response
                    .url()
                    .join(location)
                    .map(|url| url.to_string())
                    .map_err(|e| format!("酷安返回了无效的 Live Photo 视频地址: {e}"))
            })
            .transpose()?;
        if status.is_redirection() {
            if let Some(final_url) = redirect_url
                .filter(|url| url != &wrapper_url_text)
                .filter(|url| {
                    reqwest::Url::parse(url)
                        .map(|parsed| parsed.scheme() == "http" || parsed.scheme() == "https")
                        .unwrap_or(false)
                })
            {
                return Ok(json!({ "code": 200, "data": { "url": final_url } }));
            }
        }

        let body = response
            .text()
            .await
            .map_err(|e| format!("failed to read Live Photo response: {e}"))?;
        if !status.is_success() {
            let detail = body.chars().take(300).collect::<String>();
            return Err(format!("酷安 Live Photo 解析失败：HTTP {status}: {detail}"));
        }
        if let Ok(value) = serde_json::from_str::<Value>(&body) {
            if let Some(url) = extract_live_photo_video_url(&value) {
                return Ok(json!({ "code": 200, "data": { "url": url } }));
            }
        }
        Err("酷安未返回可播放的 Live Photo 视频地址".to_string())
    }

    /// 仅读取 Live Photo 视频的前部数据，用于识别容器中的实际视频编码。
    pub async fn get_live_photo_video_header(&self, video_url: &str) -> Result<String, String> {
        const HEADER_LIMIT: usize = 256 * 1024;
        let video_url = video_url.trim();
        let parsed_url = reqwest::Url::parse(video_url).map_err(|e| format!("invalid Live Photo video URL: {e}"))?;
        let host = parsed_url.host_str().unwrap_or_default();
        if parsed_url.scheme() != "https" || !is_coolapk_host(host) {
            return Err("Live Photo 视频地址必须来自酷安官方 HTTPS 域名".to_string());
        }

        let client = session(parsed_url.as_str(), SessionPolicy::Follow)?;
        let mut response = client
            .get(parsed_url)
            .timeout(std::time::Duration::from_secs(12))
            .header("Range", format!("bytes=0-{}", HEADER_LIMIT - 1))
            .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Referer", "https://www.coolapk.com/")
            .send()
            .await
            .map_err(|e| format!("failed to fetch Live Photo video header: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("Live Photo 视频头请求失败：HTTP {}", response.status()));
        }

        let mut header = Vec::with_capacity(HEADER_LIMIT);
        while header.len() < HEADER_LIMIT {
            let Some(chunk) = response.chunk().await.map_err(|e| format!("failed to read Live Photo video header: {e}"))? else {
                break;
            };
            let remaining = HEADER_LIMIT - header.len();
            header.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        }
        if header.is_empty() {
            return Err("Live Photo 视频头为空".to_string());
        }
        Ok(BASE64.encode(header))
    }

    /// 获取单条评论的完整元数据。
    /// 评论列表接口会省略设备型号等字段，详情接口用于后台补齐，不影响列表首屏显示。
    pub async fn get_reply_detail(&self, reply_id: &str) -> Result<Value, String> {
        let query = [("id", reply_id.to_string())];
        // 通知定位优先沿用当前登录态，避免独立游客设备验证失败时无法读取评论。
        let primary_error = match self.api_get("/v6/feed/replyDetail", &query).await {
            Ok(value) if matches!(application_failure(&value), Some((403, "captcha"))) => return Err(value.to_string()),
            Ok(value) => match Self::normalize_reply_detail(&value) {
                Ok(detail) => return Ok(detail),
                Err(error) => error,
            },
            Err(error) => error,
        };
        // 登录请求不可用时尝试游客读取，保留各入口的错误以便诊断。
        let mut public_errors = Vec::new();
        for api_origin in ["https://api.coolapk.com", "https://api2.coolapk.com"] {
            match self.public_api_get_from(api_origin, "/v6/feed/replyDetail", &query).await {
                Ok(value) if matches!(application_failure(&value), Some((403, "captcha"))) => return Err(value.to_string()),
                Ok(value) => match Self::normalize_reply_detail(&value) {
                    Ok(detail) => return Ok(detail),
                    Err(error) => public_errors.push(format!("{api_origin}: {error}")),
                },
                Err(error) => public_errors.push(format!("{api_origin}: {error}")),
            }
        }
        Err(format!("评论详情加载失败：{primary_error}；游客接口：{}", public_errors.join("；")))
    }

    pub(super) fn normalize_reply_detail(raw: &Value) -> Result<Value, String> {
        // 先解析服务端错误，不能将验证失败等响应误报为评论数据为空。
        let raw = wrap_api_data(raw.clone())?;
        let obj = raw
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| "评论详情数据为空".to_string())?;
        let user_info = obj.get("userInfo").and_then(Value::as_object);
        let user_agent = obj.get("useragent").and_then(Value::as_str).unwrap_or("");
        let (ua_device_title, ua_device_build, ua_device_rom) = parse_reply_user_agent(user_agent);
        let device_title = obj
            .get("device_title")
            .and_then(Value::as_str)
            .or_else(|| obj.get("deviceTitle").and_then(Value::as_str))
            .or_else(|| obj.get("device_name").and_then(Value::as_str))
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&ua_device_title);
        let device_build = obj
            .get("device_build")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&ua_device_build);
        let device_rom = obj
            .get("device_rom")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&ua_device_rom);

        // 保留 fid、rid/rrid、作者和原动态等关联字段；前端还需要用它们导航和定位。
        let mut detail = obj.clone();
        let normalized = json!({
                "id": obj.get("id").map(value_to_string).unwrap_or_default(),
                // 评论列表可能截断正文并附加“查看更多”，详情中的 message 用于原位展开。
                "message": obj.get("message").cloned().unwrap_or(Value::Null),
                "deviceTitle": device_title,
                "deviceName": obj.get("device_name").and_then(Value::as_str).unwrap_or(""),
                "deviceBuild": device_build,
                "deviceRom": device_rom,
                "userAgent": user_agent,
                "userLevel": user_info.and_then(|user| user.get("level")).map(value_to_string).unwrap_or_default(),
                "verifyTitle": user_info.and_then(|user| user.get("verify_title")).and_then(Value::as_str).unwrap_or(""),
                "pic": obj.get("pic").cloned().unwrap_or(Value::Null),
                "dateline": obj.get("dateline").cloned().unwrap_or(json!(0)),
                "isFeedAuthor": obj.get("isFeedAuthor").cloned().unwrap_or(json!(0)),
                "ipLocation": obj.get("ipLocation").and_then(Value::as_str).or_else(|| obj.get("ip_location").and_then(Value::as_str)).unwrap_or("")
        });
        if let Some(fields) = normalized.as_object() {
            detail.extend(fields.clone());
        }
        Ok(json!({ "code": 200, "data": detail }))
    }

    pub async fn get_hot_replies(&self, feed_id: &str, page: u32) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/feed/hotReplyList",
                &[
                    ("id", feed_id.to_string()),
                    ("page", page.to_string()),
                    ("discussMode", "1".to_string()),
                ],
            )
            .await?,
        )
    }

    pub async fn search_feeds(
        &self,
        query: &str,
        page: u32,
        sort_type: &str,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/search",
                &[
                    ("type", "feed".to_string()),
                    ("searchValue", query.to_string()),
                    ("page", page.to_string()),
                    ("sortType", sort_type.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }
}
