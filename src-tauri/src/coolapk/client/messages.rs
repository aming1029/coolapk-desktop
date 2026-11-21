use super::*;

impl CoolapkClient {
    pub async fn get_notification_count(&self) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/notification/checkCount", &[]).await?)
    }

    /// 官方个人收藏表情列表（APK 使用 group=default）。
    pub async fn get_emoji_pictures(&self) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/emojiPicture/list", &[("group", "default".to_string())]).await?)
    }

    pub async fn add_emoji_picture(&self, url: &str) -> Result<Value, String> {
        let parsed = reqwest::Url::parse(url).map_err(|_| "表情图片地址无效".to_string())?;
        if !matches!(parsed.scheme(), "http" | "https") || !is_coolapk_host(parsed.host_str().unwrap_or_default()) {
            return Err("表情图片必须先上传到酷安".to_string());
        }
        wrap_api_data(self.api_post("/v6/emojiPicture/addEmoji", &[], &[("group", "default".to_string()), ("url", url.to_string())]).await?)
    }

    pub async fn edit_emoji_pictures(&self, action: &str, ids: &[String]) -> Result<Value, String> {
        if !matches!(action, "delete" | "sort") || ids.is_empty() || ids.iter().any(|id| id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) || id == "0") {
            return Err("表情管理参数无效".to_string());
        }
        wrap_api_data(self.api_post("/v6/emojiPicture/edit", &[], &[("group", "default".to_string()), ("action", action.to_string()), ("id", ids.join(","))]).await?)
    }

    /// 清除服务端通知数。对应 APK 的 POST /v6/notification/clearCount?type={type}。
    pub async fn clear_notification_count(&self, notification_type: &str) -> Result<Value, String> {
        wrap_api_data(
            self.request_api(
                Method::POST,
                "/v6/notification/clearCount",
                &[("type", notification_type.to_string())],
                None,
            )
            .await?,
        )
    }

    /// 官方批量删除只传 notification，清理通知列表。
    pub async fn delete_all_notifications(&self) -> Result<Value, String> {
        wrap_api_data(self.api_post("/v6/notificationV18/deleteAll", &[], &[("type", "notification".to_string())]).await?)
    }

    /// 与 APK NotificationReportUtils 一致，只上报携带任务标识的通知点击。
    pub async fn report_notification_click(&self, task_id: &str) -> Result<Value, String> {
        wrap_api_data(self.api_post("/v6/notificationV18/reportPushTaskClick", &[], &[("task_id", task_id.to_string()), ("source", "2".to_string())]).await?)
    }

    /// 对齐 APK AppLinkActivity，应用内点击不携带系统通知来源 source=1。
    pub async fn read_notification(&self, id: &str, notification_type: &str) -> Result<Value, String> {
        if id.trim().is_empty() || notification_type.trim().is_empty() { return Err("缺少通知已读参数".to_string()); }
        wrap_api_data(self.api_post("/v6/notificationV18/read", &[], &[("id", id.trim().to_string()), ("type", notification_type.trim().to_string())]).await?)
    }

    pub async fn get_notifications(
        &self,
        notification_type: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        note_type: &str,
    ) -> Result<Value, String> {
        // APK 普通通知统一走 v18，点赞/关注仍使用各自专用列表；旧分类入口归并到通知。
        let path = match notification_type {
            "feedLikeList" | "like" | "feedlike" => "/v6/notification/feedLikeList",
            "contactsFollowList" => "/v6/notification/contactsFollowList",
            "subList" => "/v6/notificationV18/subList",
            _ => "/v6/notificationV18/list",
        };
        let mut query = vec![("page", page.max(1).to_string())];
        if !first_item.is_empty() { query.push(("firstItem", first_item.to_string())); }
        if !last_item.is_empty() { query.push(("lastItem", last_item.to_string())); }
        if notification_type == "subList" {
            if note_type.trim().is_empty() { return Err("缺少通知分组类型".to_string()); }
            query.push(("noteType", note_type.to_string()));
        }
        wrap_api_data(
            self.api_get(path, &query).await?,
        )
    }

    pub async fn list_messages(&self, page: u32, first_item: &str, last_item: &str) -> Result<Value, String> {
        let mut query = vec![("page", page.to_string())];
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.trim().to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.trim().to_string()));
        }
        wrap_api_data(
            self.api_get("/v6/message/list", &query)
                .await?,
        )
    }

    /// 最近联系人
    /// 数据来源: GET /v6/message/recentChatUser
    pub async fn get_recent_chat_users(&self, page: u32) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/message/recentChatUser", &[("page", page.to_string())])
                .await?,
        )
    }

    /// 获取私信记录。APK 向上翻页时会同时携带当前最早消息的 entityId 作为 firstItem。
    pub async fn list_chat_history(&self, ukey: &str, page: u32, first_item: &str, last_item: &str) -> Result<Value, String> {
        let mut query = vec![("ukey", ukey.to_string()), ("page", page.to_string())];
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.trim().to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.trim().to_string()));
        }
        wrap_api_data(self.api_get("/v6/message/chat", &query).await?)
    }

    /// 删除私信会话（需登录）
    /// 数据来源: GET /v6/message/deleteChat?ukey={ukey}
    pub async fn delete_message_chat(&self, ukey: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/message/deleteChat", &[("ukey", ukey.to_string())])
                .await?,
        )
    }

    /// 发送私信（需登录）
    ///
    /// 私信沿用 v1.9.1 及更早版本的兼容签名：旧版 Token cost=10、当前
    /// 使用设置中的数盟 ID 更新 DDI 写请求 Cookie。请求体仍按当前
    /// APK 的 `message/send` 契约发送 multipart，并补齐 `quick_reply=1`、
    /// 空图片和空扩展字段。
    pub async fn send_private_message(&self, uid: &str, message: &str) -> Result<Value, String> {
        let uid = uid.trim();
        if uid.is_empty() || !uid.chars().all(|ch| ch.is_ascii_digit()) {
            return Err("目标用户 UID 格式无效".to_string());
        }
        if message.trim().is_empty() {
            return Err("私信内容不能为空".to_string());
        }

        let token = self.get_token()?;
        let mut url = reqwest::Url::parse("https://api.coolapk.com/v6/message/send")
            .map_err(|e| format!("私信接口地址无效: {e}"))?;
        url.query_pairs_mut()
            .append_pair("uid", uid)
            .append_pair("quick_reply", "1");
        let form = reqwest::multipart::Form::new()
            .text("message", message.to_string())
            .text("message_pic", "")
            .text("message_extra", "");

        let mut request = self.apply_device_profile(
            self.api_client()?
                .request(reqwest::Method::POST, url)
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
            let full_cookie = self.cookie_for_path(&cookie, "/v6/message/send");
            if let Ok(header_val) = reqwest::header::HeaderValue::from_str(&full_cookie) {
                request = request.header(COOKIE, header_val);
            }
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        wrap_api_data(response_json(response).await?)
    }

    /// 发送图片私信（需登录）
    /// 与 send_private_message 相同接口，multipart 字段为 message_pic。
    pub async fn send_private_image(&self, uid: &str, message_pic: &str) -> Result<Value, String> {
        let uid = uid.trim();
        if uid.is_empty() || !uid.chars().all(|ch| ch.is_ascii_digit()) {
            return Err("目标用户 UID 格式无效".to_string());
        }
        if message_pic.trim().is_empty() {
            return Err("私信图片地址不能为空".to_string());
        }

        let token = self.get_token()?;
        let mut url = reqwest::Url::parse("https://api.coolapk.com/v6/message/send")
            .map_err(|e| format!("私信接口地址无效: {e}"))?;
        url.query_pairs_mut()
            .append_pair("uid", uid)
            .append_pair("quick_reply", "1");
        let form = reqwest::multipart::Form::new()
            .text("message", "")
            .text("message_pic", message_pic.to_string())
            .text("message_extra", "");

        let mut request = self.apply_device_profile(
            self.api_client()?
                .request(reqwest::Method::POST, url)
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
            let full_cookie = self.cookie_for_path(&cookie, "/v6/message/send");
            if let Ok(header_val) = reqwest::header::HeaderValue::from_str(&full_cookie) {
                request = request.header(COOKIE, header_val);
            }
        }

        let response = request.send().await.map_err(|e| e.to_string())?;
        wrap_api_data(response_json(response).await?)
    }

    /// 标记私信会话已读（需登录）
    /// 数据来源: GET /v6/message/read?ukey={ukey}
    pub async fn read_message(&self, ukey: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/message/read", &[("ukey", ukey.to_string())])
                .await?,
        )
    }
}
