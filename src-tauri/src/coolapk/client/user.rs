use super::*;

impl CoolapkClient {
    pub(super) async fn get_user_data(&self, path: &str, uid: &str, guest: bool) -> Result<Value, String> {
        let query = [("uid", uid.to_string())];
        if !guest {
            return wrap_api_data(self.api_get(path, &query).await?);
        }

        let mut errors = Vec::new();
        for api_origin in ["https://api.coolapk.com", "https://api2.coolapk.com"] {
            match self.public_api_get_from(api_origin, path, &query).await {
                Ok(value) => match wrap_api_data(value) {
                    Ok(data) => return Ok(data),
                    Err(error) => errors.push(format!("{api_origin}: {error}")),
                },
                Err(error) => errors.push(format!("{api_origin}: {error}")),
            }
        }
        Err(format!("游客用户资料加载失败：{}", errors.join("；")))
    }

    pub async fn get_user_space(&self, uid: &str) -> Result<Value, String> {
        self.get_user_data("/v6/user/space", uid, false).await
    }

    /// 后台读取使用游客设备与凭据，不携带登录 Cookie。
    pub async fn get_public_user_space(&self, uid: &str) -> Result<Value, String> {
        self.get_user_data("/v6/user/space", uid, true).await
    }

    pub async fn get_user_profile(&self, uid: &str) -> Result<Value, String> {
        self.get_user_data("/v6/user/profile", uid, false).await
    }

    pub async fn get_my_profile(&self, uid: &str) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/user/profile", &[
            ("uid", uid.to_string()), ("installTime", "0".to_string()),
            ("showSettingTab", "1".to_string()),
        ]).await?)
    }

    pub async fn get_public_user_profile(&self, uid: &str) -> Result<Value, String> {
        self.get_user_data("/v6/user/profile", uid, true).await
    }

    /// 读取当前登录用户维护的用户备注列表。
    pub async fn get_user_remark_list(&self, uid: &str) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/user/remarkList", &[("uid", uid.to_string())]).await?)
    }

    /// 修改个人资料字段。对应 APK 的 POST /v6/account/changeProfile。
    pub async fn update_user_profile(&self, key: &str, value: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/account/changeProfile",
                &[],
                &[("key", key.to_string()), ("value", value.to_string())],
            )
            .await?,
        )
    }

    /// 修改头像。对应 APK 的 multipart POST /v6/account/changeAvatar。
    pub async fn change_avatar(
        &self,
        image_bytes: &[u8],
        file_name: &str,
        content_type: &str,
    ) -> Result<Value, String> {
        if image_bytes.is_empty() {
            return Err("头像文件不能为空".to_string());
        }

        let safe_file_name = if file_name.trim().is_empty() {
            "avatar.jpg"
        } else {
            file_name.trim()
        };
        let part = reqwest::multipart::Part::bytes(image_bytes.to_vec())
            .file_name(safe_file_name.to_string());
        let part = if content_type.trim().is_empty() {
            part
        } else {
            part.mime_str(content_type.trim())
                .map_err(|e| format!("头像文件类型无效: {e}"))?
        };
        let form = reqwest::multipart::Form::new().part("imgFile", part);
        self.request_multipart_api("/v6/account/changeAvatar", form)
            .await
    }

    /// 修改个人主页背景图。图片地址由前端先通过 OSS 上传后再提交。
    pub async fn update_user_cover(&self, url: &str) -> Result<Value, String> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err("背景图地址不能为空".to_string());
        }
        wrap_api_data(
            self.api_post(
                "/v6/account/changeAvatarCover",
                &[],
                &[("url", trimmed.to_string())],
            )
            .await?,
        )
    }

    /// APK UserQRCodeFragment 使用 `/user/qrImage?uid=...`，这是图片响应而不是 JSON。
    /// 复用带设备指纹和 Cookie 的图片加载器，返回前端可直接展示的 data URL。
    pub async fn get_user_qr_image(&self, uid: &str) -> Result<Value, String> {
        if uid.trim().is_empty() || !uid.chars().all(|ch| ch.is_ascii_digit()) {
            return Err("用户 UID 格式无效".to_string());
        }
        let image = self
            .get_image_data_url(&format!(
                "https://api.coolapk.com/v6/user/qrImage?uid={}",
                uid.trim()
            ))
            .await?;
        Ok(json!({ "code": 200, "data": image }))
    }

    pub async fn get_user_follow_nodes(&self, uid: &str) -> Result<Value, String> {
        // /v6/user/customNodeList 已被服务端移除，APK 当前统一读取论坛关注列表。
        self.get_user_forum_follow_list(uid, 1).await
    }

    /// 用户关注的论坛列表
    /// 数据来源: GET /v6/user/forumFollowList
    pub async fn get_user_forum_follow_list(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/forumFollowList",
                &[
                    ("uid", uid.to_string()),
                    ("page", page.to_string()),
                    ("firstItem", String::new()),
                    ("lastItem", String::new()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    pub async fn get_user_feeds(
        &self,
        uid: &str,
        page: u32,
        feed_type: &str,
    ) -> Result<Value, String> {
        let feed_endpoint = match feed_type {
            "picture" | "coolpic" => "pictureList",
            "reply" => "replyList",
            "rating" => "apkRatingList",
            "ershou" => "ershouList",
            "fav" | "favorite" => "favList",
            _ => "feedList",
        };
        let raw = self
            .api_get(
                &format!("/v6/user/{feed_endpoint}"),
                &[
                    ("uid", uid.to_string()),
                    ("page", page.to_string()),
                    ("isIncludeTop", "1".to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 用户点赞过的内容
    /// 数据来源: GET /v6/user/likeList?uid={uid}&page={page}
    pub async fn get_user_like_list(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/likeList",
                &[("uid", uid.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 用户空间的服务端驱动 Tab 数据。
    ///
    /// 用户页并不是所有 Tab 都返回普通 Feed；APK 会根据 Tab 选择不同的
    /// endpoint，且 `/page/dataList` 返回的 Entity 还可能包含未知模板。因此
    /// 这里保留服务端 Entity 原文，不复用会过滤 card/banner 的 feed 清洗器。
    pub async fn get_user_tab_data(
        &self,
        uid: &str,
        tab: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        rating_target: &str,
    ) -> Result<Value, String> {
        let page_string = page.to_string();
        let mut query: Vec<(&str, String)> = vec![
            ("uid", uid.to_string()),
            ("page", page_string.clone()),
        ];
        if !first_item.is_empty() {
            query.push(("firstItem", first_item.to_string()));
        }
        if !last_item.is_empty() {
            query.push(("lastItem", last_item.to_string()));
        }

        let raw = match tab {
            "feed" => {
                query.extend([
                    ("showAnonymous", "0".to_string()),
                    ("isIncludeTop", "1".to_string()),
                    ("showDoing", "1".to_string()),
                ]);
                self.api_get("/v6/user/feedList", &query).await?
            }
            "reply" => self.api_get("/v6/user/replyList", &query).await?,
            "collection" => self.api_get("/v6/collection/list", &query).await?,
            "goods_store" => self.api_get("/v6/goods/goodsStoreItemList", &query).await?,
            "goods_rank" => self.api_get("/v6/goodsList/list", &query).await?,
            "developer_apps" => self.api_get("/v6/apk/developerAppList", &query).await?,
            "apk_follow" => self.api_get("/v6/user/apkFollowList", &query).await?,
            "article" => self.api_get("/v6/user/htmlFeedList", &query).await?,
            "qa" => self.api_get("/v6/user/questionAndAnswerList", &query).await?,
            "album" => self.api_get("/v6/user/albumList", &query).await?,
            "like" => self.api_get("/v6/user/likeList", &query).await?,
            "discovery" => self.api_get("/v6/user/discoveryList", &query).await?,
            "coolpic" => self
                .get_user_page_data(
                    uid,
                    "#/feed/userCoolPictureFeedList?fragmentTemplate=flex",
                    page,
                    first_item,
                    last_item,
                    "酷图",
                    "",
                )
                .await?,
            "rating" => self
                .get_user_page_data(
                    uid,
                    &format!(
                        "#/feed/nodeRatingList?uid={}&targetType={}&parseRatingToFeed=1",
                        uid, rating_target
                    ),
                    page,
                    first_item,
                    last_item,
                    "评分",
                    "",
                )
                .await?,
            "goods" => self
                .get_user_page_data(
                    uid,
                    "#/goods/goodsFeedList?type=default&fragmentTemplate=flex",
                    page,
                    first_item,
                    last_item,
                    "好物",
                    "",
                )
                .await?,
            "ershou" => self
                .get_user_page_data(
                    uid,
                    "#/feed/userErshouList?fragmentTemplate=flex&ershouStatus=userAll",
                    page,
                    first_item,
                    last_item,
                    "二手",
                    "",
                )
                .await?,
            "recycle" => self
                .get_user_page_data(
                    uid,
                    "#/feed/userDeleteFeedList",
                    page,
                    first_item,
                    last_item,
                    "回收站",
                    "",
                )
                .await?,
            "blacklist" => self.get_black_list(page).await?,
            _ => return Err(format!("不支持的用户页 Tab: {tab}")),
        };

        if raw.get("code").and_then(Value::as_i64) == Some(200) {
            return Ok(raw);
        }
        Ok(json!({ "code": 200, "data": Self::extract_entity_rows(&raw) }))
    }

    /// `/page/dataList` 用户页白名单分发器。调用方只能传入已确认的固定路径，
    /// 不接受任意 URL，避免把带设备指纹和 Cookie 的请求变成开放代理。
    pub(super) async fn get_user_page_data(
        &self,
        uid: &str,
        page_url: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        title: &str,
        sub_title: &str,
    ) -> Result<Value, String> {
        if !Self::is_allowed_user_page_url(page_url) {
            return Err("不受信任的用户页 page/dataList 路径".to_string());
        }

        let page_url_with_uid = if page_url.contains("uid=") {
            page_url.to_string()
        } else {
            format!("{}&uid={}", page_url, uid)
        };
        let mut query = vec![
            ("url", page_url_with_uid),
            ("title", title.to_string()),
            ("subTitle", sub_title.to_string()),
            ("page", page.to_string()),
            ("pageContext", "user_space".to_string()),
        ];
        if !first_item.is_empty() {
            query.push(("firstItem", first_item.to_string()));
        }
        if !last_item.is_empty() {
            query.push(("lastItem", last_item.to_string()));
        }
        self.api_get("/v6/page/dataList", &query).await
    }

    pub(super) fn is_allowed_user_page_url(page_url: &str) -> bool {
        if page_url == "#/feed/userCoolPictureFeedList?fragmentTemplate=flex"
            || page_url == "#/goods/goodsFeedList?type=default&fragmentTemplate=flex"
            || page_url == "#/feed/userErshouList?fragmentTemplate=flex&ershouStatus=userAll"
            || page_url == "#/feed/userDeleteFeedList"
        {
            return true;
        }
        let rating_prefix = "#/feed/nodeRatingList?uid=";
        let Some(remainder) = page_url.strip_prefix(rating_prefix) else {
            return false;
        };
        let parts: Vec<&str> = remainder.split('&').collect();
        parts.len() == 3
            && !parts[0].is_empty()
            && matches!(parts[1], "targetType=all" | "targetType=apk" | "targetType=product")
            && parts[2] == "parseRatingToFeed=1"
    }

    /// 保留 server-driven Entity 的原始字段，仅展开常见的 entities 包装。
    pub(super) fn extract_entity_rows(json_data: &Value) -> Vec<Value> {
        let Some(data) = json_data.get("data") else {
            return Vec::new();
        };
        if let Some(items) = data.as_array() {
            let mut result = Vec::new();
            for item in items {
                if let Some(entities) = item.get("entities").and_then(Value::as_array) {
                    result.extend(entities.iter().cloned());
                } else {
                    result.push(item.clone());
                }
            }
            return result;
        }
        if let Some(items) = data.get("entities").and_then(Value::as_array) {
            return items.clone();
        }
        Vec::new()
    }

    /// 判断搜索结果中的 sponsor 实体。桌面版不展示搜索广告，但仍保留其它 server-driven 字段。
    pub(super) fn is_sponsor_search_entity(value: &Value) -> bool {
        let Some(object) = value.as_object() else {
            return false;
        };
        let field_text = |key: &str| {
            object
                .get(key)
                .or_else(|| object.get(match key {
                    "entityType" => "entity_type",
                    "entityTemplate" => "entity_template",
                    _ => key,
                }))
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        };
        let type_name = field_text("entityType");
        let template = field_text("entityTemplate");
        let sponsor_type = object
            .get("sponsorType")
            .or_else(|| object.get("sponsor_type"));
        type_name.starts_with("sponsor")
            || template.starts_with("sponsor")
            || sponsor_type.is_some_and(|value| match value {
                Value::Null => false,
                Value::String(text) => !text.trim().is_empty(),
                Value::Bool(flag) => *flag,
                Value::Number(number) => number.as_i64().map(|number| number != 0).unwrap_or(true),
                Value::Array(items) => !items.is_empty(),
                Value::Object(items) => !items.is_empty(),
            })
    }

    /// 递归过滤 sponsorForSearch，同时保留 APK 返回的原始实体结构。
    pub(super) fn sanitize_search_value(value: &Value) -> Option<Value> {
        if Self::is_sponsor_search_entity(value) {
            return None;
        }
        match value {
            Value::Array(items) => Some(Value::Array(
                items
                    .iter()
                    .filter_map(Self::sanitize_search_value)
                    .collect(),
            )),
            Value::Object(object) => {
                let mut cleaned = serde_json::Map::new();
                for (key, child) in object {
                    if let Some(sanitized) = Self::sanitize_search_value(child) {
                        cleaned.insert(key.clone(), sanitized);
                    }
                }
                Some(Value::Object(cleaned))
            }
            _ => Some(value.clone()),
        }
    }

    pub(super) fn wrap_sanitized_search_data(raw: &Value) -> Value {
        let data = raw
            .get("data")
            .and_then(|value| Self::sanitize_search_value(value))
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let mut response = raw.as_object().cloned().unwrap_or_default();
        response.insert("code".to_string(), json!(200));
        response.insert("data".to_string(), data);
        Value::Object(response)
    }
}
