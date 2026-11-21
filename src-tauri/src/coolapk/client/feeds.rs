use super::*;

impl CoolapkClient {
    pub async fn get_by_full_url(&self, full_url: &str) -> Result<Value, String> {
        let token = self.get_token()?;

        // 防御性校验：带 App 指纹头 + Token + 登录 Cookie 的请求仅允许发往酷安 API 域
        let parsed = reqwest::Url::parse(full_url).map_err(|e| format!("invalid URL: {e}"))?;
        let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
        if !is_coolapk_host(&host) {
            return Err(format!("disallowed non-Coolapk host: {host}"));
        }

        let res = self
            .api_client()?
            .get(full_url)
            .header("X-App-Token", token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let json_data = response_json(res).await?;
        let cleaned_list = Self::extract_cleaned_list(&json_data);

        Ok(json!({ "code": 200, "data": cleaned_list }))
    }

    pub async fn get(&self, endpoint: &str, page: u32) -> Result<Value, String> {
        let url = format!("https://api.coolapk.com/v6{}?page={}", endpoint, page);
        self.get_by_full_url(&url).await
    }

    // 1. 首页推荐
    pub async fn get_index_v8_feeds(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/main/indexV8", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// APK 头条分页：GET /v6/main/indexV8 携带首尾动态游标。
    /// APK 同时传 firstLaunch/installTime/ids；桌面端没有 APK 安装时间和曝光 ID，
    /// 因此保持参数存在并传 0/空字符串，分页游标使用 firstItem/lastItem。
    pub async fn get_index_v8_feeds_paged(
        &self,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/main/indexV8",
                &[
                    ("page", page.to_string()),
                    ("firstLaunch", "0".to_string()),
                    ("installTime", "0".to_string()),
                    ("firstItem", first_item.to_string()),
                    ("lastItem", last_item.to_string()),
                    ("ids", "".to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// APK 首页完整实体分页：保留 indexV8 返回的卡片实体，不能只提取 feed。
    /// 数据来源: GET /v6/main/indexV8
    pub async fn get_index_v8_entities_paged(
        &self,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/main/indexV8",
                &[
                    ("page", page.to_string()),
                    ("firstLaunch", "0".to_string()),
                    ("installTime", "0".to_string()),
                    ("firstItem", first_item.to_string()),
                    ("lastItem", last_item.to_string()),
                    ("ids", "".to_string()),
                ],
            )
            .await?;
        let response = wrap_api_data(raw)?;
        Ok(json!({
            "code": 200,
            "data": response.get("data").cloned().unwrap_or_else(|| json!([])),
        }))
    }

    // 2. 热榜
    // 实测定位：官方热榜 tab（V9_HOME_TAB_RANKING）→ 总榜（V15_DONGTAI_TOP）
    // → 7天总榜 #/feed/statList。sortField 对比实测：
    //   detailnum（详情数）→ 6659/946/1545...
    //   likenum（点赞数）  → 6659/3630/1586/1545/1256... ← 点赞热榜，采用此排序
    // #/feed/hotList 与 V9_HOME_TAB_RANKING 主列表点赞仅个位数/千位以下。
    pub async fn get_hot_feeds(&self, page: u32) -> Result<Value, String> {
        self.get_hot_feeds_with_mode(page, false).await
    }

    pub(super) async fn get_hot_feeds_with_mode(&self, page: u32, guest: bool) -> Result<Value, String> {
        let res = self
            .api_get_for_list(
                "/v6/page/dataList",
                &[
                    (
                        "url",
                        "#/feed/statList?statType=7days&sortField=likenum".to_string(),
                    ),
                    ("title", "热门".to_string()),
                    ("page", page.to_string()),
                ],
                guest,
            )
            .await;

        let cleaned = match res {
            Ok(ref raw) => Self::extract_cleaned_list(raw),
            Err(_) => Vec::new(),
        };

        if !cleaned.is_empty() {
            return Ok(json!({ "code": 200, "data": cleaned }));
        }

        // 备用热榜 API: /v6/page/dataList?url=%23%2Ffeed%2FstatHotList%3Fperiod%3D24h
        let fallback = self
            .api_get_for_list(
                "/v6/page/dataList",
                &[
                    ("url", "#/feed/statHotList?period=24h".to_string()),
                    ("page", page.to_string()),
                ],
                guest,
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&fallback) }))
    }

    /// 热榜页内的五种榜单。周榜沿用带降级的主热榜，其余榜单使用官方统计页参数。
    pub async fn get_rank_feeds(&self, rank_type: &str, page: u32) -> Result<Value, String> {
        self.get_rank_feeds_with_mode(rank_type, page, false).await
    }

    pub async fn get_rank_feeds_with_mode(
        &self,
        rank_type: &str,
        page: u32,
        guest: bool,
    ) -> Result<Value, String> {
        if rank_type == "week" {
            return self.get_hot_feeds_with_mode(page, guest).await;
        }
        if rank_type == "picture" {
            return self.get_cool_picture_rank_with_mode(page, guest).await;
        }

        let rank_url =
            rank_feed_url(rank_type).ok_or_else(|| format!("不支持的热榜类型：{rank_type}"))?;
        let raw = self
            .api_get_for_list(
                "/v6/page/dataList",
                &[("url", rank_url.to_string()), ("page", page.to_string())],
                guest,
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    // 3. 科技快讯 (对接官方快讯页 V11_HOME_TAB_NEWS，含平滑降级)
    pub async fn get_latest_feeds(&self, page: u32) -> Result<Value, String> {
        let res = self
            .api_get(
                "/v6/page/dataList",
                &[
                    ("url", "V11_HOME_TAB_NEWS".to_string()),
                    ("title", "快讯".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await;

        let cleaned = match res {
            Ok(ref raw) => Self::extract_cleaned_list(raw),
            Err(_) => Vec::new(),
        };

        if !cleaned.is_empty() {
            return Ok(json!({ "code": 200, "data": cleaned }));
        }

        // 备用快讯 API: /v6/page/dataList?url=%23%2Ffeed%2FdigestList%3Ftype%3D1
        let fallback = self
            .api_get(
                "/v6/page/dataList",
                &[
                    ("url", "#/feed/digestList?type=1".to_string()),
                    ("title", "快讯".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await;

        let cleaned = match fallback {
            Ok(ref raw) => Self::extract_cleaned_list(raw),
            Err(_) => Vec::new(),
        };

        if !cleaned.is_empty() {
            return Ok(json!({ "code": 200, "data": cleaned }));
        }

        // 备用最新动态 API: /v6/page/dataList?url=%23%2Ffeed%2FnewestList
        let fallback2 = self
            .api_get(
                "/v6/page/dataList",
                &[
                    ("url", "#/feed/newestList".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&fallback2) }))
    }

    // 右侧栏：热门话题 (话题广场 V9_HOME_TAB_TOPIC)
    pub async fn get_hot_topics(&self) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/page/dataList",
                &[
                    ("url", "V9_HOME_TAB_TOPIC".to_string()),
                    ("page", "1".to_string()),
                ],
            )
            .await?;

        let mut topics = Vec::new();
        if let Some(arr) = raw.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                let obj = match item.as_object() {
                    Some(o) => o,
                    None => continue,
                };
                if obj.get("entityType").and_then(|v| v.as_str()) != Some("topic") {
                    continue;
                }
                let tag = get_str_by_keys(obj, &["title"]).unwrap_or_default();
                if tag.is_empty() {
                    continue;
                }
                let count = get_u64_by_keys(obj, &["hot_num", "commentnum", "comment_num"]);
                topics.push(json!({ "tag": tag, "count": count }));
                if topics.len() >= 5 {
                    break;
                }
            }
        }
        Ok(json!({ "code": 200, "data": topics }))
    }

    // 4. 精选热帖
    pub async fn get_digest_feeds(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/page/dataList",
                &[
                    ("url", "#/feed/digestList".to_string()),
                    ("title", "精选".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    // 5. 酷图热榜
    // 实测：digestList?type=8 返回的动态点赞全为 0（数据异常），
    // 官方酷图榜入口为 statList?statType=30days&sortField=likenum&type=8（点赞 256/169/124）
    pub async fn get_cool_picture_rank(&self, page: u32) -> Result<Value, String> {
        self.get_cool_picture_rank_with_mode(page, false).await
    }

    pub(super) async fn get_cool_picture_rank_with_mode(&self, page: u32, guest: bool) -> Result<Value, String> {
        let raw = self
            .api_get_for_list(
                "/v6/page/dataList",
                &[
                    (
                        "url",
                        "#/feed/statList?statType=30days&sortField=likenum&type=8".to_string(),
                    ),
                    ("title", "酷图热榜".to_string()),
                    ("page", page.to_string()),
                ],
                guest,
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    // 6. 酷品二手
    pub async fn get_secondhand_feeds(&self, page: u32) -> Result<Value, String> {
        self.get_board_feeds("V11_FIND_GOOD_GOODS_HOME", page).await
    }

    // 7. 全站搜索
    pub async fn search_all(&self, query: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/search",
                &[
                    ("type", "all".to_string()),
                    ("searchValue", query.to_string()),
                    ("page", page.to_string()),
                    ("show_flag", "1".to_string()),
                ],
            )
            .await?;
        Ok(Self::wrap_sanitized_search_data(&raw))
    }

    /// 按 APK 的 searchType 请求搜索结果，保留服务端实体字段供桌面端动态渲染。
    pub async fn search_by_type(
        &self,
        search_type: &str,
        query: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        page_type: &str,
        page_param: &str,
        feed_type: &str,
        sort: &str,
        is_strict: u32,
        category: &str,
        page_context: &str,
    ) -> Result<Value, String> {
        let mut params = vec![
            ("type", search_type.to_string()),
            ("searchValue", query.to_string()),
            ("page", page.to_string()),
        ];
        if !first_item.is_empty() {
            params.push(("firstItem", first_item.to_string()));
        }
        if !last_item.is_empty() {
            params.push(("lastItem", last_item.to_string()));
        }
        if !page_context.is_empty() {
            params.push(("pageContext", page_context.to_string()));
        }

        if search_type == "ershou" {
            params = vec![
                ("type", "ershou".to_string()),
                ("sort", sort.to_string()),
                ("searchValue", query.to_string()),
                ("status", "1".to_string()),
                ("deal_type", "all".to_string()),
                ("city_code", String::new()),
                ("is_link", String::new()),
                ("ershou_type", page_type.to_string()),
                ("product_id", page_param.to_string()),
                ("tags", String::new()),
                ("page", page.to_string()),
            ];
            if !first_item.is_empty() {
                params.push(("firstItem", first_item.to_string()));
            }
            if !last_item.is_empty() {
                params.push(("lastItem", last_item.to_string()));
            }
        } else if search_type == "feed" || search_type == "ask" {
            if !feed_type.is_empty() {
                params.push(("feedType", feed_type.to_string()));
            }
            // 精确搜索与官方一致，明确发送 sort="" 和 isStrict=1。
            if (search_type == "feed" && is_strict == 1) || !sort.is_empty() {
                params.push(("sort", sort.to_string()));
            }
            if !page_type.is_empty() {
                params.push(("pageType", page_type.to_string()));
            }
            if !page_param.is_empty() {
                params.push(("pageParam", page_param.to_string()));
            }
            params.push(("isStrict", is_strict.to_string()));
            params.push(("showAnonymous", "-1".to_string()));
        } else {
            if search_type == "apk" || search_type == "game" {
                params.push(("cat", category.to_string()));
                params.push(("sort", sort.to_string()));
            } else {
                if !category.is_empty() {
                    params.push(("category", category.to_string()));
                }
                if !sort.is_empty() {
                    params.push(("sort", sort.to_string()));
                }
            }
            params.push(("showAnonymous", "-1".to_string()));
        }

        let raw = self.api_get("/v6/search", &params).await?;
        Ok(Self::wrap_sanitized_search_data(&raw))
    }

    /// 搜索页热门词，接口与 APK 的 type=hotSearch 请求一致
    pub async fn get_hot_searches(&self, refresh: bool) -> Result<Value, String> {
        let refresh_value = if refresh { "1" } else { "0" };
        let raw = self
            .api_get(
                "/v6/search",
                &[
                    ("type", "hotSearch".to_string()),
                    ("refresh", refresh_value.to_string()),
                    ("returnType", "all".to_string()),
                ],
            )
            .await?;
        Ok(Self::wrap_sanitized_search_data(&raw))
    }

    pub async fn get_sub_replies(
        &self,
        feed_id: &str,
        reply_id: &str,
        page: u32,
    ) -> Result<Value, String> {
        self.get_sub_replies_paged(feed_id, reply_id, page, "")
            .await
    }

    /// 获取指定一级评论的楼中楼。
    ///
    /// 动态评论点击“查看会话”后，APK 的 `FeedReplyDetailFragment` 调用
    /// `au1.m14568 -> kb1.m52086`：把父评论 ID 放在 `id`，使用
    /// `feedType=feed_reply`，再用 `firstItem` / `lastItem` 继续分页。这里复用
    /// 公共请求层，使登录 Cookie、游客设备码和设备请求头与一级评论保持一致。
    pub async fn get_sub_replies_paged(
        &self,
        feed_id: &str,
        reply_id: &str,
        page: u32,
        last_item: &str,
    ) -> Result<Value, String> {
        self.get_sub_replies_with_captcha(feed_id, reply_id, page, last_item, None, None).await
    }

    pub async fn get_sub_replies_with_captcha(
        &self,
        _feed_id: &str,
        reply_id: &str,
        page: u32,
        last_item: &str,
        post_token: Option<&str>,
        post_token_field: Option<&str>,
    ) -> Result<Value, String> {
        let target_reply_id = reply_id.trim();
        if target_reply_id.is_empty() {
            return Err("评论 ID 不能为空".to_string());
        }

        let mut query = vec![
            ("id", target_reply_id.to_string()),
            ("listType", String::new()),
            ("page", page.to_string()),
            ("discussMode", "0".to_string()),
            ("feedType", "feed_reply".to_string()),
            ("blockStatus", "0".to_string()),
            ("fromFeedAuthor", "0".to_string()),
        ];
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.trim().to_string()));
        }

        append_comment_captcha_token(&mut query, post_token, post_token_field);
        let validation = post_token.filter(|_| post_token_field.is_some_and(|field| field.trim().is_empty()));
        let raw = self.request_api_from_with_validation("https://api.coolapk.com", Method::GET, "/v6/feed/replyList", &query, None, validation).await?;
        let data_arr = comment_list_data(raw)?;

        let mut cleaned_replies = Vec::new();
        for r in data_arr {
            let Some(obj) = r.as_object() else {
                continue;
            };

            let item_id = obj.get("id").map(value_to_string).unwrap_or_default();
            if item_id.is_empty() || item_id == target_reply_id {
                continue;
            }

            let item_rid = obj.get("rid").map(value_to_string).unwrap_or_default();
            let item_rrid = obj.get("rrid").map(value_to_string).unwrap_or_default();
            // 接口已经按 rid 限定了范围；保留无层级字段的有效评论，同时排除
            // 明确属于其他父评论的卡片。
            if (item_rid != target_reply_id && item_rrid != target_reply_id)
                && (!item_rid.is_empty() || !item_rrid.is_empty())
            {
                continue;
            }

            let user_info = obj.get("userInfo").or_else(|| obj.get("user"));
            let username = obj
                .get("username")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    user_info
                        .and_then(|u| u.get("username"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("");

            let raw_avatar = obj
                .get("userAvatar")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    user_info
                        .and_then(|u| u.get("userAvatar"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("");

            let avatar = if raw_avatar.starts_with("http") {
                raw_avatar.to_string()
            } else if !raw_avatar.is_empty() {
                format!(
                    "https://image.coolapk.com/{}",
                    raw_avatar.trim_start_matches('/')
                )
            } else {
                String::new()
            };

            let message = obj
                .get("message")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("description").and_then(|v| v.as_str()))
                .unwrap_or("");

            let device_title = obj
                .get("device_title")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("deviceTitle").and_then(|v| v.as_str()))
                .or_else(|| obj.get("device_name").and_then(|v| v.as_str()))
                .or_else(|| obj.get("device").and_then(|v| v.as_str()))
                .unwrap_or("");

            let user_level = user_info
                .and_then(|u| u.get("level"))
                .or_else(|| obj.get("level"))
                .map(value_to_string)
                .unwrap_or_default();

            let user_action_like = obj
                .get("userAction")
                .and_then(|ua| ua.get("like"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            cleaned_replies.push(json!({
                "id": item_id,
                "fid": obj.get("fid").map(value_to_string).unwrap_or_default(),
                "rid": item_rid,
                "rrid": item_rrid,
                "uid": obj.get("uid").map(value_to_string).or_else(|| user_info.and_then(|u| u.get("uid")).map(value_to_string)).unwrap_or_default(),
                "username": username,
                "rusername": obj.get("rusername").and_then(|v| v.as_str()).unwrap_or(""),
                "replyUsername": obj.get("rusername").and_then(|v| v.as_str()).unwrap_or(""),
                "userAvatar": avatar,
                "userLevel": user_level,
                "verifyTitle": user_info.and_then(|u| u.get("verify_title")).and_then(|v| v.as_str()).or_else(|| obj.get("verify_title").and_then(|v| v.as_str())).unwrap_or(""),
                "deviceTitle": device_title,
                "message": message,
                "pic": obj.get("pic").and_then(|v| v.as_str()).unwrap_or(""),
                "picArr": obj.get("picArr").cloned().unwrap_or(json!([])),
                "images": obj.get("images").cloned().unwrap_or(json!([])),
                "dateline": obj.get("dateline").cloned().unwrap_or(json!(0)),
                "infoHtml": obj.get("dateline_text").and_then(|v| v.as_str()).or_else(|| obj.get("infoHtml").and_then(|v| v.as_str())).unwrap_or(""),
                "floor": obj.get("floor").map(value_to_string).or_else(|| obj.get("rank").map(value_to_string)).unwrap_or_default(),
                "ipLocation": obj.get("ipLocation").and_then(|v| v.as_str()).or_else(|| obj.get("ip_location").and_then(|v| v.as_str())).or_else(|| obj.get("location").and_then(|v| v.as_str())).unwrap_or(""),
                "isFeedAuthor": obj.get("isFeedAuthor").cloned().unwrap_or(json!(0)),
                "feedUid": obj.get("feedUid").map(value_to_string).unwrap_or_default(),
                "likenum": obj.get("likenum").and_then(|v| v.as_u64()).unwrap_or(0),
                "userAction": { "like": user_action_like },
                "replyRows": obj.get("replyRows").cloned().unwrap_or(json!([])),
                "replyRowsCount": obj.get("replynum").and_then(|v| v.as_u64()).or_else(|| obj.get("replyRowsCount").and_then(|v| v.as_u64())).unwrap_or(0),
                "replyRowsMore": obj.get("replyRowsMore").cloned().unwrap_or(json!(0)),
                "targetRow": obj.get("targetRow").cloned().unwrap_or(json!(null))
            }));
        }

        Ok(json!({ "code": 200, "data": cleaned_replies }))
    }

    // 8. 楼层评论：对应 APK 的 GET /v6/feed/replyList 分页请求。
    // 每一页只走主接口；登录时由公共请求层附带 Cookie，未登录时使用游客设备身份。
    // 空页原样返回给上层作为分页结束信号，不切换公开主机、备用主机，也不把热门评论
    // 混入普通评论分页。
    pub async fn get_feed_replies(&self, feed_id: &str, page: u32) -> Result<Value, String> {
        self.get_feed_replies_paged(feed_id, page, "", "", "lastupdate_desc", 0)
            .await
    }

    pub async fn get_feed_replies_paged(
        &self,
        feed_id: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        list_type: &str,
        from_feed_author: u32,
    ) -> Result<Value, String> {
        self.get_feed_replies_with_captcha(feed_id, page, first_item, last_item, list_type, from_feed_author, None, None).await
    }

    pub async fn get_feed_replies_with_captcha(
        &self,
        feed_id: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
        list_type: &str,
        from_feed_author: u32,
        post_token: Option<&str>,
        post_token_field: Option<&str>,
    ) -> Result<Value, String> {
        let requested_list_type = list_type.trim();
        if !requested_list_type.is_empty()
            && !matches!(
                requested_list_type,
                "lastupdate_desc" | "dateline_desc" | "popular"
            )
        {
            return Err(format!("不支持的评论排序类型：{requested_list_type}"));
        }
        if from_feed_author > 1 {
            return Err(format!("不支持的楼主评论筛选值：{from_feed_author}"));
        }

        // APK 的 AUTHOR 筛选会把 listType 置空，仅使用 fromFeedAuthor=1；普通请求
        // 缺省时则使用 ReplyListV13 的默认排序 lastupdate_desc。
        let effective_list_type = if requested_list_type.is_empty() && from_feed_author == 0 {
            "lastupdate_desc"
        } else {
            requested_list_type
        };
        let mut query = vec![("id", feed_id.to_string())];
        if !effective_list_type.is_empty() {
            query.push(("listType", effective_list_type.to_string()));
        }
        query.push(("page", page.to_string()));
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.to_string()));
        }
        query.extend([
            ("discussMode", "1".to_string()),
            ("feedType", "feed".to_string()),
            ("blockStatus", "0".to_string()),
            ("fromFeedAuthor", from_feed_author.to_string()),
        ]);
        append_comment_captcha_token(&mut query, post_token, post_token_field);
        let validation = post_token.filter(|_| post_token_field.is_some_and(|field| field.trim().is_empty()));
        let raw = self.request_api_from_with_validation("https://api.coolapk.com", Method::GET, "/v6/feed/replyList", &query, None, validation).await?;
        let data_arr = comment_list_data(raw)?;

        let mut cleaned_replies = Vec::new();
        {
            for r in data_arr {
                if let Some(obj) = r.as_object() {
                    let reply_id = obj
                        .get("id")
                        .and_then(|value| {
                            value
                                .as_str()
                                .map(ToString::to_string)
                                .or_else(|| value.as_u64().map(|number| number.to_string()))
                        })
                        .unwrap_or_default();
                    // replyList 会夹带没有评论 ID 的分隔卡，不能计入一级评论数量。
                    if reply_id.is_empty() {
                        continue;
                    }
                    let user_info = obj.get("userInfo").or_else(|| obj.get("user"));
                    let username = obj
                        .get("username")
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            user_info
                                .and_then(|u| u.get("username"))
                                .and_then(|v| v.as_str())
                        })
                        .unwrap_or("酷友");

                    let raw_avatar = obj
                        .get("userAvatar")
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            user_info
                                .and_then(|u| u.get("userAvatar"))
                                .and_then(|v| v.as_str())
                        })
                        .unwrap_or("");

                    let avatar = if raw_avatar.starts_with("http") {
                        raw_avatar.to_string()
                    } else if !raw_avatar.is_empty() {
                        format!(
                            "https://image.coolapk.com/{}",
                            raw_avatar.trim_start_matches('/')
                        )
                    } else {
                        String::new()
                    };

                    let message = obj
                        .get("message")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("description").and_then(|v| v.as_str()))
                        .unwrap_or("");

                    let device_title = obj
                        .get("device_title")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("deviceTitle").and_then(|v| v.as_str()))
                        .or_else(|| obj.get("device_name").and_then(|v| v.as_str()))
                        .or_else(|| obj.get("device").and_then(|v| v.as_str()))
                        .unwrap_or("");

                    let user_level = user_info
                        .and_then(|u| u.get("level"))
                        .or_else(|| obj.get("level"))
                        .map(value_to_string)
                        .unwrap_or_default();

                    let reply_rows = obj.get("replyRows").cloned().unwrap_or(json!([]));
                    let reply_rows_count = obj
                        .get("replyRowsCount")
                        .and_then(|v| v.as_u64())
                        .or_else(|| reply_rows.as_array().map(|rows| rows.len() as u64))
                        .unwrap_or(0);
                    let reply_num = obj.get("replynum").cloned().unwrap_or(json!(0));
                    let reply_rows_more = obj.get("replyRowsMore").cloned().unwrap_or(json!(0));

                    let user_action_like = obj
                        .get("userAction")
                        .and_then(|ua| ua.get("like"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    cleaned_replies.push(json!({
            "id": reply_id,
            "fid": obj.get("fid").map(value_to_string).unwrap_or_default(),
            "rid": obj.get("rid").map(value_to_string).unwrap_or_default(),
            "rrid": obj.get("rrid").map(value_to_string).unwrap_or_default(),
            "uid": obj.get("uid").map(value_to_string).or_else(|| user_info.and_then(|u| u.get("uid")).map(value_to_string)).unwrap_or_default(),
            "username": username,
            "rusername": obj.get("rusername").and_then(|v| v.as_str()).unwrap_or(""),
            "userAvatar": avatar,
            "userLevel": user_level,
            "verifyTitle": user_info.and_then(|u| u.get("verify_title")).and_then(|v| v.as_str()).or_else(|| obj.get("verify_title").and_then(|v| v.as_str())).unwrap_or(""),
            "deviceTitle": device_title,
            "message": message,
            "pic": obj.get("pic").and_then(|v| v.as_str()).unwrap_or(""),
            "picArr": obj.get("picArr").cloned().unwrap_or(json!([])),
            "images": obj.get("images").cloned().unwrap_or(json!([])),
            "dateline": obj.get("dateline").cloned().unwrap_or(json!(0)),
            "infoHtml": obj.get("dateline_text").and_then(|v| v.as_str()).or_else(|| obj.get("infoHtml").and_then(|v| v.as_str())).unwrap_or(""),
            "floor": obj.get("floor").map(value_to_string).or_else(|| obj.get("rank").map(value_to_string)).unwrap_or_default(),
            "ipLocation": obj.get("ipLocation").and_then(|v| v.as_str()).or_else(|| obj.get("ip_location").and_then(|v| v.as_str())).or_else(|| obj.get("location").and_then(|v| v.as_str())).unwrap_or(""),
            "isFeedAuthor": obj.get("isFeedAuthor").cloned().unwrap_or(json!(0)),
            "feedUid": obj.get("feedUid").map(value_to_string).unwrap_or_default(),
            "likenum": obj.get("likenum").and_then(|v| v.as_u64()).unwrap_or(0),
            "userAction": { "like": user_action_like },
            "replyRows": reply_rows,
            "replyRowsCount": reply_rows_count,
            "replynum": reply_num,
            "replyRowsMore": reply_rows_more,
            "targetRow": obj.get("targetRow").cloned().unwrap_or(json!(null))
        }));
                }
            }
        }

        Ok(json!({ "code": 200, "data": cleaned_replies }))
    }

    pub async fn get_board_feeds(&self, board_tag: &str, page: u32) -> Result<Value, String> {
        self.get_board_feeds_with_mode(board_tag, page, false).await
    }

    pub async fn get_board_feeds_with_mode(
        &self,
        board_tag: &str,
        page: u32,
        guest: bool,
    ) -> Result<Value, String> {
        let tag = board_tag.trim();
        if tag == "/main/headline" || tag == "headline" || tag == "V9_HOME_TAB_HEADLINE" {
            if guest {
                return Err("头条页不支持后台预取".to_string());
            }
            return self.get_headline_feeds(page).await;
        }
        if tag == "/main/indexV8" || tag == "index_v8" {
            if guest {
                return Err("头条页不支持后台预取".to_string());
            }
            return self.get_index_v8_feeds(page).await;
        }

        let url_param = if tag.starts_with("/page?url=") {
            tag.to_string()
        } else if tag.starts_with('/') || tag.starts_with('#') {
            tag.to_string()
        } else {
            format!("/page?url={tag}")
        };

        let raw = self
            .api_get_for_list(
                "/v6/page/dataList",
                &[
                    ("url", url_param),
                    ("page", page.to_string()),
                ],
                guest,
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    // 获取酷安游戏中心列表/热门与分类榜单
    //
    // 实测确认：酷安已废弃 #/game/* 系列 dataList 路由（返回空数据），
    // 官方接口中唯一可用的游戏数据源是「游戏专项搜索」 GET /v6/search?type=game
    // （返回实体 apktype=2 / apkTypeName=游戏），按分类关键词拉取。
    pub async fn get_game_list(&self, page: u32, game_type: &str) -> Result<Value, String> {
        let query = match game_type {
            "hot" => "手游",
            "new" => "新游戏",
            "single" => "单机游戏",
            "online" => "网游",
            "casual" => "休闲游戏",
            "indie" => "独立游戏",
            _ => "手游",
        };

        let search_raw = self
            .api_get(
                "/v6/search",
                &[
                    ("type", "game".to_string()),
                    ("searchValue", query.to_string()),
                    ("page", page.to_string()),
                    ("show_flag", "1".to_string()),
                ],
            )
            .await?;

        let search_apks = Self::extract_apk_list(&search_raw, "game");
        Ok(json!({ "code": 200, "data": search_apks }))
    }

    // 专项搜索游戏与 APK 软件实体
    pub async fn search_apks(&self, query: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/search",
                &[
                    ("type", "apk".to_string()),
                    ("searchValue", query.to_string()),
                    ("page", page.to_string()),
                    ("show_flag", "1".to_string()),
                ],
            )
            .await?;

        let apks = Self::extract_apk_list(&raw, "all");
        Ok(json!({ "code": 200, "data": apks }))
    }

    // 游戏专项搜索（仅返回游戏实体，type=game）
    pub async fn search_games(&self, query: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/search",
                &[
                    ("type", "game".to_string()),
                    ("searchValue", query.to_string()),
                    ("page", page.to_string()),
                    ("show_flag", "1".to_string()),
                ],
            )
            .await?;

        let apks = Self::extract_apk_list(&raw, "game");
        Ok(json!({ "code": 200, "data": apks }))
    }

    // 获取酷安应用中心列表/热门与分类榜单
    //
    // 实测确认：/v6/page/dataList?url=#/apk/rankList 与 #/apk/newestList 有效，
    // 但 url 上的 type= 参数被服务端忽略（tools/social/media/theme 等均返回默认推荐榜），
    // #/apk/category?catId=... 已废弃（返回空）。分类榜单改用应用搜索 type=apk 拉取。
    pub async fn get_app_list(&self, page: u32, cat: &str) -> Result<Value, String> {
        let page_url = match cat {
            "recommend" => "#/apk/rankList",
            "newest" => "#/apk/newestList",
            _ => "",
        };

        if !page_url.is_empty() {
            let raw = self
                .api_get(
                    "/v6/page/dataList",
                    &[("url", page_url.to_string()), ("page", page.to_string())],
                )
                .await;

            let apks = match raw {
                Ok(ref json_val) => Self::extract_apk_list(json_val, "app"),
                Err(_) => Vec::new(),
            };

            if !apks.is_empty() {
                return Ok(json!({ "code": 200, "data": apks }));
            }
        }

        let query = match cat {
            "tools" => "系统工具",
            "social" => "社交聊天",
            "media" => "影音播放",
            "beauty" => "主题美化",
            "newest" => "应用",
            _ => "常用应用",
        };

        let search_raw = self
            .api_get(
                "/v6/search",
                &[
                    ("type", "apk".to_string()),
                    ("searchValue", query.to_string()),
                    ("page", page.to_string()),
                    ("show_flag", "1".to_string()),
                ],
            )
            .await?;

        let search_apks = Self::extract_apk_list(&search_raw, "app");
        Ok(json!({ "code": 200, "data": search_apks }))
    }

    /// 图片代理与通用文件下载共用认证、来源站点和网络策略。
    pub(super) fn resource_request(&self, source_url: &str, upgrade_http: bool) -> Result<reqwest::RequestBuilder, String> {
        let mut url =
            reqwest::Url::parse(source_url).map_err(|e| format!("invalid image URL: {e}"))?;
        let scheme = url.scheme().to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err("only HTTP/HTTPS image schemes are allowed".to_string());
        }

        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        if host.is_empty()
            || host == "localhost"
            || host == "127.0.0.1"
            || host == "0.0.0.0"
            || host.starts_with("192.168.")
            || host.starts_with("10.")
            || host.starts_with("172.16.")
            || host.starts_with("172.17.")
            || host.starts_with("172.18.")
            || host.starts_with("172.19.")
            || host.starts_with("172.20.")
            || host.starts_with("172.30.")
            || host.starts_with("172.31.")
        {
            return Err(format!("disallowed private or local host: {host}"));
        }

        if upgrade_http && url.scheme() == "http" {
            url.set_scheme("https")
                .map_err(|_| "failed to upgrade image URL to HTTPS".to_string())?;
        }

        // 酷安 API 域下的图片接口（如 /v6/message/showImage）需要完整的 App 指纹头
        // （X-Sdk-Int/X-App-Id/X-App-Version 等）+ Token 认证，必须复用主 client；
        // 其余 CDN 图片用独立浏览器 UA 客户端（浏览器 UA 访问 image.coolapk.com 会被 CDN 放行）
        let mut req = if host == "api.coolapk.com" || host == "api2.coolapk.com" {
            // 私信图片接口与普通 API 一样校验设备码和自定义设备信息，
            // 不能只带 Token，否则持久缓存改为原生代理加载后会出现空白图片。
            let token = self.get_token()?;
            self.apply_device_profile(
                self.api_client()?
                    .get(url)
                    .timeout(std::time::Duration::from_secs(20))
                    .header(
                        "Accept",
                        "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
                    )
                    .header("X-Requested-With", "XMLHttpRequest")
                    .header("X-App-Token", token),
            )?
        } else {
            let referer = if is_weibo_image_host(&host) {
                "https://weibo.com/"
            } else {
                "https://www.coolapk.com/"
            };
            session(url.as_str(), SessionPolicy::Follow)?
                .get(url)
                .timeout(std::time::Duration::from_secs(12))
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .header("Referer", referer)
        };

        if let Ok(guard) = self.user_cookie.read() {
            if let Some(cookie_str) = guard.as_ref() {
                // 登录 Cookie 只允许发送给酷安官方域；第三方 CDN/图片地址不得携带，
                // 否则发帖人可控的图片链接会把登录凭据送到攻击者服务器
                if is_coolapk_host(&host) {
                    let cookie = cookie_without_ddid(cookie_str);
                    req = req.header("Cookie", cookie);
                }
            }
        }

        Ok(req)
    }

    /// 流式保存任意 HTTP 文件，返回服务器文件名、类型及最终地址供保存端命名。
    pub async fn download_file_to(&self, source_url: &str, path: &std::path::Path) -> Result<(String, String, String), String> {
        let response = self.resource_request(source_url, false)?.header("Accept", "*/*").timeout(std::time::Duration::from_secs(3600)).send().await.map_err(|error| format!("下载文件失败：{error}"))?;
        Self::write_download_response(response, path).await
    }

    /// 原样写入响应体，HTTP 错误或中途断流不能报告保存成功。
    pub(super) async fn write_download_response(mut response: reqwest::Response, path: &std::path::Path) -> Result<(String, String, String), String> {
        use tokio::io::AsyncWriteExt;
        if !response.status().is_success() { return Err(format!("文件下载返回 HTTP {}", response.status())); }
        let mime_type = response.headers().get("content-type").and_then(|value| value.to_str().ok()).unwrap_or("application/octet-stream").to_string();
        let disposition = response.headers().get("content-disposition").and_then(|value| value.to_str().ok()).unwrap_or_default().to_string();
        let final_url = response.url().to_string();
        let mut file = tokio::fs::OpenOptions::new().write(true).create_new(true).open(path).await.map_err(|error| format!("创建下载临时文件失败：{error}"))?;
        while let Some(chunk) = response.chunk().await.map_err(|error| format!("读取下载内容失败：{error}"))? {
            file.write_all(&chunk).await.map_err(|error| format!("写入下载文件失败：{error}"))?;
        }
        file.flush().await.map_err(|error| format!("刷新下载文件失败：{error}"))?;
        file.sync_all().await.map_err(|error| format!("保存下载文件失败：{error}"))?;
        Ok((mime_type, disposition, final_url))
    }
}
