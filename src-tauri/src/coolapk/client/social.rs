use super::*;

impl CoolapkClient {
    /// 获取酷安直播详情
    /// 数据来源: GET /v6/live/detail?id={liveId}
    pub async fn get_live_detail(&self, live_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/live/detail", &[("id", live_id.to_string())])
                .await?,
        )
    }

    /// 预约/取消预约酷安直播
    /// 数据来源: GET /v6/live/follow 或 /v6/live/unFollow?id={liveId}
    pub async fn follow_live(&self, live_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/live/follow", &[("id", live_id.to_string())])
                .await?,
        )
    }

    pub async fn unfollow_live(&self, live_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/live/unFollow", &[("id", live_id.to_string())])
                .await?,
        )
    }

    /// 动态转发列表
    /// 数据来源: GET /v6/feed/forwardList?id={feedId}&type={feedType}&page={page}
    pub async fn get_feed_forward_list(
        &self,
        feed_id: &str,
        feed_type: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/feed/forwardList",
                &[
                    ("id", feed_id.to_string()),
                    ("type", feed_type.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        wrap_api_data(raw)
    }

    /// 动态点赞列表
    /// 数据来源: GET /v6/feed/likeList?id={feedId}&listType=lastupdate_desc&page={page}
    pub async fn get_feed_like_list(&self, feed_id: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/feed/likeList",
                &[
                    ("id", feed_id.to_string()),
                    ("listType", "lastupdate_desc".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        wrap_api_data(raw)
    }

    /// 动态修改历史
    /// 数据来源: GET /v6/feed/changeHistoryList?id={feedId}
    pub async fn get_feed_change_history(&self, feed_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/feed/changeHistoryList", &[("id", feed_id.to_string())])
                .await?,
        )
    }

    /// 话题搜索
    /// 数据来源: GET /v6/feed/searchTag?q={query}&page={page}
    pub async fn search_tags(&self, query: &str, page: u32) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/feed/searchTag",
                &[("q", query.to_string()), ("page", page.to_string())],
            )
            .await?,
        )
    }

    /// 关注/取消关注话题
    /// 数据来源: GET /v6/feed/followTag?tag={tag}
    pub async fn follow_tag(&self, tag: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/feed/followTag", &[("tag", tag.to_string())])
                .await?,
        )
    }

    pub async fn unfollow_tag(&self, tag: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/feed/unFollowTag", &[("tag", tag.to_string())])
                .await?,
        )
    }

    /// 用户关注的话题列表
    /// 数据来源: GET /v6/page/dataList?url=#/topic/userFollowTagList?&title=我关注的话题
    pub async fn get_followed_topics(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/page/dataList",
                &[
                    (
                        "url",
                        "#/topic/userFollowTagList?&title=我关注的话题".to_string(),
                    ),
                    ("title", "我关注的话题".to_string()),
                    ("subTitle", String::new()),
                    ("page", page.to_string()),
                    ("firstItem", String::new()),
                    ("lastItem", String::new()),
                    ("pageContext", String::new()),
                ],
            )
            .await?;
        Ok(json!({
            "code": raw.get("code").cloned().unwrap_or(json!(200)),
            "message": raw.get("message").cloned().unwrap_or(Value::Null),
            "data": raw.get("data").cloned().unwrap_or(json!([]))
        }))
    }
}
