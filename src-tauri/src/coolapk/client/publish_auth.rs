use super::*;

impl CoolapkClient {
    pub(super) async fn submit_create_feed_form(
        &self,
        form: Vec<(&'static str, String)>,
        failure_prefix: &str,
    ) -> Result<Value, String> {
        self.submit_create_feed_form_with_device(form, failure_prefix, true).await
    }

    pub(super) fn create_feed_request(&self, form: &[(&str, String)], include_device_model: bool) -> Result<reqwest::RequestBuilder, String> {
        let code = self.device_code.read().map_err(|_| "failed to read device code")?.clone();
        let code = if include_device_model { code } else { device_code_without_model(&code)? };
        // 发布时单独签名，不能改动共享登录指纹或影响并发请求。
        let token = CoolapkAuth::new(code.clone()).get_app_token_with_app_code(self.request_app_code()?)?;
        let request = self.apply_device_profile_with_code(
            self.api_client()?
                .request(
                    reqwest::Method::POST,
                    "https://api.coolapk.com/v6/feed/createFeed",
                )
                .header("X-App-Token", token)
                .header("X-Requested-With", "XMLHttpRequest")
                .form(form),
            &code,
        )?;
        if include_device_model { return Ok(request); }
        let profile = self.effective_device_profile();
        // 覆盖默认 UA 和自定义 UA，避免从 #Build 或 Dalvik 片段重新识别机型。
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_str(&format!("CoolMarket/{}-{}-universal",
            profile.app_version.as_deref().unwrap_or("16.6.5"),
            profile.app_code.as_deref().unwrap_or("2610162"))).map_err(|_| "发布 UA 格式无效")?);
        Ok(request.headers(headers))
    }

    pub(super) async fn submit_create_feed_form_with_device(&self, form: Vec<(&'static str, String)>, failure_prefix: &str, include_device_model: bool) -> Result<Value, String> {
        let mut request = self.create_feed_request(&form, include_device_model)?;

        let cookie = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .clone();
        if let Some(cookie) = cookie {
            let full_cookie = self.cookie_for_path(&cookie, "/v6/feed/createFeed");
            if let Ok(header_val) = reqwest::header::HeaderValue::from_str(&full_cookie) {
                request = request.header(COOKIE, header_val);
            }
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        let wrapped = wrap_api_data(response_json(response).await?)?;
        // 发布成功时服务端必须返回新建实体（含 id）；data 缺失/为空说明
        // 服务端虽然返回了 200 信封但并未真正创建内容，必须视为失败。
        let created = wrapped
            .get("data")
            .and_then(|d| d.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .or_else(|| {
                wrapped
                    .get("data")
                    .and_then(|d| d.get("id").and_then(|v| v.as_u64()))
                    .map(|n| n.to_string())
            });
        if created.is_none() {
            return Err(format!("{failure_prefix}服务端未返回发布结果，请重试"));
        }
        Ok(wrapped)
    }

    /// 发布动态（需登录）
    /// 官方客户端要求 POST application/x-www-form-urlencoded；普通动态默认 is_html_article=0，
    /// 图文文章使用 is_html_article=1，大封面动态沿用 is_html_article=2。
    pub async fn create_feed(
        &self,
        message: &str,
        pic: Option<&str>,
        post_token: Option<&str>,
    ) -> Result<Value, String> {
        // createFeed 在 `PostToken.List` 内，官方建议携带网易易盾 _v2_post_token。
        // 实测服务端对该字段并非强制（无 token 亦能发布成功），因此 token 为可选，
        // 仅在调用方（前端）提供时附加；缺失时仍正常提交，若服务端拒绝再提示验证。
        self.submit_create_feed_form_with_device(
            build_create_feed_form(message, pic, post_token),
            "发布动态失败：",
            self.default_publish_device_model(),
        )
        .await
    }

    /// 普通动态扩展选项使用同一个官方表单接口，应用板块按 APK 提交 comment 类型。
    pub async fn create_feed_with_options(&self, message: &str, pic: Option<&str>, post_token: Option<&str>, options: Option<&PublishOptions>) -> Result<Value, String> {
        let mut form = build_create_feed_form(message, pic, post_token);
        if let Some(options) = options { apply_publish_options(&mut form, options)?; }
        self.submit_create_feed_form_with_device(form, "发布动态失败：", options.and_then(|value| value.include_device_model).unwrap_or_else(|| self.default_publish_device_model())).await
    }

    /// 回答问题（需登录）。APK 仍使用 createFeed，只是 type=answer 且 fid 为问题 ID。
    pub async fn create_answer(
        &self,
        question_id: &str,
        message: &str,
        pic: Option<&str>,
        post_token: Option<&str>,
    ) -> Result<Value, String> {
        let question_id = question_id.trim();
        if question_id.is_empty() {
            return Err("问题 ID 不能为空".to_string());
        }
        let message = message.trim();
        if message.is_empty() {
            return Err("回答内容不能为空".to_string());
        }
        self.submit_create_feed_form(
            build_create_feed_form_for_type(message, pic, post_token, "answer", question_id),
            "发布回答失败：",
        )
        .await
    }

    /// 转发动态（需登录）
    /// 官方通过 createFeed 的 forwardid 字段关联原动态，使用表单提交。
    pub async fn create_forward(
        &self,
        feed_id: &str,
        message: &str,
        pic: Option<&str>,
    ) -> Result<Value, String> {
        if feed_id.trim().is_empty() {
            return Err("转发失败：原动态 ID 为空".to_string());
        }
        self.submit_create_feed_form(build_forward_form(message, pic, feed_id), "转发失败：").await
    }

    pub async fn check_login_status(&self) -> Result<Value, String> {
        // 先验证当前会话，避免 /user/space 把任意公开用户资料误当成当前登录用户。
        let login_info = self.check_login_info().await?;
        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Ok(guard) = self.user_cookie.read() {
            if let Some(cookie_str) = guard.as_ref() {
                for item in cookie_str.split(';') {
                    let parts: Vec<&str> = item.trim().split('=').collect();
                    if parts.len() == 2 && parts[0] == "uid" {
                        query_params.push(("uid", parts[1].to_string()));
                        break;
                    }
                }
            }
        }

        let query_refs: Vec<(&str, String)> =
            query_params.iter().map(|(k, v)| (*k, v.clone())).collect();
        if query_params.is_empty() {
            return Ok(login_info);
        }
        match self.api_get("/v6/user/space", &query_refs).await {
            Ok(res) => {
                if let Some(data) = res.get("data") {
                    return Ok(json!({ "code": 200, "data": data }));
                }
            }
            Err(_) => {
                log::warn!("login.user_space_failed_after_status_success");
            }
        }
        Ok(login_info)
    }

    pub fn clear_user_cookie(&self) -> Result<(), String> {
        let mut stored = self
            .user_cookie
            .write()
            .map_err(|_| "failed to lock login state".to_string())?;
        *stored = None;
        drop(stored);
        if let Ok(mut state) = self.generated_identity.write() { state.session_changed(); }
        // 清空当前登录标记（保留账户记录，便于下次快速切换）
        let mut root = self.load_accounts_root();
        root["lastLoginUid"] = json!("");
        self.save_accounts_root(&root);
        // 清理旧版 txt 遗留文件
        if let Some(path) = self.cookie_file.read().ok().and_then(|g| g.clone()) {
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        }
        // 登出后回到游客态，同步游客设备码
        self.sync_device_code();
        Ok(())
    }

    pub async fn login_by_account(&self, account: &str, password: &str) -> Result<Value, String> {
        use md5::{Digest, Md5};
        let mut hasher = Md5::new();
        hasher.update(password.as_bytes());
        let md5_pwd = format!("{:x}", hasher.finalize());

        let res = self
            .api_post(
                "/v6/account/login",
                &[],
                &[
                    ("login", account.to_string()),
                    ("password", password.to_string()),
                    ("md5_pass", md5_pwd.clone()),
                    ("md5_password", md5_pwd),
                ],
            )
            .await?;

        if let Some(msg) = res.get("message").and_then(Value::as_str) {
            if msg.contains("unsupported") || res.get("status").and_then(Value::as_i64) == Some(403)
            {
                return Err("酷安官方现已停用第三方原生账号密码 API (403 Unsupported)，请切换至【SESSID 凭据】标签导入凭据登录。".to_string());
            }
        }

        self.extract_and_set_session(&res);
        wrap_api_data(res)
    }

    pub async fn send_sms_vcode(&self, mobile: &str) -> Result<Value, String> {
        let first_try = self
            .api_post(
                "/v6/account/sendVcode",
                &[],
                &[
                    ("mobile", mobile.to_string()),
                    ("type", "login".to_string()),
                ],
            )
            .await;

        match first_try {
            Ok(res) => {
                if let Some(msg) = res.get("message").and_then(Value::as_str) {
                    if msg.contains("unsupported")
                        || res.get("status").and_then(Value::as_i64) == Some(403)
                    {
                        return Err("酷安官方已停用第三方纯验证码直连 API (403 API Unsupported)，请使用【SESSID 凭据】快捷登录。".to_string());
                    }
                }
                wrap_api_data(res)
            }
            Err(err1) => Err(format!("验证码下发失败: {err1}")),
        }
    }

    pub async fn login_by_mobile(&self, mobile: &str, vcode: &str) -> Result<Value, String> {
        let res = self
            .api_post(
                "/v6/account/loginByMobile",
                &[],
                &[
                    ("mobile", mobile.to_string()),
                    ("vcode", vcode.to_string()),
                    ("code", vcode.to_string()),
                ],
            )
            .await?;

        if let Some(msg) = res.get("message").and_then(Value::as_str) {
            if msg.contains("unsupported") || res.get("status").and_then(Value::as_i64) == Some(403)
            {
                return Err("酷安官方已停用第三方手机号登录 API (403 API Unsupported)，请使用【SESSID 凭据】快捷登录。".to_string());
            }
        }

        self.extract_and_set_session(&res);
        wrap_api_data(res)
    }

    /// 使用官方登录 WebView 的完整 Cookie 校验并保存当前账户。
    pub async fn login_by_webview_cookie(&self, cookie: &str) -> Result<Value, String> {
        let cookie = Self::sanitize_cookie(cookie);
        if !Self::has_valid_session_cookie(&cookie) {
            return Err("官方登录 WebView 没有返回有效的 SESSID".to_string());
        }
        self.set_user_cookie(cookie.clone())?;
        log::info!("login.cookie_validation_started");
        let result = self.check_login_info().await?;
        log::info!("login.cookie_validation_succeeded");
        let data = result.get("data").unwrap_or(&result);
        let uid = data
            .get("uid")
            .or_else(|| data.get("id"))
            .map(value_to_string)
            .unwrap_or_default();
        let username = data
            .get("username")
            .or_else(|| data.get("userName"))
            .map(value_to_string)
            .unwrap_or_default();
        let avatar = data
            .get("userAvatar")
            .or_else(|| data.get("avatar"))
            .map(value_to_string)
            .unwrap_or_default();
        if uid.is_empty() || uid == "0" || uid == "10000" {
            return Err("官方登录 WebView Cookie 未通过账号校验".to_string());
        }
        self.save_account(&uid, &username, &avatar, &cookie).await?;
        Ok(result)
    }

    /// 按 APK 登录页的回调流程，用一次性授权码换取 LoginInfo。
    /// 官方流程是先用 WebView 的 SESSID 调用 accessToken，再把 uid、username、token 写入后续请求。
    pub async fn login_by_access_code(
        &self,
        code: &str,
        callback_cookie: Option<&str>,
    ) -> Result<Value, String> {
        let code = code.trim();
        if code.is_empty() {
            return Err("酷安授权码为空".to_string());
        }

        let raw_cookie = match callback_cookie {
            Some(value) if !value.trim().is_empty() => Self::sanitize_cookie(value),
            _ => self
                .get_user_cookie()
                .ok_or_else(|| "授权回调没有带回登录 Cookie".to_string())?,
        };
        let cookie = remove_cookie_values(&raw_cookie, &["uid", "username", "token"]);
        if !Self::has_valid_session_cookie(&cookie) {
            return Err("授权回调没有带回有效的 SESSID".to_string());
        }

        // accessToken 只应使用本次 WebView 回调的会话，避免把旧账号的 uid/token 带给一次性授权码。
        self.set_user_cookie(cookie.clone())?;
        log::info!("login.access_token_request_started");
        let result = wrap_api_data(
            self.api_get(
                "/v6/account/accessToken",
                &[("code", code.to_string())],
            )
            .await?,
        )?;
        log::info!("login.access_token_response_accepted");
        let data = result.get("data").unwrap_or(&result);
        let uid = data
            .get("uid")
            .or_else(|| data.get("id"))
            .map(value_to_string)
            .unwrap_or_default();
        let username = data
            .get("username")
            .or_else(|| data.get("userName"))
            .map(value_to_string)
            .unwrap_or_default();
        // APK 会优先使用 refreshToken 作为后续请求的 token。
        let token = data
            .get("refreshToken")
            .map(value_to_string)
            .filter(|value| !value.is_empty())
            .or_else(|| data.get("token").map(value_to_string))
            .unwrap_or_default();
        let avatar = data
            .get("userAvatar")
            .or_else(|| data.get("avatar"))
            .map(value_to_string)
            .unwrap_or_default();
        if uid.is_empty() || uid == "0" || uid == "10000" || username.is_empty() || token.is_empty()
        {
            return Err("酷安授权接口未返回完整登录信息".to_string());
        }
        log::info!("login.access_token_identity_complete");

        let stored_cookie = merge_cookie_value(&cookie, "uid", &encode_login_cookie_value(&uid));
        let stored_cookie = merge_cookie_value(
            &stored_cookie,
            "username",
            &encode_login_cookie_value(&username),
        );
        let stored_cookie = merge_cookie_value(
            &stored_cookie,
            "token",
            &encode_login_cookie_value(&token),
        );
        self.save_account(&uid, &username, &avatar, &stored_cookie)
            .await?;
        Ok(result)
    }

    pub(super) fn extract_and_set_session(&self, response: &Value) {
        if let Some(data) = response.get("data") {
            let sessid = data
                .get("sessid")
                .or_else(|| data.get("token"))
                .and_then(|v| v.as_str());
            let uid = data
                .get("uid")
                .or_else(|| data.get("id"))
                .and_then(|v| v.as_str());

            if let (Some(s), Some(u)) = (sessid, uid) {
                let cookie_str = format!("SESSID={}; uid={}", s, u);
                let _ = self.set_user_cookie(cookie_str);
            } else if let Some(s) = sessid {
                let cookie_str = format!("SESSID={}", s);
                let _ = self.set_user_cookie(cookie_str);
            }
        }
    }
}
