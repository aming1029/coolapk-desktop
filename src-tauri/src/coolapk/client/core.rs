use super::*;

impl CoolapkClient {
    /// 设备码策略：
    /// - 未登录（游客态）：每台电脑首次启动随机生成一次并持久化，之后固定
    /// - 已登录：使用账号绑定的固定设备码（首次登录生成随机并持久化，之后固定）
    /// 设备码与 Token V3 绑定，切换时 auth 签名同步切换。
    pub fn new() -> Self {
        let native_device_profile = crate::coolapk::native_device::detect();
        let device_code = device_code_with_profile(&generate_random_device_code(), &native_device_profile);
        let (client, redirect_client) = build_api_clients(&native_device_profile).unwrap_or_else(|_| (Client::new(), Client::new()));

        Self {
            client: RwLock::new(client),
            redirect_client: RwLock::new(redirect_client),
            auth: RwLock::new(CoolapkAuth::new(device_code.clone())),
            user_cookie: RwLock::new(None),
            cookie_file: RwLock::new(None),
            device_profile: RwLock::new(DeviceProfile::default()),
            native_device_profile,
            device_code: RwLock::new(device_code),
            device_headers: RwLock::new(None),
            captcha_validation: RwLock::new(None),
            generated_identity: RwLock::new(crate::coolapk::shuzilm::IdentityState::default()),
            identity_generation: tokio::sync::Mutex::new(()),
        }
    }

    pub(super) fn api_client(&self) -> Result<Client, String> { self.client.read().map(|client| client.clone()).map_err(|_| "无法读取酷安 HTTP 客户端".to_string()) }
    pub(super) fn api_redirect_client(&self) -> Result<Client, String> { self.redirect_client.read().map(|client| client.clone()).map_err(|_| "无法读取酷安重定向客户端".to_string()) }

    // 先校验代理地址，再同时替换接口客户端；已开始的请求继续使用原客户端。
    pub fn set_network_proxy(&self, proxy_url: Option<String>) -> Result<(), String> {
        let mut client = self.client.write().map_err(|_| "无法更新酷安 HTTP 客户端".to_string())?;
        let mut redirect_client = self.redirect_client.write().map_err(|_| "无法更新酷安重定向客户端".to_string())?;
        let previous = crate::coolapk::network_proxy::update(proxy_url.as_deref())?;
        let rebuilt = build_api_clients(&self.native_device_profile);
        let (new_client, new_redirect_client) = match rebuilt { Ok(clients) => clients, Err(error) => { crate::coolapk::network_proxy::update(previous.as_deref())?; return Err(error); } };
        *client = new_client;
        *redirect_client = new_redirect_client;
        log::info!("network.proxy_changed enabled={}", proxy_url.as_deref().is_some_and(|value| !value.trim().is_empty()));
        Ok(())
    }

    /// 获取当前设备码签名（Token V3 与设备码绑定，切换设备码后自动换签名）
    pub(super) fn get_token(&self) -> Result<String, String> {
        self.auth
            .read()
            .map_err(|_| "failed to lock auth state".to_string())?
            .get_app_token_with_app_code(self.request_app_code()?)
    }

    /// 开启自定义机型后，请求版本可能来自旧设置，签名必须使用实际发送的版本编码。
    pub(super) fn request_app_code(&self) -> Result<u64, String> {
        // 优先复用已构建的固定头缓存（含 X-App-Code），避免每次请求都合并设备资料。
        if let Ok(cache) = self.device_headers.read() {
            if let Some(headers) = cache.as_ref() {
                return match headers.get("X-App-Code") {
                    Some(value) => value
                        .to_str()
                        .ok()
                        .and_then(|value| value.trim().parse::<u32>().ok())
                        .filter(|value| *value > 0)
                        .map(u64::from)
                        .ok_or_else(|| "自定义酷安版本编码无效，请在设备信息中填写正整数".to_string()),
                    None => Ok(APP_CODE),
                };
            }
        }
        let profile = self.effective_device_profile();
        match profile.app_code.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            Some(value) => value.parse::<u32>().ok().filter(|value| *value > 0).map(u64::from).ok_or_else(|| "自定义酷安版本编码无效，请在设备信息中填写正整数".to_string()),
            None => Ok(APP_CODE),
        }
    }

    /// 同步设备码：已登录使用该账号绑定的固定设备码，
    /// 未登录使用本机持久化的游客设备码（每台电脑首次生成后固定）。
    /// 两者都会持久化，重启后保持不变。
    pub fn sync_device_code(&self) {
        let uid = self.current_uid();
        let code = if let Some(uid) = uid {
            self.account_device_code(&uid)
        } else {
            self.guest_device_code()
        };
        // 保留账号/游客设备标识，只覆盖机型部分；无数盟 ID 时也必须生效。
        let code = device_code_with_profile(&code, &self.effective_device_profile());
        if let Ok(mut auth) = self.auth.write() {
            auth.set_device_code(code.clone());
        }
        if let Ok(mut guard) = self.device_code.write() {
            *guard = code;
        }
    }

    pub(crate) fn current_uid(&self) -> Option<String> {
        self.get_user_cookie()?.split(';').find_map(|kv| {
            let mut parts = kv.trim().splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some("uid"), Some(v)) => {
                    let uid = v.trim().to_string();
                    if uid.is_empty() { None } else { Some(uid) }
                }
                _ => None,
            }
        })
    }

    /// 获取当前用户配置的有效数盟设备 ID（设备ID / ShuzlmID）
    pub fn effective_custom_device_id(&self) -> Option<String> {
        let uid = self.current_uid();
        if let Some(id) = self.generated_identity.read().ok()
            .and_then(|state| state.active_id(uid.as_deref()).map(str::to_owned)) {
            return Some(id);
        }
        let guard = self.device_profile.read().ok()?;
        guard
            .device_id
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// 启动时恢复游客及当前账号的 DUID；登录状态下不自动生成。
    pub async fn initialize_device_identity(&self) -> Result<Value, String> {
        {
            let cached = self.load_accounts_root().get("generatedGuestDeviceId").and_then(Value::as_str)
                .filter(|id| crate::coolapk::shuzilm::valid_duid(id)).map(str::to_owned);
            let mut state = self.generated_identity.write().map_err(|_| "无法读取设备 ID 状态")?;
            state.automatic = true;
            if state.guest_id.is_none() { state.guest_id = cached; }
        }
        self.restore_account_device_identity();
        self.ensure_guest_device_identity().await;
        self.sync_device_code();
        self.get_device_info()
    }

    pub(super) async fn ensure_guest_device_identity(&self) {
        let should_generate = self.generated_identity.read().map(|state|
            state.should_generate_guest(self.current_uid().is_some())).unwrap_or(false);
        if !should_generate { return; }
        // 所有首次游客请求共享一次生成；网络失败后一分钟内不自动重复请求。
        let _generation = self.identity_generation.lock().await;
        let should_generate = self.generated_identity.read().map(|state|
            state.should_generate_guest(self.current_uid().is_some())).unwrap_or(false);
        if !should_generate { return; }
        match crate::coolapk::shuzilm::generate().await {
            Ok(id) => {
                if let Ok(mut state) = self.generated_identity.write() {
                    state.guest_id = Some(id.clone());
                    state.error = None;
                    state.last_failure = None;
                }
                let mut root = self.load_accounts_root();
                root["generatedGuestDeviceId"] = json!(id);
                self.save_accounts_root(&root);
                // 若等待期间发生登录，这里会按当前账号重新选设备，不会启用游客值。
                self.sync_device_code();
            }
            Err(error) => {
                if let Ok(mut state) = self.generated_identity.write() {
                    state.last_failure = Some(std::time::Instant::now());
                    state.error = Some(error);
                }
                log::warn!("device.duid_generation_failed context=guest");
            }
        }
    }

    /// 仅用户点击才为当前登录账号生成并使用；不改写保存的手动设备 ID。
    pub async fn generate_device_identity(&self) -> Result<Value, String> {
        self.generate_device_identity_with(crate::coolapk::shuzilm::generate()).await
    }

    pub(super) async fn generate_device_identity_with(
        &self,
        generation: impl std::future::Future<Output = Result<String, String>>,
    ) -> Result<Value, String> {
        let uid = self.current_uid();
        let epoch = self.generated_identity.read().map_err(|_| "无法读取设备 ID 状态")?.auth_epoch;
        let _generation = self.identity_generation.lock().await;
        let id = generation.await?;
        {
            let mut state = self.generated_identity.write().map_err(|_| "无法读取设备 ID 状态")?;
            if state.auth_epoch != epoch || self.current_uid() != uid {
                return Err("登录状态已变化，生成结果未应用，请重新点击生成".into());
            }
            // 保存成功后才应用，防止显示成功却在重启后丢失。
            let mut root = self.load_accounts_root();
            if let Some(uid) = uid.as_deref() {
                if !root["generatedAccountDeviceIds"].is_object() {
                    root["generatedAccountDeviceIds"] = json!({});
                }
                root["generatedAccountDeviceIds"][uid] = json!(id);
            } else {
                root["generatedGuestDeviceId"] = json!(id);
            }
            self.save_device_identity_root(&root)?;
            if let Some(uid) = uid { state.account_id = Some((uid, id.clone())); }
            else { state.guest_id = Some(id.clone()); }
            state.error = None;
            state.last_failure = None;
        }
        self.sync_device_code();
        self.get_device_info()
    }

    pub fn use_saved_device_identity(&self) -> Result<Value, String> {
        let mut state = self.generated_identity.write().map_err(|_| "无法读取设备 ID 状态")?;
        if let Some(uid) = self.current_uid() {
            let mut root = self.load_accounts_root();
            if let Some(ids) = root.get_mut("generatedAccountDeviceIds").and_then(Value::as_object_mut) {
                if ids.remove(&uid).is_some() { self.save_device_identity_root(&root)?; }
            }
        }
        state.session_changed();
        drop(state);
        self.sync_device_code();
        self.get_device_info()
    }

    pub(super) fn restore_account_device_identity(&self) {
        let uid = self.current_uid();
        let root = self.load_accounts_root();
        let cached = uid.as_deref().and_then(|uid| root.get("generatedAccountDeviceIds")?.get(uid)?.as_str())
            .filter(|id| crate::coolapk::shuzilm::valid_duid(id)).map(str::to_owned);
        if let Ok(mut state) = self.generated_identity.write() {
            if self.current_uid() != uid { return; }
            state.account_id = uid.zip(cached);
        }
    }

    pub(super) fn save_device_identity_root(&self, root: &Value) -> Result<(), String> {
        let path = self.accounts_file_path().ok_or("设备 ID 存储尚未初始化，请重启应用后重试")?;
        let text = serde_json::to_string_pretty(root).map_err(|_| "无法保存设备 ID")?;
        let temporary = path.with_extension("device-id.tmp");
        std::fs::write(&temporary, text).map_err(|_| "无法保存设备 ID，请检查应用数据目录权限".to_string())?;
        std::fs::rename(&temporary, path).map_err(|_| "无法保存设备 ID，请检查应用数据目录权限".to_string())
    }

    /// 获取用户手动填写的 ddid Cookie 值。
    pub(super) fn effective_custom_ddid(&self) -> Option<String> {
        let guard = self.device_profile.read().ok()?;
        guard.ddid.as_ref().map(|value| value.trim().to_string()).filter(|value| !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control))
    }

    /// 在服务端要求 DDI 的写接口上使用设置页单独填写的会话值作为 ddid Cookie。
    pub(super) fn cookie_for_path(&self, cookie: &str, path: &str) -> String {
        let needs_ddid = classify_path(path).needs_ddid;
        let ddid = if needs_ddid { self.effective_custom_ddid() } else { None };
        cookie_for_request(cookie, needs_ddid, ddid.as_deref())
    }

    /// 账号绑定的固定设备码：
    /// 若用户在设置中配置了数盟设备 ID，则优先按该设备 ID 生成标准设备码；
    /// 否则使用 UID 派生算法保持向后兼容。
    pub(super) fn account_device_code(&self, uid: &str) -> String {
        let mut accounts = self.load_accounts();
        let custom_id = self.effective_custom_device_id();
        let code = if let Some(ref dev_id) = custom_id {
            let (model, build) = if let Ok(guard) = self.device_profile.read() {
                (guard.model.clone(), guard.build.clone())
            } else {
                (None, None)
            };
            generate_device_code_with_device_id(dev_id, model.as_deref(), build.as_deref())
        } else {
            generate_device_code_for_id(uid)
        };
        if let Some(pos) = accounts
            .iter()
            .position(|a| a.get("uid").and_then(|v| v.as_str()) == Some(uid))
        {
            if let Some(obj) = accounts[pos].as_object_mut() {
                obj.insert("deviceCode".to_string(), json!(code.clone()));
                if let Some(ref dev_id) = custom_id {
                    obj.insert("deviceId".to_string(), json!(dev_id.clone()));
                }
            }
            self.save_accounts(&accounts);
        }
        // 账户不存在时不写入占位条目：设备码由 uid 确定性推导，无需落盘；
        // 而空凭据条目会污染账户库，让“已保存账户”出现无法登录的幽灵账号。
        code
    }

    /// 游客设备码：配置了数盟设备 ID 时优先使用，未配置时首次随机生成并持久化
    pub(super) fn guest_device_code(&self) -> String {
        let custom_id = self.effective_custom_device_id();
        if let Some(ref dev_id) = custom_id {
            let (model, build) = if let Ok(guard) = self.device_profile.read() {
                (guard.model.clone(), guard.build.clone())
            } else {
                (None, None)
            };
            return generate_device_code_with_device_id(dev_id, model.as_deref(), build.as_deref());
        }
        let mut root = self.load_accounts_root();
        if let Some(code) = root
            .get("guestDeviceCode")
            .and_then(|v| v.as_str())
            .filter(|c| is_valid_device_code(c))
        {
            return code.to_string();
        }
        let code = generate_random_device_code();
        root["guestDeviceCode"] = json!(code.clone());
        self.save_accounts_root(&root);
        code
    }

    /// 将用户自定义设备信息覆盖到请求头（None 字段保留默认值）。
    /// X-App-Device 由当前生效设备码决定（游客随机/账号固定），此处统一写入。
    pub(super) fn apply_device_profile(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, String> {
        let device_code = self
            .device_code
            .read()
            .map_err(|_| "failed to read device code".to_string())?
            .clone();
        self.apply_device_profile_with_code(request, &device_code)
    }

    pub(super) fn apply_device_profile_with_code(
        &self,
        request: reqwest::RequestBuilder,
        device_code: &str,
    ) -> Result<reqwest::RequestBuilder, String> {
        // 固定头走缓存；只有 X-App-Device 随当前设备码按请求变化。
        let mut headers = self.device_headers();
        if let Ok(header_value) = HeaderValue::from_str(device_code) {
            headers.insert("X-App-Device", header_value);
        }
        Ok(request.headers(headers))
    }

    /// 由生效设备信息派生的固定请求头（不含 X-App-Device / X-App-Token）。
    /// 命中缓存时只克隆 HeaderMap，避免每个请求重复合并资料并解析字符串。
    // 缓存仅在自定义设备信息变化时失效（见 update_device_profile）。
    fn device_headers(&self) -> HeaderMap {
        if let Ok(cache) = self.device_headers.read() {
            if let Some(headers) = cache.as_ref() {
                return headers.clone();
            }
        }
        let profile = self.effective_device_profile();
        let mut headers = HeaderMap::new();
        for (header_name, value) in [
            ("X-Sdk-Int", profile.sdk_int.as_ref()),
            ("X-Sdk-Locale", profile.locale.as_ref()),
            ("X-App-Version", profile.app_version.as_ref()),
            ("X-App-Code", profile.app_code.as_ref()),
            ("X-Api-Version", profile.api_version.as_ref()),
            ("X-Dark-Mode", profile.dark_mode.as_ref()),
        ] {
            if let Some(value) = value.filter(|s| !s.trim().is_empty()) {
                if let Ok(header_value) = HeaderValue::from_str(value) {
                    headers.insert(header_name, header_value);
                }
            }
        }
        if let Some(app_code) = profile.app_code.as_ref().filter(|s| !s.trim().is_empty()) {
            if let Ok(header_value) = HeaderValue::from_str(app_code) {
                headers.insert("X-App-Supported", header_value);
            }
        }
        if let Some(ua) = profile.user_agent.as_ref().filter(|s| !s.trim().is_empty()) {
            if let Ok(header_value) = HeaderValue::from_str(ua) {
                headers.insert(USER_AGENT, header_value);
            }
        }
        if let Ok(mut cache) = self.device_headers.write() {
            *cache = Some(headers.clone());
        }
        headers
    }

    pub(super) fn effective_device_profile(&self) -> DeviceProfile {
        let mut profile = self.native_device_profile.clone();
        if let Ok(custom) = self.device_profile.read() {
            macro_rules! override_fields {
                ($($field:ident),*) => { $(if let Some(value) = custom.$field.as_ref().filter(|value| !value.trim().is_empty()) {
                    profile.$field = Some(value.clone());
                })* };
            }
            override_fields!(manufacturer, brand, model, build, device_id, ddid, user_agent,
                sdk_int, locale, app_version, app_code, api_version, dark_mode);
        }
        profile
    }

    pub(super) fn default_publish_device_model(&self) -> bool {
        true
    }

    /// 更新设备信息覆盖配置（由设置页调用；传空字段即恢复默认）
    pub fn update_device_profile(&self, profile: DeviceProfile) {
        if let Ok(mut guard) = self.device_profile.write() {
            *guard = profile;
        }
        // 设备信息变化后固定请求头缓存失效，下个请求重新派生。
        if let Ok(mut cache) = self.device_headers.write() {
            *cache = None;
        }
        self.sync_device_code();
    }

    /// 当前设备信息（设置页展示用）：登录态 + 生效设备码 + 自定义设备 ID
    pub fn get_device_info(&self) -> Result<Value, String> {
        let code = self
            .device_code
            .read()
            .map_err(|_| "failed to read device code".to_string())?
            .clone();
        let logged_in = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .is_some();
        let device_id = self.effective_custom_device_id();
        let uid = self.current_uid();
        let (identity_source, generation_error) = {
            let state = self.generated_identity.read().map_err(|_| "无法读取设备 ID 状态")?;
            let source = if state.active_id(uid.as_deref()).is_some() {
                if uid.is_some() { "generated_account" } else { "generated_guest" }
            } else if device_id.is_some() { "manual" } else { "default" };
            (source, if uid.is_none() { state.error.clone() } else { None })
        };
        Ok(json!({
            "code": 200,
            "data": {
                "loggedIn": logged_in,
                "deviceCode": code,
                "deviceId": device_id,
                "deviceIdSource": identity_source,
                "deviceIdGenerationError": generation_error,
                "defaultProfile": {
                    "source": if self.native_device_profile.model.is_some() { "native" } else { "compatibility" },
                    "platform": std::env::consts::OS,
                    "includeDeviceModel": self.default_publish_device_model(),
                    "manufacturer": self.native_device_profile.manufacturer.as_deref().unwrap_or("Xiaomi"),
                    "brand": self.native_device_profile.brand.as_deref().unwrap_or("Redmi"),
                    "model": self.native_device_profile.model.as_deref().unwrap_or("23113RKC6C"),
                    "build": self.native_device_profile.build.as_deref().unwrap_or("AQ3A.250226.002"),
                    "userAgent": self.native_device_profile.user_agent.as_deref().unwrap_or(crate::coolapk::native_device::COMPATIBILITY_UA),
                    "sdkInt": self.native_device_profile.sdk_int.as_deref().unwrap_or("36")
                }
            }
        }))
    }

    /// 绑定 Cookie 持久化文件路径，并载入上次保存的登录凭据
    /// 凭据统一存 JSON（accounts.json，含全部账户与当前登录 uid），
    /// 兼容迁移旧版 session_cookie.txt。
    pub fn persist_cookie_to(&self, path: PathBuf) {
        {
            let mut guard = match self.cookie_file.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            *guard = Some(path.clone());
        }

        let dir = path.parent().map(|p| p.to_path_buf());
        let accounts_path = dir.as_ref().map(|d| d.join("accounts.json"));
        if let Some(ap) = accounts_path {
            if let Ok(content) = std::fs::read_to_string(&ap) {
                if let Ok(root) = serde_json::from_str::<Value>(&content) {
                    if let Some(validation) = root.get("captchaValidation").and_then(Value::as_str).filter(|value| !value.trim().is_empty()) {
                        if let Ok(mut stored) = self.captcha_validation.write() {
                            *stored = Some(validation.to_string());
                        }
                    }
                    let last_uid = root
                        .get("lastLoginUid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let cookie = root
                        .get("accounts")
                        .and_then(|a| a.as_array())
                        .and_then(|arr| {
                            arr.iter().find(|a| {
                                a.get("uid").and_then(|v| v.as_str()) == Some(last_uid.as_str())
                            })
                        })
                        .and_then(|a| a.get("cookie"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !cookie.is_empty() {
                        if let Ok(mut stored) = self.user_cookie.write() {
                            *stored = Some(cookie);
                        }
                        self.restore_account_device_identity();
                        self.sync_device_code();
                        return;
                    }
                }
            }
        }

        // 旧版 txt 迁移：首次升级时把 txt 中的 cookie 转为 JSON 账户
        if let Ok(content) = std::fs::read_to_string(&path) {
            let content = content.trim().to_string();
            if !content.is_empty() {
                let uid = content
                    .split(';')
                    .find_map(|kv| {
                        let mut parts = kv.trim().splitn(2, '=');
                        match (parts.next(), parts.next()) {
                            (Some("uid"), Some(v)) => Some(v.trim().to_string()),
                            _ => None,
                        }
                    })
                    .unwrap_or_default();
                if !uid.is_empty() {
                    let entry = json!({
                        "uid": uid,
                        "username": "",
                        "userAvatar": "",
                        "cookie": content,
                    });
                    let root = json!({
                        "lastLoginUid": uid,
                        "accounts": [entry],
                    });
                    if let Some(dir) = dir {
                        if let Ok(json_str) = serde_json::to_string_pretty(&root) {
                            let _ = std::fs::write(dir.join("accounts.json"), json_str);
                        }
                    }
                    if let Ok(mut stored) = self.user_cookie.write() {
                        *stored = Some(content);
                    }
                }
                let _ = std::fs::remove_file(&path);
            }
        }
        self.sync_device_code();
    }

    #[allow(dead_code)]
    pub(super) fn save_cookie_file(&self, _cookie: &str) {
        // 凭据统一由 accounts.json 管理，旧 txt 文件不再写入
    }

    /// 读取当前登录凭据（可能为 None）
    pub fn get_user_cookie(&self) -> Option<String> {
        self.user_cookie.read().ok().and_then(|g| g.clone())
    }

    /// 为酷安下载请求补齐官方客户端使用的公共请求头。
    pub fn apply_download_headers(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, String> {
        let request = request
            .header("X-App-Token", self.get_token()?)
            .header("X-Requested-With", "XMLHttpRequest")
            .header("X-Sdk-Int", "36")
            .header("X-Sdk-Locale", "zh-CN")
            .header("X-App-Mode", "universal")
            .header("X-App-Channel", "coolapk")
            .header("X-App-Id", "com.coolapk.market")
            .header("X-App-Version", "16.6.5")
            .header("X-App-Code", APP_CODE)
            .header("X-Api-Version", "16")
            .header("X-App-Supported", APP_CODE)
            .header("X-Dark-Mode", "0");
        self.apply_device_profile(request)
    }

    /// 账户库文件路径（与 session_cookie.txt 同目录，统一 JSON 存储）
    pub(super) fn accounts_file_path(&self) -> Option<std::path::PathBuf> {
        let path = self.cookie_file.read().ok()?.clone()?;
        let dir = path.parent()?.to_path_buf();
        Some(dir.join("accounts.json"))
    }

    /// 读取账户库根对象（{ lastLoginUid, accounts: [...] }）
    pub(super) fn load_accounts_root(&self) -> Value {
        let Some(path) = self.accounts_file_path() else {
            return json!({ "lastLoginUid": "", "accounts": [] });
        };
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<Value>(&c).ok())
            .unwrap_or_else(|| json!({ "lastLoginUid": "", "accounts": [] }))
    }

    pub(super) fn save_accounts_root(&self, root: &Value) {
        if let Some(path) = self.accounts_file_path() {
            if let Ok(json) = serde_json::to_string_pretty(root) {
                let _ = std::fs::write(&path, json);
            }
        }
    }

    /// 读取全部已保存账户（uid/username/userAvatar/cookie）
    pub(super) fn load_accounts(&self) -> Vec<Value> {
        self.load_accounts_root()
            .get("accounts")
            .and_then(|a| a.as_array().cloned())
            .unwrap_or_default()
    }

    pub(super) fn save_accounts(&self, accounts: &[Value]) {
        // 保留 root 上的其他字段（如 guestDeviceCode 游客设备码）
        let mut root = self.load_accounts_root();
        root["accounts"] = Value::Array(accounts.to_vec());
        self.save_accounts_root(&root);
    }

    pub(super) fn set_last_login_uid(&self, uid: &str) {
        let mut root = self.load_accounts_root();
        root["lastLoginUid"] = json!(uid);
        self.save_accounts_root(&root);
    }

    /// 列出已保存的账户（不含 Cookie 原文，仅展示信息）
    pub async fn list_accounts(&self) -> Result<Value, String> {
        let accounts = self.load_accounts();
        let list: Vec<Value> = accounts
            .iter()
            .map(|a| {
                json!({
                    "uid": a.get("uid").cloned().unwrap_or_default(),
                    "username": a.get("username").cloned().unwrap_or_default(),
                    "userAvatar": a.get("userAvatar").cloned().unwrap_or_default(),
                })
            })
            .collect();
        Ok(json!({ "code": 200, "data": list }))
    }

    /// 切换登录到已保存的账户
    pub async fn login_as(&self, uid: &str) -> Result<Value, String> {
        let accounts = self.load_accounts();
        let target = accounts
            .iter()
            .find(|a| a.get("uid").and_then(|v| v.as_str()) == Some(uid))
            .cloned()
            .ok_or_else(|| "未找到该账户的已保存凭据".to_string())?;
        let cookie = target
            .get("cookie")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if cookie.is_empty() {
            return Err("该账户凭据为空".to_string());
        }

        let previous_cookie = self.get_user_cookie();
        let previous_uid = self
            .load_accounts_root()
            .get("lastLoginUid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        self.set_user_cookie(cookie.clone())?;
        match self.check_login_info().await {
            Ok(result) => {
                let data = result.get("data").unwrap_or(&result);
                let validated_uid = data
                    .get("uid")
                    .or_else(|| data.get("id"))
                    .map(value_to_string)
                    .unwrap_or_default();
                if validated_uid != uid {
                    self.restore_login_state(previous_cookie, &previous_uid)?;
                    return Err("保存的账户凭据与目标 UID 不一致，请重新登录该账户".to_string());
                }

                let username = data
                    .get("username")
                    .and_then(Value::as_str)
                    .or_else(|| target.get("username").and_then(Value::as_str))
                    .unwrap_or("");
                let avatar = data
                    .get("userAvatar")
                    .or_else(|| data.get("avatar"))
                    .and_then(Value::as_str)
                    .or_else(|| target.get("userAvatar").and_then(Value::as_str))
                    .unwrap_or("");
                self.save_account(uid, username, avatar, &cookie).await
            }
            Err(error) => {
                self.restore_login_state(previous_cookie, &previous_uid)?;
                Err(format!("切换账户失败，酷安接口未能确认凭据: {error}"))
            }
        }
    }

    pub(super) fn restore_login_state(
        &self,
        previous_cookie: Option<String>,
        previous_uid: &str,
    ) -> Result<(), String> {
        if let Some(cookie) = previous_cookie {
            self.set_user_cookie(cookie)?;
            self.set_last_login_uid(previous_uid);
        } else {
            self.clear_user_cookie()?;
        }
        Ok(())
    }

    /// 保存（或更新）一个账户并切换为当前登录
    pub async fn save_account(
        &self,
        uid: &str,
        username: &str,
        user_avatar: &str,
        cookie: &str,
    ) -> Result<Value, String> {
        if uid.trim().is_empty() || uid == "0" || uid == "10000" {
            return Err("账户 UID 无效".to_string());
        }
        let safe_cookie = Self::sanitize_cookie(cookie);
        if !Self::has_valid_session_cookie(&safe_cookie) {
            return Err("账户凭据缺少有效的 SESSID".to_string());
        }
        let mut accounts = self.load_accounts();
        let entry = json!({
            "uid": uid,
            "username": username,
            "userAvatar": user_avatar,
            "cookie": safe_cookie.clone(),
        });
        if let Some(pos) = accounts
            .iter()
            .position(|a| a.get("uid").and_then(|v| v.as_str()) == Some(uid))
        {
            accounts[pos] = entry;
        } else {
            accounts.push(entry);
        }
        self.save_accounts(&accounts);
        self.set_user_cookie(safe_cookie)?;
        self.set_last_login_uid(uid);
        Ok(json!({
            "code": 200,
            "data": { "uid": uid, "username": username, "userAvatar": user_avatar }
        }))
    }

    /// 将已经通过服务端校验的当前内存会话写入账户库。
    pub async fn persist_current_account(
        &self,
        uid: &str,
        username: &str,
        user_avatar: &str,
    ) -> Result<Value, String> {
        let cookie = self
            .get_user_cookie()
            .ok_or_else(|| "当前没有可持久化的登录凭据".to_string())?;
        self.save_account(uid, username, user_avatar, &cookie).await
    }

    /// 删除一个已保存的账户；若删除的是当前登录账户则同时清空登录态
    pub async fn remove_account(&self, uid: &str) -> Result<Value, String> {
        let last_login_uid = self
            .load_accounts_root()
            .get("lastLoginUid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut accounts = self.load_accounts();
        accounts.retain(|a| a.get("uid").and_then(|v| v.as_str()) != Some(uid));
        self.save_accounts(&accounts);

        let current = self.get_user_cookie().unwrap_or_default();
        let current_uid = current
            .split(';')
            .find_map(|kv| {
                let mut parts = kv.trim().splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some("uid"), Some(v)) => Some(v.trim().to_string()),
                    _ => None,
                }
            })
            .unwrap_or_default();
        if current_uid == uid || last_login_uid == uid {
            self.clear_user_cookie()?;
        }
        Ok(json!({ "code": 200, "data": true }))
    }

    pub fn set_user_cookie(&self, cookie: String) -> Result<(), String> {
        let previous_uid = self.current_uid();
        let safe_ascii = Self::sanitize_cookie(&cookie);

        let mut stored = self
            .user_cookie
            .write()
            .map_err(|_| "failed to lock login state".to_string())?;
        *stored = if safe_ascii.is_empty() {
            None
        } else {
            Some(safe_ascii)
        };
        drop(stored);
        if previous_uid != self.current_uid() {
            if let Ok(mut state) = self.generated_identity.write() { state.session_changed(); }
            self.restore_account_device_identity();
        }
        // 登录态变化（登录/登出/切换账号）后同步设备码
        self.sync_device_code();
        Ok(())
    }

    pub(super) fn sanitize_cookie(cookie: &str) -> String {
        let clean = cookie
            .replace('\r', "")
            .replace('\n', " ")
            .trim()
            .to_string();
        // 转换非 ASCII 字符，防止 reqwest 构造 HeaderValue 出现 builder error
        clean
            .chars()
            .map(|c| {
                if c.is_ascii() && c != '\r' && c != '\n' {
                    c.to_string()
                } else {
                    format!("%{:02X}", c as u32)
                }
            })
            .collect()
    }

    /// 校验 Cookie 是否包含真实有效的 SESSID 会话
    /// （"deleted"/"expired" 等占位值视为无效）
    pub fn has_valid_session_cookie(cookie: &str) -> bool {
        cookie.split(';').any(|item| {
            let mut parts = item.trim().splitn(2, '=');
            matches!(
                (parts.next(), parts.next()),
                (Some("SESSID"), Some(value))
                    if !value.trim().is_empty()
                        && !value.eq_ignore_ascii_case("deleted")
                        && !value.eq_ignore_ascii_case("expired")
            )
        })
    }

    pub(super) async fn request_api(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
    ) -> Result<Value, String> {
        self.request_api_from("https://api.coolapk.com", method, path, query, form)
            .await
    }

    pub(super) async fn request_api_from(
        &self,
        api_origin: &str,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
    ) -> Result<Value, String> {
        self.request_api_from_with_validation(api_origin, method, path, query, form, None).await
    }

    pub(super) fn validation_for_request(&self, validation: Option<&str>) -> Result<Option<String>, String> {
        let mut stored = self.captcha_validation.write().map_err(|_| "无法读取验证码凭证".to_string())?;
        if let Some(validation) = validation.filter(|value| !value.trim().is_empty()) {
            if stored.as_deref() != Some(validation) {
                *stored = Some(validation.to_string());
                let mut root = self.load_accounts_root();
                root["captchaValidation"] = json!(validation);
                self.save_accounts_root(&root);
            }
        }
        Ok(stored.clone())
    }

    pub(super) async fn request_api_from_with_validation(
        &self,
        api_origin: &str,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        form: Option<&[(&str, String)]>,
        validation: Option<&str>,
    ) -> Result<Value, String> {
        // 该方法只供内部固定 API 主机调用，禁止把登录凭据发送到其他域名。
        if api_origin != "https://api.coolapk.com" && api_origin != "https://api2.coolapk.com" {
            return Err("不受信任的酷安 API 主机".to_string());
        }
        self.ensure_guest_device_identity().await;
        let validation = self.validation_for_request(validation)?;
        let started = std::time::Instant::now();
        let log_path = path.split('?').next().unwrap_or(path);
        let method_name = method.as_str().to_string();
        let token = self.get_token()?;
        let url = format!("{api_origin}{path}");
        let requested_with = "XMLHttpRequest";
        let mut request = self.apply_device_profile(
            self.api_client()?
                .request(method, url)
                .header("X-App-Token", token)
                .header("X-Requested-With", requested_with)
                .query(query),
        )?;

        let cookie = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .clone();
        let mut cookie_attached = false;
        if cookie.is_some() || validation.is_some() {
            let full_cookie = self.cookie_for_path(cookie.as_deref().unwrap_or(""), path);
            let full_cookie = cookie_with_captcha_validation(&full_cookie, validation.as_deref());
            if let Ok(header_val) = reqwest::header::HeaderValue::from_str(&full_cookie) {
                request = request.header(COOKIE, header_val);
                cookie_attached = true;
            }
        }
        log::debug!(
            "api.request method={} path={} cookie_attached={}",
            method_name,
            log_path,
            cookie_attached
        );
        if let Some(form) = form {
            request = request.form(form);
        }

        let response = request.send().await.map_err(|error| {
            log_api_failure_limited("api.transport_failed", &method_name, api_origin, log_path,
                if error.is_timeout() { "timeout" } else { "transport" },
                &format!("timeout={} elapsed_ms={} cause={}", error.is_timeout(), started.elapsed().as_millis(), error_cause_chain(&error)));
            error.to_string()
        })?;
        let status = response.status();
        let result = response_json(response).await;
        match &result {
            Err(error) => log_api_failure_limited("api.response_failed", &method_name, api_origin, log_path, classify_api_failure(error),
                &format!("status={} elapsed_ms={}", status.as_u16(), started.elapsed().as_millis())),
            Ok(value) => {
                if let Some((code, reason)) = application_failure(value) {
                    log_api_failure_limited("api.application_failed", &method_name, api_origin, log_path, reason,
                        &format!("status={} code={}", status.as_u16(), code));
                } else {
                    log::debug!("api.response_ok method={} path={} status={} elapsed_ms={}",
                        method_name, log_path, status.as_u16(), started.elapsed().as_millis());
                }
            }
        }
        result
    }

    pub(super) async fn api_get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        self.request_api(Method::GET, path, query, None).await
    }

    pub(super) async fn api_get_for_list(
        &self,
        path: &str,
        query: &[(&str, String)],
        guest: bool,
    ) -> Result<Value, String> {
        if guest {
            self.public_api_get_from("https://api.coolapk.com", path, query)
                .await
        } else {
            self.api_get(path, query).await
        }
    }

    pub(super) fn public_api_request(
        &self,
        api_origin: &str,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<reqwest::RequestBuilder, String> {
        if api_origin != "https://api.coolapk.com" && api_origin != "https://api2.coolapk.com" {
            return Err("不受信任的酷安 API 主机".to_string());
        }
        // 公开内容只读回退使用本机持久化的游客设备码，不携带账号 Cookie。
        // 这样既能避开登录设备触发的 -415，也不会改变已绑定账号的写操作指纹。
        let public_device_code = device_code_with_profile(&self.guest_device_code(), &self.effective_device_profile());
        let public_token = CoolapkAuth::new(public_device_code.clone()).get_app_token_with_app_code(self.request_app_code()?)?;
        // 与官方统一请求头逻辑一致：签名和实际发送的版本、设备信息必须配套。
        // 不能只覆盖 Token/Device，否则自定义版本的签名会搭配客户端默认版本头。
        self.apply_device_profile_with_code(
            self.api_client()?
                .get(format!("{api_origin}{path}"))
                .header("X-App-Token", public_token)
                .header("X-Requested-With", "XMLHttpRequest")
                .query(query),
            &public_device_code,
        )
    }

    pub(super) async fn public_api_get_from(
        &self,
        api_origin: &str,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Value, String> {
        let started = std::time::Instant::now();
        let log_path = path.split('?').next().unwrap_or(path);
        let request = self.public_api_request(api_origin, path, query)?;
        log::debug!("api.guest_request path={} cookie_attached=false", log_path);
        let response = request
            .send()
            .await
            .map_err(|error| {
                log_api_failure_limited("api.guest_transport_failed", "GET", api_origin, log_path,
                    if error.is_timeout() { "timeout" } else { "transport" },
                    &format!("timeout={} elapsed_ms={} cause={}", error.is_timeout(), started.elapsed().as_millis(), error_cause_chain(&error)));
                error.to_string()
            })?;
        let status = response.status();
        let result = response_json(response).await;
        match &result {
            Err(error) => log_api_failure_limited("api.guest_response_failed", "GET", api_origin, log_path, classify_api_failure(error),
                &format!("status={} app_code={} elapsed_ms={}", status.as_u16(), self.request_app_code().unwrap_or(APP_CODE), started.elapsed().as_millis())),
            Ok(value) => {
                if let Some((code, reason)) = application_failure(value) {
                    log_api_failure_limited("api.guest_application_failed", "GET", api_origin, log_path, reason,
                        &format!("status={} app_code={} code={}", status.as_u16(), self.request_app_code().unwrap_or(APP_CODE), code));
                } else {
                    log::debug!("api.guest_response_ok path={} status={} elapsed_ms={}",
                        log_path, status.as_u16(), started.elapsed().as_millis());
                }
            }
        }
        result
    }

    pub(super) async fn api_post(
        &self,
        path: &str,
        query: &[(&str, String)],
        form: &[(&str, String)],
    ) -> Result<Value, String> {
        self.request_api(Method::POST, path, query, Some(form))
            .await
    }

    /// 发送带登录态的 multipart 写请求。头像接口使用 multipart，不能复用普通表单请求。
    pub(super) async fn request_multipart_api(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> Result<Value, String> {
        let token = self.get_token()?;
        let url = format!("https://api.coolapk.com{path}");
        let mut request = self.apply_device_profile(
            self.api_client()?
                .request(Method::POST, url)
                .header("X-App-Token", token)
                .header("X-Requested-With", "XMLHttpRequest")
                .multipart(form),
        )?;

        let cookie = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .clone();
        if let Some(cookie) = cookie {
            let full_cookie = self.cookie_for_path(&cookie, path);
            if let Ok(header_val) = HeaderValue::from_str(&full_cookie) {
                request = request.header(COOKIE, header_val);
            }
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        wrap_api_data(response_json(response).await?)
    }
}
