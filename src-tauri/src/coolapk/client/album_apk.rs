use super::*;

impl CoolapkClient {
    /// 检查登录态（比 user/space 更轻量的专用接口）
    /// 数据来源: GET /v6/account/checkLoginInfo
    pub async fn check_login_info(&self) -> Result<Value, String> {
        let cookie = self
            .user_cookie
            .read()
            .map_err(|_| "failed to read login state".to_string())?
            .clone()
            .ok_or_else(|| "当前没有登录凭据".to_string())?;
        let has_session = Self::has_valid_session_cookie(&cookie);
        if !has_session {
            return Err("当前 Cookie 不包含有效会话".to_string());
        }

        // 官方客户端会为登录初始化检查显式传入 checkInit=1，缺少该参数时服务端可能返回“登录信息有误”。
        let result = wrap_api_data(self.api_get("/v6/account/checkLoginInfo", &[("checkInit", "1".to_string())]).await?)?;
        let data = result.get("data").unwrap_or(&result);
        let uid = data
            .get("uid")
            .or_else(|| data.get("id"))
            .map(value_to_string)
            .unwrap_or_default();
        if uid.is_empty() || uid == "0" || uid == "10000" {
            return Err("酷安账号尚未登录".to_string());
        }
        Ok(result)
    }

    #[allow(dead_code)]
    pub(super) async fn post_id_action(&self, path: &str, field: &str, value: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(path, &[], &[(field, value.to_string())])
                .await?,
        )
    }

    /// 应用集列表
    /// 数据来源: GET /v6/album/list
    pub async fn get_album_list(&self, list_type: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/album/list",
                &[
                    ("listType", list_type.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 搜索应用集
    /// 数据来源: GET /v6/album/search
    pub async fn search_albums(&self, query: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/album/search",
                &[("q", query.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 应用集详情
    /// 数据来源: GET /v6/album/detail
    pub async fn get_album_detail(&self, album_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/album/detail", &[("id", album_id.to_string())])
                .await?,
        )
    }

    /// 我的专辑列表
    /// 数据来源: GET /v6/user/albumList
    pub async fn get_user_album_list(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/albumList",
                &[("uid", uid.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 创建专辑
    pub async fn create_album(&self, title: &str, intro: &str, cover: &str) -> Result<Value, String> {
        let form = vec![
            ("title", title.to_string()),
            ("intro", intro.to_string()),
            ("cover", cover.to_string()),
        ];
        wrap_api_data(self.api_post("/v6/album/create", &[], &form).await?)
    }

    /// 编辑专辑
    pub async fn edit_album(&self, album_id: &str, title: &str, intro: &str, cover: &str) -> Result<Value, String> {
        let form = vec![
            ("title", title.to_string()),
            ("intro", intro.to_string()),
            ("cover", cover.to_string()),
        ];
        wrap_api_data(
            self.api_post("/v6/album/edit", &[("id", album_id.to_string())], &form)
                .await?,
        )
    }

    /// 向专辑添加应用
    pub async fn add_album_apk(
        &self,
        album_id: &str,
        package_name: &str,
        title: &str,
        url: &str,
        note: &str,
        display_order: i32,
        logo: &str,
    ) -> Result<Value, String> {
        let form = vec![
            ("packageName", package_name.to_string()),
            ("title", title.to_string()),
            ("url", url.to_string()),
            ("note", note.to_string()),
            ("displayOrder", display_order.to_string()),
            ("logo", logo.to_string()),
        ];
        wrap_api_data(
            self.api_post("/v6/album/addApk", &[("id", album_id.to_string())], &form)
                .await?,
        )
    }

    /// 从专辑移除应用
    pub async fn delete_album_apk(&self, album_id: &str, package_name: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/album/delApk",
                &[("id", album_id.to_string())],
                &[("packageName", package_name.to_string())],
            )
            .await?,
        )
    }

    /// 应用集评论
    /// 数据来源: GET /v6/album/replyList
    pub async fn get_album_replies(&self, album_id: &str, page: u32) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/album/replyList",
                &[("id", album_id.to_string()), ("page", page.to_string())],
            )
            .await?,
        )
    }

    /// 头条列表
    /// 数据来源: GET /v6/main/headline
    pub async fn get_headline_feeds(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/main/headline", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 更新列表
    /// 数据来源: GET /v6/main/updateList
    pub async fn get_update_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/main/updateList", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 编辑精选
    /// 数据来源: GET /v6/feed/editorChoiceList
    pub async fn get_editor_choice_feeds(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/feed/editorChoiceList", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 应用发现者列表
    /// 数据来源: GET /v6/apk/discovererList
    pub async fn get_apk_discoverers(
        &self,
        package_name: &str,
        page: u32,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/apk/discovererList",
                &[("id", package_name.to_string()), ("page", page.to_string())],
            )
            .await?,
        )
    }

    /// 推荐应用列表
    /// 数据来源: GET /v6/apk/recommendList
    pub async fn get_apk_recommend_list(
        &self,
        apk_type: &str,
        title: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/apk/recommendList",
                &[
                    ("apkType", apk_type.to_string()),
                    ("title", title.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        let apks = Self::extract_apk_list(&raw, "all");
        Ok(json!({ "code": 200, "data": apks }))
    }

    /// 相关应用列表
    /// 数据来源: GET /v6/apk/search?q={包名}&apkType=0&searchType=related&page={page}
    /// 官方客户端 RelatedAppsFragment 经 /v6/apk/search 的 searchType=related 获取相关应用
    pub async fn get_apk_related_apps(
        &self,
        package_name: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/apk/search",
                &[
                    ("q", package_name.to_string()),
                    ("apkType", "0".to_string()),
                    ("searchType", "related".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        let apks = Self::extract_apk_list(&raw, "all");
        Ok(json!({ "code": 200, "data": apks }))
    }

    /// 应用礼品列表
    /// 数据来源: GET /v6/apk/giftList
    pub async fn get_apk_gift_list(
        &self,
        apk_id: Option<&str>,
        page: u32,
    ) -> Result<Value, String> {
        let mut params: Vec<(&str, String)> = vec![("page", page.to_string())];
        if let Some(apk_id) = apk_id {
            params.push(("apkId", apk_id.to_string()));
        }
        wrap_api_data(self.api_get("/v6/apk/giftList", &params).await?)
    }

    /// 下载版本列表
    /// 数据来源: GET /v6/apk/detail（取应用数字 ID）→ GET /v6/apk/downloadVersionList?id={数字ID}
    /// 注意：downloadVersionList 的 id 参数是应用数字 ID，传包名会恒返回"没有历史版本"
    pub async fn get_download_version_list(&self, package_name: &str) -> Result<Value, String> {
        let detail = wrap_api_data(
            self.api_get("/v6/apk/detail", &[("id", package_name.to_string())])
                .await?,
        )?;
        let apk_id = detail
            .get("data")
            .and_then(|d| d.get("aid").or_else(|| d.get("id")))
            .map(value_to_string)
            .unwrap_or_default();
        if apk_id.is_empty() {
            return Ok(json!({ "code": 200, "data": [] }));
        }
        let raw = self
            .api_get(
                "/v6/apk/downloadVersionList",
                &[("id", apk_id), ("page", "1".to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 图片列表(按标签)
    /// 数据来源: GET /v6/picture/list
    pub async fn get_picture_list(&self, tag: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/picture/list",
                &[("tag", tag.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 用户评分列表
    /// 数据来源: GET /v6/user/apkRatingList
    pub async fn get_user_rating_list(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/apkRatingList",
                &[("uid", uid.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 按开发者搜索应用
    /// 数据来源: GET /v6/apk/search?searchType=developer
    pub async fn search_apks_by_developer(
        &self,
        developer: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/apk/search",
                &[
                    ("searchType", "developer".to_string()),
                    ("developer", developer.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        let apks = Self::extract_apk_list(&raw, "all");
        Ok(json!({ "code": 200, "data": apks }))
    }

    /// 按标签搜索应用
    /// 数据来源: GET /v6/apk/search?searchType=tag
    pub async fn search_apks_by_tag(
        &self,
        tag: &str,
        apk_type: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/apk/search",
                &[
                    ("searchType", "tag".to_string()),
                    ("tag", tag.to_string()),
                    ("apkType", apk_type.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        let apks = Self::extract_apk_list(&raw, "all");
        Ok(json!({ "code": 200, "data": apks }))
    }

    // === 好物 / 购物生态 ===
}
