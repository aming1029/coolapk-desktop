use super::*;

impl CoolapkClient {
    /// 用户黑名单（需登录）
    /// 数据来源: GET /v6/user/blackList?page={page}
    pub async fn get_black_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/user/blackList", &[("page", page.to_string())])
            .await?;
        Self::wrap_user_list_result(raw, "获取黑名单失败")
    }

    /// 用户屏蔽列表（需登录）
    /// 数据来源: GET /v6/user/ignoreList?page={page}
    pub async fn get_ignore_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/user/ignoreList", &[("page", page.to_string())])
            .await?;
        Self::wrap_user_list_result(raw, "获取屏蔽列表失败")
    }

    /// 受限用户列表（需登录）
    /// 数据来源: GET /v6/user/limitList?page={page}
    pub async fn get_limit_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/user/limitList", &[("page", page.to_string())])
            .await?;
        Self::wrap_user_list_result(raw, "获取受限列表失败")
    }

    /// 黑名单/屏蔽列表数据为「用户实体」而非 Feed，不能走 clean_single_feed
    ///（该函数会因缺少 message/title/pic 把所有用户卡片丢弃，导致列表恒为空）。
    /// 这里仅解包外层 data 并展开可能的 card 包装实体，保留用户卡片原始字段。
    pub(super) fn extract_user_list(json_data: &Value) -> Vec<Value> {
        let mut users = Vec::new();
        if let Some(data_arr) = json_data.get("data").and_then(|v| v.as_array()) {
            for item in data_arr.iter() {
                if let Some(entities) = item.get("entities").and_then(|v| v.as_array()) {
                    users.extend(entities.iter().cloned());
                } else {
                    users.push(item.clone());
                }
            }
        }
        users
    }

    pub(super) fn wrap_user_list_result(raw: Value, fail_msg: &str) -> Result<Value, String> {
        if let Some(status) = raw.get("status").and_then(|v| v.as_i64()) {
            if status < 0 {
                let msg = raw
                    .get("message")
                    .or_else(|| raw.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or(fail_msg);
                return Err(msg.to_string());
            }
        }
        Ok(json!({ "code": 200, "data": Self::extract_user_list(&raw) }))
    }

    /// 拉黑/移出黑名单（需登录，GET 写接口）
    /// 实测：POST 返回 404 请求方式错误，v6 写接口一律 GET + uid 查询参数。
    pub(super) async fn blacklist_action(&self, path: &str, uid: &str) -> Result<Value, String> {
        wrap_api_data(self.api_get(path, &[("uid", uid.to_string())]).await?)
    }

    pub async fn add_to_black_list(&self, uid: &str) -> Result<Value, String> {
        self.blacklist_action("/v6/user/addToBlackList", uid).await
    }

    pub async fn remove_from_black_list(&self, uid: &str) -> Result<Value, String> {
        self.blacklist_action("/v6/user/removeFromBlackList", uid)
            .await
    }

    /// 屏蔽/取消屏蔽用户（需登录，GET 写接口）
    pub async fn add_to_ignore_list(&self, uid: &str) -> Result<Value, String> {
        self.blacklist_action("/v6/user/addToIgnoreList", uid).await
    }

    pub async fn remove_from_ignore_list(&self, uid: &str) -> Result<Value, String> {
        self.blacklist_action("/v6/user/removeFromIgnoreList", uid)
            .await
    }

    /// 应用下载链接
    /// 数据来源: GET /v6/apk/url?id={packageName}
    pub async fn get_apk_url(&self, package_name: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/apk/url", &[("id", package_name.to_string())])
                .await?,
        )
    }

    /// 下载校验接口：对应 APK 的 POST /v6/apk/downloadVerify。
    /// 官方下载器在拿到最终响应地址后调用该接口，用于识别被劫持的下载页面。
    pub async fn verify_apk_download(
        &self,
        apk_name: &str,
        request_url: &str,
        download_url: &str,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/apk/downloadVerify",
                &[],
                &[
                    ("apkName", apk_name.to_string()),
                    ("requestUrl", request_url.to_string()),
                    ("downloadUrl", download_url.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 应用二维码
    /// 数据来源: GET /v6/apk/qr?id={packageName}
    pub async fn get_apk_qr(&self, package_name: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/apk/qr", &[("id", package_name.to_string())])
                .await?,
        )
    }

    /// 点赞/取消点赞通用实现（对应官方 APK kb1.java）
    pub(super) async fn like_action(&self, path: &str, id: &str) -> Result<Value, String> {
        let res = self
            .api_post(
                path,
                &[("id", id.to_string()), ("detail", "0".to_string())],
                &[("trace", "".to_string())],
            )
            .await?;
        if let Some(status) = res.get("status").and_then(|v| v.as_i64()) {
            if status == 401 || status == 403 {
                let msg = res
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("请先登录后再点赞")
                    .to_string();
                return Err(format!("{msg}（当前未登录或登录已失效）"));
            }
            if status < 0 {
                let msg = res
                    .get("message")
                    .or_else(|| res.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("点赞失败")
                    .to_string();
                return Err(msg);
            }
        }
        wrap_api_data(res)
    }

    pub async fn like_feed(&self, feed_id: &str) -> Result<Value, String> {
        self.like_action("/v6/feed/like", feed_id).await
    }

    pub async fn unlike_feed(&self, feed_id: &str) -> Result<Value, String> {
        self.like_action("/v6/feed/unlike", feed_id).await
    }

    /// 点赞评论（对应 APK /v6/feed/likeReply）
    pub async fn like_reply(&self, reply_id: &str) -> Result<Value, String> {
        self.like_action("/v6/feed/likeReply", reply_id).await
    }

    /// 取消点赞评论（对应 APK /v6/feed/unLikeReply）
    pub async fn unlike_reply(&self, reply_id: &str) -> Result<Value, String> {
        self.like_action("/v6/feed/unLikeReply", reply_id).await
    }

    /// 发表评论；rid 非空时表示回复某条评论，pic 非空时表示评论图片，post_token 为网易易盾滑块验证 Token。
    ///
    /// 官方 APK 回复评论时并不是把 rid 作为表单字段发送，而是将目标评论 ID
    /// 放到查询参数 id，并把 type 设为 reply；只有直接评论动态时才使用动态 ID + type=feed。
    /// 对应官方 APK kb1.java:496 (@POST("feed/reply") @Query("id") @Query("type") @Body FormBody)。
    pub async fn reply_feed(
        &self,
        feed_id: &str,
        message: &str,
        rid: Option<&str>,
        pic: Option<&str>,
        post_token: Option<&str>,
    ) -> Result<Value, String> {
        // reply 在 `PostToken.List` 内，官方建议携带网易易盾 _v2_post_token。
        // 实测服务端对该字段并非强制，token 为可选：提供则附加，缺失仍正常提交。
        let (target_id, reply_type) = reply_target_params(feed_id, rid);
        let query = [("id", target_id), ("type", reply_type)];
        let mut form = vec![
            ("message", message.to_string()),
        ];
        if let Some(pic) = pic {
            if !pic.is_empty() {
                form.push(("pic", pic.to_string()));
            }
        }
        if let Some(token) = post_token {
            if !token.is_empty() {
                form.push(("_v2_post_token", token.to_string()));
            }
        }
        wrap_api_data(self.api_post("/v6/feed/reply", &query, &form).await?)
    }

    /// 发表评论到应用评价区；此接口与动态评论 /v6/feed/reply 的语义不同。
    /// 对应公开 V6 接口资料中的 POST /v6/apk/comment?id={id}，正文使用 message 表单字段。
    pub async fn comment_apk(&self, app_id: &str, message: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/apk/comment",
                &[("id", app_id.to_string())],
                &[("message", message.to_string())],
            )
            .await?,
        )
    }

    pub async fn follow_user(&self, uid: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post("/v6/user/follow", &[("uid", uid.to_string())], &[])
                .await?,
        )
    }

    pub async fn unfollow_user(&self, uid: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/user/unfollow", &[("uid", uid.to_string())])
                .await?,
        )
    }

    pub async fn special_follow_user(&self, uid: &str, special: bool) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/user/specialFollowUser",
                &[
                    ("uid", uid.to_string()),
                    ("special", if special { "1" } else { "0" }.to_string()),
                ],
                &[],
            )
            .await?,
        )
    }

    pub async fn cancel_follower(&self, uid: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post("/v6/user/cancelFollower", &[("uid", uid.to_string())], &[])
                .await?,
        )
    }

    pub async fn update_user_remark(&self, uid: &str, name: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/user/updateRemark",
                &[],
                &[("uid", uid.to_string()), ("name", name.to_string())],
            )
            .await?,
        )
    }

    // 官方关注页的横向入口卡片必须完整保留，不能按动态清洗或展开成推荐用户。
    pub(super) fn extract_following_list(raw: &Value) -> Vec<Value> {
        let mut rows = Vec::new();
        if let Some(items) = raw.get("data").and_then(Value::as_array) {
            for item in items {
                if item.get("entityTemplate").and_then(Value::as_str) == Some("iconScrollCard") {
                    rows.push(item.clone());
                } else {
                    rows.extend(Self::extract_cleaned_list(&json!({ "data": [item] })));
                }
            }
        }
        rows
    }

    // 仅有官方头像栏不代表已经取得动态；保留首个卡片并继续原有接口回退。
    pub(super) fn merge_following_page(rows: &mut Vec<Value>, raw: &Value) -> bool {
        let incoming = Self::extract_following_list(raw);
        let mut has_feed = false;
        for item in incoming {
            if item.get("entityTemplate").and_then(Value::as_str) == Some("iconScrollCard") {
                if !rows.iter().any(|row| row.get("entityTemplate").and_then(Value::as_str) == Some("iconScrollCard")) { rows.push(item); }
            } else {
                has_feed = true;
                rows.push(item);
            }
        }
        has_feed
    }

    pub async fn get_following_feeds(&self, page: u32) -> Result<Value, String> {
        let mut rows = Vec::new();
        // 1. APK 当前首页使用 V15_HOME_TAB_FOLLOW，优先走同一条动态接口。
        if let Ok(raw) = self
            .api_get(
                "/v6/page/dataList",
                &[
                    ("url", "V15_HOME_TAB_FOLLOW".to_string()),
                    ("title", "关注".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await
        {
            if Self::merge_following_page(&mut rows, &raw) {
                return Ok(json!({ "code": 200, "data": rows }));
            }
        }

        // 2. 兼容旧版 V9 首页的 page/dataList 关注流接口。
        if let Ok(raw) = self
            .api_get(
                "/v6/page/dataList",
                &[
                    ("url", "/user/followFeedList".to_string()),
                    ("title", "关注".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await
        {
            if Self::merge_following_page(&mut rows, &raw) {
                return Ok(json!({ "code": 200, "data": rows }));
            }
        }

        // 3. 备用尝试 /v6/feed/followFeedList 关注流接口
        if let Ok(raw) = self
            .api_get("/v6/feed/followFeedList", &[("page", page.to_string())])
            .await
        {
            if Self::merge_following_page(&mut rows, &raw) {
                return Ok(json!({ "code": 200, "data": rows }));
            }
        }

        // 4. 备用尝试主页关注页接口 /v6/main/indexV8?type=follow
        let raw = self
            .api_get(
                "/v6/main/indexV8",
                &[("type", "follow".to_string()), ("page", page.to_string())],
            )
            .await?;
        Self::merge_following_page(&mut rows, &raw);
        Ok(json!({ "code": 200, "data": rows }))
    }

    pub async fn get_follow_user_list(&self, uid: &str, page: u32, category: Option<&str>) -> Result<Value, String> {
        // 与官方酷友列表保持相同的服务端分类，避免仅筛选已加载的第一页。
        let mut query = vec![("uid", uid.to_string()), ("page", page.to_string())];
        match category.unwrap_or("follow") {
            "follow" => { query.push(("isSpecialFollow", "-1".to_string())); query.push(("isFriend", "0".to_string())); }
            "special" => query.push(("isSpecialFollow", "1".to_string())),
            "friend" => query.push(("isFriend", "1".to_string())),
            _ => return Err("无效的关注列表分类".to_string()),
        }
        let raw = self
            .api_get("/v6/user/followList", &query)
            .await?;

        let list = raw.get("data").cloned().unwrap_or(Value::Array(Vec::new()));
        let mut clean_list = Vec::new();
        if let Some(arr) = list.as_array() {
            let self_uid = uid.trim().to_string();
            for item in arr {
                let user_info = item.get("fUserInfo").or_else(|| item.get("userInfo"));
                let real_uid = user_info
                    .and_then(|info| info.get("uid"))
                    .and_then(|v| value_to_string_opt(v))
                    .or_else(|| item.get("fuid").and_then(|v| value_to_string_opt(v)))
                    .unwrap_or_default();

                if real_uid.is_empty() || real_uid == self_uid {
                    continue;
                }

                let username = user_info
                    .and_then(|info| info.get("username").or_else(|| info.get("displayUserName")))
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("fusername").and_then(|v| v.as_str()))
                    .unwrap_or("酷友");

                let avatar = user_info
                    .and_then(|info| info.get("userAvatar").or_else(|| info.get("avatar")))
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("fUserAvatar").and_then(|v| v.as_str()))
                    .unwrap_or("");

                let bio = user_info
                    .and_then(|info| info.get("bio").or_else(|| info.get("signature")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let is_follow = user_info
                    .and_then(|info| info.get("isFollow"))
                    .or_else(|| item.get("isFollow"))
                    .cloned()
                    .unwrap_or(json!(1));

                let is_special_follow = user_info
                    .and_then(|info| info.get("isSpecialFollow"))
                    .or_else(|| item.get("isSpecialFollow"))
                    .cloned()
                    .unwrap_or(json!(0));

                let level = user_info
                    .and_then(|info| info.get("level"))
                    .or_else(|| item.get("level"))
                    .cloned()
                    .unwrap_or(json!(0));

                let mut new_item = item.clone();
                if let Some(obj) = new_item.as_object_mut() {
                    obj.insert("uid".to_string(), json!(real_uid));
                    obj.insert("fuid".to_string(), json!(real_uid));
                    obj.insert("username".to_string(), json!(username));
                    obj.insert("fusername".to_string(), json!(username));
                    obj.insert("userAvatar".to_string(), json!(avatar));
                    obj.insert("fUserAvatar".to_string(), json!(avatar));
                    obj.insert("bio".to_string(), json!(bio));
                    obj.insert("signature".to_string(), json!(bio));
                    obj.insert("isFollow".to_string(), is_follow);
                    obj.insert("isSpecialFollow".to_string(), is_special_follow);
                    obj.insert("level".to_string(), level);
                }
                clean_list.push(new_item);
            }
        }

        Ok(json!({ "code": 200, "data": clean_list }))
    }

    /// 获取粉丝列表。
    /// 注意：酷安 /v6/user/fansList 返回数据中，真实粉丝信息在 `userInfo` 字段，
    /// 而 `fUserInfo`/`fuid`/`fusername` 是"自己"的占位数据。
    /// 这里用 userInfo 重写顶层字段并剔除占位，保证前端渲染的是真实粉丝。
    pub async fn get_fans_user_list(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/fansList",
                &[
                    ("uid", uid.to_string()),
                    ("page", page.to_string()),
                    ("isIncludeTop", "1".to_string()),
                ],
            )
            .await?;

        let list = raw.get("data").cloned().unwrap_or(Value::Array(Vec::new()));
        let mut clean_list = Vec::new();
        if let Some(arr) = list.as_array() {
            let self_uid = uid.trim().to_string();
            for item in arr {
                let user_info = item.get("userInfo").or_else(|| item.get("fUserInfo"));
                let real_uid = user_info
                    .and_then(|info| info.get("uid"))
                    .and_then(|v| value_to_string_opt(v))
                    .or_else(|| item.get("uid").and_then(|v| value_to_string_opt(v)))
                    .unwrap_or_default();

                // 剔除占位数据：真实 uid 为空 或 等于请求者自己
                if real_uid.is_empty() || real_uid == self_uid {
                    continue;
                }

                // 用 userInfo 重写顶层字段，前端可直接读取
                let username = user_info
                    .and_then(|info| info.get("username").or_else(|| info.get("displayUserName")))
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("username").and_then(|v| v.as_str()))
                    .unwrap_or("酷友");
                let avatar = user_info
                    .and_then(|info| info.get("userAvatar").or_else(|| info.get("avatar")))
                    .and_then(|v| v.as_str())
                    .or_else(|| item.get("userAvatar").and_then(|v| v.as_str()))
                    .unwrap_or("");

                let bio = user_info
                    .and_then(|info| info.get("bio").or_else(|| info.get("signature")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let is_follow = user_info
                    .and_then(|info| info.get("isFollow"))
                    .or_else(|| item.get("isFollow"))
                    .cloned()
                    .unwrap_or(json!(0));

                let is_special_follow = user_info
                    .and_then(|info| info.get("isSpecialFollow"))
                    .or_else(|| item.get("isSpecialFollow"))
                    .cloned()
                    .unwrap_or(json!(0));

                let level = user_info
                    .and_then(|info| info.get("level"))
                    .or_else(|| item.get("level"))
                    .cloned()
                    .unwrap_or(json!(0));

                let mut new_item = item.clone();
                if let Some(obj) = new_item.as_object_mut() {
                    obj.insert("uid".to_string(), json!(real_uid));
                    obj.insert("fuid".to_string(), json!(real_uid));
                    obj.insert("username".to_string(), json!(username));
                    obj.insert("fusername".to_string(), json!(username));
                    obj.insert("userAvatar".to_string(), json!(avatar));
                    obj.insert("fUserAvatar".to_string(), json!(avatar));
                    obj.insert("bio".to_string(), json!(bio));
                    obj.insert("signature".to_string(), json!(bio));
                    obj.insert("isFollow".to_string(), is_follow);
                    obj.insert("isSpecialFollow".to_string(), is_special_follow);
                    obj.insert("level".to_string(), level);
                }
                clean_list.push(new_item);
            }
        }

        Ok(json!({ "code": 200, "data": clean_list }))
    }
}
