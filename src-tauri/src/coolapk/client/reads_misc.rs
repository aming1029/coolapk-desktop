use super::*;

impl CoolapkClient {
    /// 话题设备（数码）动态列表
    /// 数据来源: GET /v6/topic/deviceFeedList?tag={tag}&page={page}&listType=lastupdate_desc
    pub async fn get_device_feed_list(
        &self,
        tag: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let mut query = vec![
            ("tag", tag.to_string()),
            ("page", page.max(1).to_string()),
            ("listType", "lastupdate_desc".to_string()),
        ];
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.trim().to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.trim().to_string()));
        }
        let raw = self.api_get("/v6/topic/deviceFeedList", &query).await?;
        let data = Self::extract_cleaned_list(&raw);
        let first = data.first().map(topic_hub_cursor).unwrap_or_default();
        let last = data.last().map(topic_hub_cursor).unwrap_or_default();
        Ok(json!({ "code": 200, "data": data, "firstItem": first, "lastItem": last }))
    }

    /// 问答（Q&A）列表
    /// 数据来源: GET /v6/question/answerList?id={feedId}&sort={sort}&page={page}&firstItem={firstItem}&lastItem={lastItem}
    pub async fn get_question_answers(
        &self,
        feed_id: &str,
        sort: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let mut query = vec![
            ("id", feed_id.to_string()),
            ("sort", sort.to_string()),
            ("page", page.max(1).to_string()),
        ];
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.trim().to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.trim().to_string()));
        }

        let raw = self.api_get("/v6/question/answerList", &query).await?;
        let data = Self::extract_cleaned_list(&raw);
        let first = data.first().map(topic_hub_cursor).unwrap_or_default();
        let last = data.last().map(topic_hub_cursor).unwrap_or_default();
        Ok(json!({ "code": 200, "data": data, "firstItem": first, "lastItem": last }))
    }

    /// 关注问题。对应 APK 的 GET /v6/question/follow?id={questionId}。
    pub async fn follow_question(&self, question_id: &str) -> Result<Value, String> {
        let question_id = question_id.trim();
        if question_id.is_empty() {
            return Err("问题 ID 不能为空".to_string());
        }
        wrap_api_data(
            self.api_get("/v6/question/follow", &[("id", question_id.to_string())])
                .await?,
        )
    }

    /// 取消关注问题。对应 APK 的 GET /v6/question/unFollow?id={questionId}。
    pub async fn unfollow_question(&self, question_id: &str) -> Result<Value, String> {
        let question_id = question_id.trim();
        if question_id.is_empty() {
            return Err("问题 ID 不能为空".to_string());
        }
        wrap_api_data(
            self.api_get("/v6/question/unFollow", &[("id", question_id.to_string())])
                .await?,
        )
    }

    /// 邀请一个或多个用户回答问题。APK 以逗号分隔的 UID 作为 multipart 字段提交。
    pub async fn invite_question_answer(&self, question_id: &str, uid: &str) -> Result<Value, String> {
        let question_id = question_id.trim();
        let uid = uid.trim();
        if question_id.is_empty() {
            return Err("问题 ID 不能为空".to_string());
        }
        if uid.is_empty() {
            return Err("邀请用户 UID 不能为空".to_string());
        }
        let form = reqwest::multipart::Form::new()
            .text("uid", uid.to_string())
            .text("questionId", question_id.to_string());
        self.request_multipart_api("/v6/question/inviteAnswer", form)
            .await
    }

    /// 投票评论列表
    /// 数据来源: GET /v6/vote/commentList?fid={feedId}&page={page}
    pub async fn get_vote_comments(&self, feed_id: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/vote/commentList",
                &[("fid", feed_id.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 提交用户投票
    /// 数据来源: POST /v6/vote/createUserVote
    /// APK 参数: id、select_option[0..n]、anonymous_status
    pub async fn create_user_vote(
        &self,
        feed_id: &str,
        option_ids: &[String],
        anonymous_status: bool,
    ) -> Result<Value, String> {
        if feed_id.trim().is_empty() {
            return Err("投票动态 ID 不能为空".to_string());
        }
        if option_ids.is_empty() {
            return Err("至少选择一个投票选项".to_string());
        }

        let mut owned_fields = vec![
            ("id".to_string(), feed_id.to_string()),
            (
                "anonymous_status".to_string(),
                if anonymous_status { "1" } else { "0" }.to_string(),
            ),
        ];
        for (index, option_id) in option_ids.iter().enumerate() {
            let option_id = option_id.trim();
            if option_id.is_empty() {
                return Err("投票选项 ID 不能为空".to_string());
            }
            owned_fields.push((format!("select_option[{index}]"), option_id.to_string()));
        }

        let form: Vec<(&str, String)> = owned_fields
            .iter()
            .map(|(key, value)| (key.as_str(), value.clone()))
            .collect();
        wrap_api_data(
            self.api_post("/v6/vote/createUserVote", &[], &form)
                .await?,
        )
    }

    /// 用户浏览历史
    /// 数据来源: GET /v6/user/hitHistoryList?type={type}&page={page}&firstItem={firstItem}&lastItem={lastItem}
    pub async fn get_hit_history(&self, page: u32, history_type: &str, first_item: Option<&str>, last_item: Option<&str>) -> Result<Value, String> {
        let mut query = vec![("type", history_type.to_string()), ("page", page.to_string())];
        if let Some(first_item) = first_item.filter(|value| !value.trim().is_empty()) {
            query.push(("firstItem", first_item.to_string()));
        }
        if let Some(last_item) = last_item.filter(|value| !value.trim().is_empty()) {
            query.push(("lastItem", last_item.to_string()));
        }
        let raw = self
            .api_get("/v6/user/hitHistoryList", &query)
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_history_list(&raw) }))
    }

    /// 内容回收站/垃圾动态列表（仅账号具备审核权限时可读）
    /// 数据来源: GET /v6/feed/spamFeedList?type=feed
    pub async fn get_spam_feed_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/feed/spamFeedList",
                &[
                    ("type", "feed".to_string()),
                    ("channel", "feed".to_string()),
                    ("spamType", "feed".to_string()),
                    ("subType", "feed".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 指定动态的隐藏回复
    /// 数据来源: GET /v6/feed/replyList?feedType=feed_reply&blockStatus=4
    pub async fn get_hidden_replies(&self, feed_id: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/feed/replyList",
                &[
                    ("id", feed_id.to_string()),
                    ("listType", String::new()),
                    ("page", page.to_string()),
                    ("discussMode", "0".to_string()),
                    ("feedType", "feed_reply".to_string()),
                    ("blockStatus", "4".to_string()),
                    ("fromFeedAuthor", "0".to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 用户最近历史（访问过的用户/话题等）
    /// 数据来源: GET /v6/user/recentHistoryList?page={page}&firstItem={firstItem}&lastItem={lastItem}
    pub async fn get_recent_history(&self, page: u32, first_item: Option<&str>, last_item: Option<&str>) -> Result<Value, String> {
        let mut query = vec![("page", page.to_string())];
        if let Some(first_item) = first_item.filter(|value| !value.trim().is_empty()) {
            query.push(("firstItem", first_item.to_string()));
        }
        if let Some(last_item) = last_item.filter(|value| !value.trim().is_empty()) {
            query.push(("lastItem", last_item.to_string()));
        }
        let raw = self
            .api_get("/v6/user/recentHistoryList", &query)
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_history_list(&raw) }))
    }

    /// 用户搜索
    /// 数据来源: GET /v6/user/search?q={query}&page={page}
    pub async fn search_users(&self, query: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/search",
                &[("q", query.to_string()), ("page", page.to_string())],
            )
            .await?;
        let mut users = Vec::new();
        if let Some(arr) = raw.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                let obj = match item.as_object() {
                    Some(o) => o,
                    None => continue,
                };
                if obj.get("entityType").and_then(|v| v.as_str()) != Some("user") {
                    continue;
                }
                let uid = get_str_by_keys(obj, &["uid"]).unwrap_or_default();
                let username = get_str_by_keys(obj, &["username"]).unwrap_or_default();
                if uid.is_empty() || username.is_empty() {
                    continue;
                }
                let raw_avatar = get_str_by_keys(obj, &["userAvatar"]).unwrap_or_default();
                let avatar = if raw_avatar.starts_with("http") {
                    raw_avatar
                } else if !raw_avatar.is_empty() {
                    format!(
                        "https://avatar.coolapk.com/{}",
                        raw_avatar.trim_start_matches('/')
                    )
                } else {
                    String::new()
                };
                users.push(json!({
                    "uid": uid,
                    "username": username,
                    "avatar": avatar,
                    "verifyTitle": get_str_by_keys(obj, &["verify_title"]).unwrap_or_default(),
                    "level": get_u64_by_keys(obj, &["level"]),
                    "bio": get_str_by_keys(obj, &["bio", "sign"]).unwrap_or_default(),
                    "fans": get_u64_by_keys(obj, &["fans", "fansnum"]),
                    "follow": get_u64_by_keys(obj, &["follow", "follownum"])
                }));
            }
        }
        Ok(json!({ "code": 200, "data": users }))
    }

    /// 搜索联想（应用类）
    /// 数据来源: GET /v6/search/suggestSearchWordsNew?searchValue={query}&type=app
    pub async fn get_search_suggestions_app(&self, query: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/search/suggestSearchWordsNew",
                &[
                    ("searchValue", query.to_string()),
                    ("type", "app".to_string()),
                ],
            )
            .await?,
        )
    }

    /// 搜索话题
    /// 数据来源: GET /v6/search?type=feedTopic&searchValue={query}&page={page}
    pub async fn search_feed_topics(&self, query: &str, page: u32) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/search",
                &[
                    ("type", "feedTopic".to_string()),
                    ("searchValue", query.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 发帖话题选择使用官方 searchTag，空关键词同时返回最近参与及热门话题。
    pub async fn search_publish_topics(&self, query: &str, page: u32, recent_ids: &str) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/feed/searchTag", &[("q", query.to_string()), ("page", page.to_string()), ("recentIds", recent_ids.to_string())]).await?)
    }

    /// 产品详情（按名称）
    /// 数据来源: GET /v6/product/detail?name={name}
    pub async fn get_product_detail_by_name(&self, name: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/product/detail", &[("name", name.to_string())])
                .await?,
        )
    }

    /// 到手价使用官方版本配置列表，而非产品详情中的参数展示行。
    pub async fn get_product_versions(&self, product_id: &str) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/product/getVersionList", &[("product_id", product_id.to_string())]).await?)
    }

    /// 加载个人页卡片配置
    /// 数据来源: GET /v6/account/loadConfig?key=my_page_card_config
    pub async fn get_load_config(&self) -> Result<Value, String> {
        self.get_my_profile_cards(false).await
    }

    pub async fn get_my_profile_cards(&self, refresh: bool) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/account/loadConfig",
                &[("key", "my_page_card_config".to_string()), ("refresh", if refresh { "1" } else { "0" }.to_string())],
            )
            .await?,
        )
    }

    /// 官方个人中心卡片管理清单，包含已添加和未添加的卡片。
    pub async fn get_my_card_manager(&self) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/account/myPageCardManage", &[("key", "my_page_card_config".to_string())]).await?)
    }

    /// 与官方卡片管理一致，保存 show/hide ID 顺序。
    pub async fn update_my_card_config(&self, config_json: &str) -> Result<Value, String> {
        let config: Value = serde_json::from_str(config_json).map_err(|_| "卡片配置格式错误".to_string())?;
        for key in ["show", "hide"] {
            let ids = config.get(key).and_then(Value::as_array).ok_or_else(|| "卡片配置必须包含 show/hide 列表".to_string())?;
            if ids.iter().any(|id| id.as_u64().is_none()) { return Err("卡片 ID 必须为非负整数".to_string()); }
        }
        wrap_api_data(self.api_post("/v6/account/updateConfig", &[], &[("key", "my_page_card_config".to_string()), ("value", config_json.to_string())]).await?)
    }

    /// 加载 APK 首页栏目配置。
    /// 数据来源: GET /v6/account/loadConfig?key=home_tab_config&reSet=0|1
    pub async fn get_home_tab_config(&self, reset: bool) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/account/loadConfig",
                &[
                    ("key", "home_tab_config".to_string()),
                    ("reSet", if reset { "1".to_string() } else { "0".to_string() }),
                ],
            )
            .await?,
        )
    }

    pub async fn get_topic_detail(&self, tag: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/topic/newTagDetail", &[("tag", tag.to_string())])
                .await?,
        )
    }

    /// 保留 NodeRating 原始字段，不能用动态清洗逻辑丢弃星级和详细评分。
    pub async fn get_topic_rating_list(
        &self, topic_id: &str, page: u32, star: i32, rating_type: &str,
        detail_only: bool, first_item: &str, last_item: &str,
    ) -> Result<Value, String> {
        if topic_id.trim().is_empty() { return Err("缺少话题 ID".to_string()); }
        let query = build_topic_rating_list_query(topic_id, page, star, rating_type, detail_only, first_item, last_item);
        wrap_api_data(self.api_get("/v6/page/dataList", &query).await?)
    }

    pub async fn get_topic_rating_chart(&self, topic_id: &str) -> Result<Value, String> {
        if topic_id.trim().is_empty() { return Err("缺少话题 ID".to_string()); }
        wrap_api_data(self.api_get("/v6/topic/ratingChart", &[("id", topic_id.to_string())]).await?)
    }

    pub async fn get_topic_feeds(
        &self,
        tag: &str,
        page: u32,
        list_type: &str,
        first_item: &str,
        last_item: &str,
        block_status: i32,
    ) -> Result<Value, String> {
        let mut query = vec![("tag", tag.to_string()), ("page", page.to_string())];
        if !list_type.trim().is_empty() {
            query.push(("listType", list_type.to_string()));
        }
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.to_string()));
        }
        query.push(("blockStatus", block_status.to_string()));
        let raw = self.api_get("/v6/topic/tagFeedList", &query).await?;
        Ok(json!({
            "code": 200,
            "data": Self::extract_cleaned_list(&raw),
            "sortOptions": Self::extract_topic_sort_options(&raw),
        }))
    }

    /// 加载 APK 话题页服务端下发的其他栏目。
    /// 数据来源: GET /v6/page/dataList?url={tab_url}
    pub async fn get_topic_tab_data(
        &self,
        url: &str,
        title: &str,
        sub_title: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        page_context: &str,
    ) -> Result<Value, String> {
        if !is_safe_discovery_page_url(url) {
            return Err("话题栏目地址不受信任，已拒绝请求".to_string());
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
        let raw = self.api_get("/v6/page/dataList", &query).await?;
        let data = Self::extract_topic_tab_list(&raw);
        let first = data.first().map(topic_hub_cursor).unwrap_or_default();
        let last = data.last().map(topic_hub_cursor).unwrap_or_default();
        Ok(json!({
            "code": 200,
            "data": data,
            "firstItem": first,
            "lastItem": last,
            "sortOptions": Self::extract_topic_sort_options(&raw),
        }))
    }

    pub(super) async fn fetch_topic_hub_page(&self, url: &str, title: &str, page: u32, first_item: &str, last_item: &str) -> Result<Value, String> {
        let mut query = vec![("url", url.to_string()), ("page", page.max(1).to_string())];
        if !title.trim().is_empty() {
            query.push(("title", title.to_string()));
        }
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.trim().to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.trim().to_string()));
        }
        self.api_get("/v6/page/dataList", &query).await
    }

    pub async fn get_topic_hub_data(
        &self,
        sub_url: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let requested_url = sub_url.trim();
        let clean_sub_url = requested_url.trim_start_matches('#');

        // APK 话题首页先请求 V11_VERTICAL_TOPIC 获取服务端栏目，再按配置中的 selectedTab 加载当前栏目。
        // 栏目标题和地址全部来自服务端，桌面端不维护本地栏目兜底。
        if clean_sub_url.contains("V11_VERTICAL_TOPIC") {
            let config_raw = self
                .fetch_topic_hub_page(requested_url, "话题", 1, "", "")
                .await?;
            let tabs = extract_topic_hub_tabs(&config_raw);
            let mut result = json!({ "code": 200, "data": [], "tabs": tabs });
            if let Some((selected_title, selected_url)) = topic_hub_selected_category(&config_raw) {
                let selected_raw = self
                    .fetch_topic_hub_page(&selected_url, &selected_title, page, first_item, last_item)
                    .await?;
                result = topic_hub_result(&selected_raw);
                if let Some(obj) = result.as_object_mut() {
                    obj.insert("tabs".to_string(), tabs);
                    obj.insert("selectedUrl".to_string(), json!(selected_url));
                }
            }
            return Ok(result);
        }

        // APK 的热门话题使用服务端驱动的连续流，而不是 topic/tagList 的分页网格。
        // 后续请求必须携带首尾实体游标，否则服务端可能返回另一段热门话题，导致
        // 手机上的“薅羊毛”等高热度话题在桌面端缺失或下刷时跳页。
        if clean_sub_url.contains("/topic/hotTagList") {
            let raw = self.fetch_topic_hub_page(requested_url, "热门话题", page, first_item, last_item).await?;
            return Ok(topic_hub_result(&raw));
        }

        // 服务端栏目可能是 /page?url=... 或 #/topic/...，必须原样交给 page/dataList，
        // 否则会被桌面端旧的本地分类逻辑改写成另一套接口。
        if clean_sub_url.starts_with("/page?url=") || clean_sub_url.starts_with("/topic/") {
            let raw = self.fetch_topic_hub_page(requested_url, "", page, first_item, last_item).await?;
            return Ok(topic_hub_result(&raw));
        }

        // 识别分类 Tag 维度 (1: 手机数码, 2: 电脑外设, 3: 游戏生活)
        let tag_type = if clean_sub_url.contains("tagType=1") || clean_sub_url.contains("type=1") {
            Some(1)
        } else if clean_sub_url.contains("tagType=2") || clean_sub_url.contains("type=2") {
            Some(2)
        } else if clean_sub_url.contains("tagType=3") || clean_sub_url.contains("type=3") {
            Some(3)
        } else {
            None
        };

        // 如果选择具体领域维度分类，使用酷安原生 /v6/search?type=topic 接口精准拉取专属话题
        if let Some(tt) = tag_type {
            let search_term = match tt {
                1 => "手机",
                2 => "电脑",
                3 => "游戏",
                _ => "数码",
            };

            let search_raw = self
                .api_get(
                    "/v6/search",
                    &[
                        ("type", "topic".to_string()),
                        ("searchValue", search_term.to_string()),
                        ("page", page.to_string()),
                        ("show_flag", "1".to_string()),
                    ],
                )
                .await?;

            let data = search_raw.get("data").cloned().unwrap_or(json!([]));
            return Ok(json!({ "code": 200, "data": data }));
        }

        // 基础排行榜维度：热门/最受关注/最新
        let mut query = vec![("page", page.to_string())];
        if clean_sub_url.contains("sort=follow") {
            query.push(("sort", "follow".to_string()));
        } else if clean_sub_url.contains("sort=new") {
            query.push(("sort", "new".to_string()));
        } else {
            query.push(("sort", "hot".to_string()));
        }

        let raw = self.api_get("/v6/topic/tagList", &query).await;

        let res = match raw {
            Ok(val)
                if val
                    .get("data")
                    .and_then(|d| d.as_array())
                    .map_or(false, |arr| !arr.is_empty()) =>
            {
                val
            }
            _ => {
                let page_url = if clean_sub_url.is_empty() || clean_sub_url == "/main/tagList" {
                    "/topic/tagList".to_string()
                } else {
                    clean_sub_url.to_string()
                };
                self.api_get(
                    "/v6/page/dataList",
                    &[("url", page_url), ("page", page.to_string())],
                )
                .await?
            }
        };

        let mut data = res.get("data").cloned().unwrap_or(json!([]));

        // 对最受关注维度按照关注人数 follower_num 进行二次精准倒序重排
        if clean_sub_url.contains("sort=follow") {
            if let Some(arr) = data.as_array_mut() {
                arr.sort_by(|a, b| {
                    let f_a = a
                        .get("follower_num")
                        .and_then(|v| v.as_u64())
                        .or_else(|| a.get("follownum").and_then(|v| v.as_u64()))
                        .unwrap_or(0);
                    let f_b = b
                        .get("follower_num")
                        .and_then(|v| v.as_u64())
                        .or_else(|| b.get("follownum").and_then(|v| v.as_u64()))
                        .unwrap_or(0);
                    f_b.cmp(&f_a)
                });
            }
        }

        let mut result = topic_hub_result(&json!({ "data": data }));
        if clean_sub_url.contains("sort=follow") {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("sort".to_string(), json!("follow"));
            }
        }
        Ok(result)
    }

    pub async fn get_app_detail(&self, package_name: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/apk/detail", &[("id", package_name.to_string())])
                .await?,
        )
    }

    /// 应用评论列表（应用评价，不是动态讨论）
    /// 数据来源: GET /v6/apk/commentList?id={id}&listType={list_type}&page={page}
    pub async fn get_apk_comments(
        &self,
        app_id: &str,
        list_type: &str,
        page: u32,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/apk/commentList",
                &[
                    ("id", app_id.to_string()),
                    ("listType", list_type.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?,
        )
    }
}
