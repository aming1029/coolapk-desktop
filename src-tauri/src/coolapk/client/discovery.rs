use super::*;

impl CoolapkClient {
    /// 获取首页 Tab 配置（关注/头条/热榜/快讯/话题等频道 + 热门搜索）
    /// 数据来源: GET /v6/main/init
    pub async fn get_tab_config(&self) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/main/init", &[]).await?)
    }

    /// 更新/同步用户自定义首页频道顺序与显隐状态到酷安云端账号
    /// 数据来源: POST /v6/account/updateConfig
    pub async fn update_home_tab_config(&self, home_tab_config_json: &str) -> Result<Value, String> {
        let is_logged_in = self
            .user_cookie
            .read()
            .map(|c| c.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
            .unwrap_or(false);

        if !is_logged_in {
            return Ok(json!({ "code": 200, "message": "未登录，已保存至本地" }));
        }

        match self
            .api_post(
                "/v6/account/updateConfig",
                &[],
                &[("home_tab_config", home_tab_config_json.to_string())],
            )
            .await
        {
            Ok(raw) => Ok(json!({ "code": 200, "data": raw })),
            Err(err) => {
                log::warn!("home_tab.remote_sync_failed");
                Ok(json!({ "code": 200, "warning": err }))
            }
        }
    }

    /// 获取发现频道的服务端配置。
    /// 数据来源: GET /v6/main/init，前端从 entity id=20131 的卡片中解析 ConfigPage。
    pub async fn get_discovery_config(&self) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/main/init", &[]).await?)
    }

    /// APK 动态频道统一列表接口。
    /// 数据来源: GET /v6/page/dataList
    pub async fn get_discovery_page_data(
        &self,
        url: &str,
        title: &str,
        sub_title: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        page_context: &str,
        request_args_json: &str,
    ) -> Result<Value, String> {
        if !is_safe_discovery_page_url(url) {
            return Err("发现页地址不受信任，已拒绝请求".to_string());
        }

        let mut query = vec![("url", url.to_string()), ("page", page.max(1).to_string())];
        if !title.trim().is_empty() {
            query.push(("title", title.to_string()));
        }
        if !sub_title.trim().is_empty() {
            query.push(("subTitle", sub_title.to_string()));
        }
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.to_string()));
        }
        if !page_context.trim().is_empty() {
            query.push(("pageContext", page_context.to_string()));
        }
        let extra_query = parse_discovery_request_args(request_args_json);
        for (key, value) in &extra_query {
            query.push((key.as_str(), value.clone()));
        }

        wrap_page_data_response(self.api_get("/v6/page/dataList", &query).await?)
    }

    /// 搜索候选词（输入联想）
    /// 数据来源: GET /v6/search/suggestSearchWordsNew
    pub async fn get_search_suggestions(&self, query: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/search/suggestSearchWordsNew",
                &[("searchValue", query.to_string())],
            )
            .await?,
        )
    }

    /// 话题详情（旧版 tagDetail，仍可用，部分场景返回字段与 newTagDetail 互补）
    /// 数据来源: GET /v6/topic/tagDetail
    pub async fn get_topic_detail_v7(&self, tag: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/topic/tagDetail", &[("tag", tag.to_string())])
                .await?,
        )
    }
}
