use super::*;

impl CoolapkClient {
    pub async fn get_image_data_url(&self, source_url: &str) -> Result<String, String> {
        let source = reqwest::Url::parse(source_url).map_err(|error| error.to_string())?;
        let host = source.host_str().unwrap_or_default();
        let response = self.resource_request(source_url, true)?
            .send()
            .await
            .map_err(|e| format!("failed to fetch image: {e}"))?;
        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_string();
        if !status.is_success() {
            return Err(format!("Coolapk image CDN returned HTTP {status}"));
        }
        // 诊断 ZIP 仅允许酷安 feed 原始附件地址，其他图片请求维持原有类型限制。
        let diagnostic_zip = host == "image.coolapk.com"
            && reqwest::Url::parse(source_url).map(|source| source.path().starts_with("/feed/") && source.path().to_ascii_lowercase().ends_with(".zip") && source.query().is_none() && source.fragment().is_none()).unwrap_or(false)
            && matches!(content_type.split(';').next().unwrap_or("").trim(), "application/zip" | "application/x-zip-compressed" | "application/octet-stream");
        if !content_type.starts_with("image/") && !diagnostic_zip {
            return Err(format!("unexpected image content type: {content_type}"));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("failed to read image: {e}"))?;
        if bytes.len() > 50 * 1024 * 1024 {
            return Err("image exceeds the 50 MB desktop limit".to_string());
        }
        let content_type = if diagnostic_zip { "application/zip" } else { &content_type };
        Ok(format!(
            "data:{content_type};base64,{}",
            BASE64.encode(bytes)
        ))
    }

    /// 官方挂件页面使用网页 JSON 协议，不能套用 /v6 API 或静态 HTML 抓取。
    pub(super) async fn user_plugin_request(
        &self,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
    ) -> Result<Value, String> {
        let cookie = self.get_user_cookie().filter(|s| !s.trim().is_empty())
            .ok_or_else(|| "请先登录酷安账号".to_string())?;
        let did = self.effective_custom_device_id();
        let ddid = if path == "getPlugin" || path == "savePlugin" { self.effective_custom_ddid() } else { None };
        let cookie = user_plugin_cookie(&cookie, did.as_deref(), ddid.as_deref());
        let url = format!("https://m.coolapk.com/mp/userPlugin/{path}");
        let client = session(&url, SessionPolicy::NoRedirect)?;
        let request = match form {
            Some(form) => client.post(&url).form(form),
            None => client.get(&url),
        };
        // APK UserAgentHandler 经 C2304 添加 (#Build; ...) 后追加 CoolMarket 版本。
        // 缺少 Build 段时，服务端会将该页面判为普通移动浏览器并拒绝访问。
        let response = request.timeout(std::time::Duration::from_secs(20)).query(query)
            .header(USER_AGENT, "Mozilla/5.0 (Linux; Android 14; wv) AppleWebKit/537.36 Version/4.0 Chrome/130.0.0.0 Mobile Safari/537.36 (#Build; nubia; NX789J; AQ3A.250226.002; Android) +CoolMarket/16.6.2-2609151-universal")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", "https://m.coolapk.com/mp/userPlugin/myPlugin?autoTheme=1")
            .header("Origin", "https://m.coolapk.com")
            .header(COOKIE, cookie)
            .send().await.map_err(|e| format!("挂件请求失败：{e}"))?;
        if !response.status().is_success() {
            return Err(format!("挂件服务返回 HTTP {}", response.status().as_u16()));
        }
        let value: Value = response.json().await.map_err(|_| "挂件服务未返回有效 JSON".to_string())?;
        // 网页 JSON 还包含会话信息；只把功能所需的字段交给前端。
        let mut data = serde_json::Map::new();
        for key in ["avatarPluginList", "feedPluginList", "pluginList", "avatarPluginUrl", "feedPluginUrl",
            "selectedAvatarPluginRow", "selectedFeedPluginRow", "deviceTitle", "status", "message", "forwardUrl"] {
            if let Some(item) = value.get(key) { data.insert(key.to_string(), item.clone()); }
        }
        Ok(json!({ "code": 200, "data": data }))
    }

    pub async fn get_user_plugins(&self, store: bool, page: u32, plugin_type: u8) -> Result<Value, String> {
        if page == 0 || plugin_type > 1 { return Err("挂件分页参数无效".to_string()); }
        self.user_plugin_request(if store { "store" } else { "myPlugin" },
            &[("page", page.to_string()), ("type", plugin_type.to_string())], None).await
    }

    pub async fn save_user_plugins(&self, avatar_id: u64, feed_id: u64) -> Result<Value, String> {
        self.user_plugin_request("savePlugin", &[], Some(&[("avatar_id", avatar_id.to_string()), ("feed_id", feed_id.to_string())])).await
    }

    pub async fn claim_user_plugin(&self, id: u64) -> Result<Value, String> {
        if id == 0 { return Err("挂件 ID 无效".to_string()); }
        self.user_plugin_request("getPlugin", &[("id", id.to_string())], None).await
    }

    /// 抓取外部网页（内置浏览器阅读模式用）：带移动 UA 与已登录 Cookie（仅限酷安官方域），
    /// 返回页面标题与 HTML 正文，由前端安全化渲染。
    /// 注意：必须用独立干净 Client——主 client 携带酷安 App 指纹头（Dalvik UA/X-App-Token），
    /// 网页版服务器遇到这些头会返回空响应。
    pub async fn fetch_external_page(&self, url: &str) -> Result<Value, String> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("仅支持 http(s) 链接".to_string());
        }

        let page_client = session(url, SessionPolicy::Follow)?;

        let parsed_url = reqwest::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
        let is_coolapk_target = parsed_url.host_str().map(is_coolapk_host).unwrap_or(false);

        let mut request = page_client
            .get(url)
            .timeout(std::time::Duration::from_secs(15))
            .header("User-Agent", MOBILE_UA)
            .header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.8");

        // 酷安网页版动态页（www.coolapk.com/feed/xxx）带 X- 系列头时会直接返回 JSON 动态详情
        // （绕过 api.coolapk.com/v6/feed/detail 的验证码风控），其他页面保持 HTML 渲染；
        // 指纹头只允许发给酷安官方域（不能仅靠字符串 contains 判断，防止第三方域伪造路径）
        if is_coolapk_target && url.contains("coolapk.com/feed/") {
            request = request
                .header("X-Requested-With", "XMLHttpRequest")
                .header("X-App-Id", "com.coolapk.market");
        }

        let cookie = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .clone();
        if let Some(cookie) = cookie {
            // 仅当目标是酷安官方域时才附带登录 Cookie；
            // 抓取任意第三方网页时绝不携带凭据，防止恶意链接窃取登录态
            if is_coolapk_target {
                let cookie = cookie_without_ddid(&cookie);
                if let Ok(header_val) = reqwest::header::HeaderValue::from_str(&cookie) {
                    request = request.header(COOKIE, header_val);
                }
            }
        }

        let resp = request.send().await.map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(|e| e.to_string())?;
        let title = extract_html_title(&body).unwrap_or_else(|| "外部链接".to_string());
        // 只取正文：剥离导航/页脚/脚本等外壳噪音（酷安 /feed/ 分享页即为纯扫码落地页）
        // 动态接口在 XHR 模式下返回 JSON，必须保留原文，否则正文里的 HTML 可能被网页清洗器误删。
        let is_report_page = parsed_url.scheme() == "https"
            && parsed_url.host_str() == Some("m.coolapk.com")
            && parsed_url.path() == "/mp/do"
            && parsed_url.query_pairs().any(|(key, value)| key == "m" && value == "report");
        let content = if is_report_page || (is_coolapk_target
            && url.contains("coolapk.com/feed/")
            && serde_json::from_str::<Value>(&body).is_ok())
        {
            body.clone()
        } else {
            extract_readable_content(&body)
        };

        Ok(json!({
            "code": 200,
            "data": { "title": title, "html": content, "status": status }
        }))
    }

    /// 官方网页举报表单：固定目标地址，凭据不经过前端，禁止跨域重定向。
    pub async fn submit_feed_report(&self, id: &str, report_type: &str, reason: &str, custom_reason: &str, request_hash: &str, pictures: &[String]) -> Result<Value, String> {
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) || !["feed", "reply"].contains(&report_type)
            || reason.trim().is_empty() || reason.len() > 1000 || request_hash.is_empty()
            || request_hash.len() > 128 || custom_reason.len() > 4000 || pictures.len() > 9 {
            return Err("举报参数无效".to_string());
        }
        if reason == "其他" && custom_reason.trim().is_empty() { return Err("请输入自定义举报原因".to_string()); }
        for picture in pictures {
            let parsed = reqwest::Url::parse(picture).map_err(|_| "举报图片地址无效".to_string())?;
            if parsed.scheme() != "https" || !parsed.host_str().map(is_coolapk_host).unwrap_or(false) {
                return Err("举报图片地址无效".to_string());
            }
        }
        let cookie = self.user_cookie.read().map_err(|_| "无法读取登录状态".to_string())?.clone()
            .ok_or_else(|| "请先登录后举报".to_string())?;
        let client = session("https://m.coolapk.com", SessionPolicy::NoRedirect)?;
        let form = reqwest::multipart::Form::new()
            .text("requestHash", request_hash.to_string()).text("submit", "1")
            .text("id", id.to_string()).text("type", report_type.to_string())
            .text("report_reason", reason.to_string()).text("custom_report_reason", custom_reason.to_string())
            .text("pic", pictures.join(","));
        let response = client.post("https://m.coolapk.com/mp/do?c=feed&m=report")
            .timeout(std::time::Duration::from_secs(30))
            .header("User-Agent", MOBILE_UA).header("X-Requested-With", "XMLHttpRequest")
            .header("Origin", "https://m.coolapk.com").header("Referer", "https://m.coolapk.com/mp/do?c=feed&m=report")
            .header(COOKIE, cookie_without_ddid(&cookie)).multipart(form).send().await.map_err(|e| e.to_string())?;
        if !response.status().is_success() { return Err(format!("举报提交失败：HTTP {}", response.status())); }
        let value: Value = response.json().await.map_err(|_| "举报服务返回异常，请重新加载表单".to_string())?;
        let success = value.get("status").and_then(Value::as_i64).map(|status| status == 1)
            .or_else(|| value.get("code").and_then(Value::as_i64).map(|code| [0, 1, 200].contains(&code))).unwrap_or(false);
        if !success { return Err(value.get("message").and_then(Value::as_str).unwrap_or("举报服务返回异常，请重新加载表单").to_string()); }
        Ok(json!({ "code": 200, "data": value }))
    }
}
