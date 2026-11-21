use super::*;

impl CoolapkClient {
    /// 看看号（官方号）详情
    /// 数据来源: GET /v6/dyh/detail
    pub async fn get_dyh_detail(&self, dyh_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/dyh/detail", &[("dyhId", dyh_id.to_string())])
                .await?,
        )
    }

    /// 看看号（官方号）列表
    /// 数据来源: GET /v6/dyh/list
    pub async fn get_dyh_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/dyh/list", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 看看号（官方号）动态列表
    /// 数据来源: GET /v6/dyhArticle/list
    pub async fn get_dyh_feeds(
        &self,
        dyh_id: &str,
        feed_type: &str,
        page: u32,
    ) -> Result<Value, String> {
        let feed_type = match feed_type {
            "square" => "square",
            _ => "all",
        };
        let raw = self
            .api_get(
                "/v6/dyhArticle/list",
                &[
                    ("dyhId", dyh_id.to_string()),
                    ("type", feed_type.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 酷友圈活动列表
    /// 数据来源: GET /v6/event/list
    pub async fn get_event_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/event/list", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 酷友圈活动详情
    /// 数据来源: GET /v6/event/detail?id={id}
    pub async fn get_event_detail(&self, event_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/event/detail", &[("id", event_id.to_string())])
                .await?,
        )
    }

    /// 我关注的动态号（看看号）列表
    /// 数据来源: GET /v6/user/dyhFollowList
    pub async fn get_dyh_follow_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/user/dyhFollowList", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 我订阅的动态号（看看号）列表
    /// 数据来源: GET /v6/user/dyhSubscribe
    pub async fn get_dyh_subscribe_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get("/v6/user/dyhSubscribe", &[("page", page.to_string())])
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 我管理的动态号（编辑者身份）列表
    /// 数据来源: GET /v6/user/editorDyhList
    pub async fn get_dyh_editor_list(&self, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/editorDyhList",
                &[
                    ("showNews", "1".to_string()),
                    ("showType", "1".to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 官方 DyhEditorFragment：收录使用编辑列表，分享到广场使用 shareType=1。
    pub async fn get_feed_share_dyh_list(&self, share_type: u32, page: u32) -> Result<Value, String> {
        let (path, params) = match share_type {
            1 => ("/v6/user/editorDyhList", vec![("showNews", "1".to_string()), ("showType", "0".to_string()), ("page", page.to_string())]),
            2 => ("/v6/dyh/list", vec![("type", String::new()), ("shareType", "1".to_string()), ("page", page.to_string())]),
            _ => return Err("无效的看看号分享类型".to_string()),
        };
        wrap_api_data(self.api_get(path, &params).await?)
    }

    /// 官方 DyhIncludeActivity：逗号分隔多选 ID，type=1 收录、type=2 分享到广场。
    pub async fn share_feed_to_dyh(&self, feed_id: &str, dyh_ids: &str, share_type: u32) -> Result<Value, String> {
        if !matches!(share_type, 1 | 2) || feed_id.is_empty() || !feed_id.bytes().all(|b| b.is_ascii_digit())
            || dyh_ids.is_empty() || dyh_ids.split(',').any(|id| id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit())) {
            return Err("无效的动态或看看号参数".to_string());
        }
        wrap_api_data(self.api_post("/v6/dyh/includeFeed", &[("dyhId", dyh_ids.to_string()), ("feedId", feed_id.to_string()), ("type", share_type.to_string())], &[]).await?)
    }

    /// 用户创建的万物清单（productAlbum）列表
    /// 数据来源: GET /v6/user/productAlbumList
    pub async fn get_user_product_albums(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/productAlbumList",
                &[("uid", uid.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 好物清单（万物清单）条目列表
    /// 数据来源: GET /v6/goodsList/list
    pub async fn get_goods_list_items(
        &self,
        uid: &str,
        goods_id: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/goodsList/list",
                &[
                    ("uid", uid.to_string()),
                    ("goodsId", goods_id.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": raw.get("data").cloned().unwrap_or(json!([])) }))
    }

    /// 创建万物清单（自定义产品清单）
    /// 数据来源: POST /v6/productAlbum/create
    ///
    /// product_items 为 JSON 数组，每项含 item_id/item_name/item_description/item_logo/item_images/display_order。
    pub async fn create_product_album(
        &self,
        title: &str,
        description: &str,
        album_type: u32,
        target_type: &str,
        target_id: &str,
        product_items: &str,
    ) -> Result<Value, String> {
        let mut owned_fields = vec![
            ("title".to_string(), title.to_string()),
            ("description".to_string(), description.to_string()),
            ("album_type".to_string(), album_type.to_string()),
            ("targetType".to_string(), target_type.to_string()),
            ("targetId".to_string(), target_id.to_string()),
        ];
        if let Ok(items) = serde_json::from_str::<Value>(product_items) {
            if let Some(arr) = items.as_array() {
                for (idx, item) in arr.iter().enumerate() {
                    let field = |key: &str, default: &str| {
                        item.get(key)
                            .and_then(|v| v.as_str())
                            .unwrap_or(default)
                            .to_string()
                    };
                    owned_fields.push((
                        format!("productItems[{idx}][id]"),
                        field("id", ""),
                    ));
                    owned_fields.push((
                        format!("productItems[{idx}][level]"),
                        field("level", "1"),
                    ));
                    owned_fields.push((
                        format!("productItems[{idx}][item_id]"),
                        field("item_id", ""),
                    ));
                    owned_fields.push((
                        format!("productItems[{idx}][item_logo]"),
                        field("item_logo", ""),
                    ));
                    owned_fields.push((
                        format!("productItems[{idx}][item_name]"),
                        field("item_name", ""),
                    ));
                    owned_fields.push((
                        format!("productItems[{idx}][item_description]"),
                        field("item_description", ""),
                    ));
                    owned_fields.push((
                        format!("productItems[{idx}][item_images]"),
                        field("item_images", ""),
                    ));
                    owned_fields.push((
                        format!("productItems[{idx}][display_order]"),
                        idx.to_string(),
                    ));
                }
            }
        }

        let form: Vec<(&str, String)> = owned_fields
            .iter()
            .map(|(key, value)| (key.as_str(), value.clone()))
            .collect();
        wrap_api_data(self.api_post("/v6/productAlbum/create", &[], &form).await?)
    }

    /// 节点（版块）动态列表
    ///
    /// 酷安新版「版块」节点通过 `/v6/page/dataList?url=#/feed/nodeFeedList` 返回
    /// 服务端驱动的 Feed 流；同时兼容 topic / product / app 等实体节点的既有端点。
    pub async fn get_node_feeds(
        &self,
        node_type: &str,
        node_id: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = match node_type {
            "topic" => {
                self.api_get(
                    "/v6/topic/tagFeedList",
                    &[("tag", node_id.to_string()), ("page", page.to_string())],
                )
                .await?
            }
            "product" => {
                self.api_get(
                    "/v6/page/dataList",
                    &[
                        ("url", "/page?url=/product/feedList".to_string()),
                        ("id", node_id.to_string()),
                        ("type", "feed".to_string()),
                        ("page", page.to_string()),
                    ],
                )
                .await?
            }
            "app" => {
                self.api_get(
                    "/v6/page/dataList",
                    &[
                        ("url", "#/feed/apkCommentList".to_string()),
                        ("id", node_id.to_string()),
                        ("sort", "lastupdate_desc".to_string()),
                        ("page", page.to_string()),
                    ],
                )
                .await?
            }
            _ => {
                self.api_get(
                    "/v6/page/dataList",
                    &[
                        (
                            "url",
                            format!("#/feed/nodeFeedList?nodeType={node_type}&nodeId={node_id}"),
                        ),
                        ("page", page.to_string()),
                    ],
                )
                .await?
            }
        };
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 应用所属动态列表（点评/讨论）
    /// 与官方 AppViewListFragment 一致：GET /v6/apk/commentList，使用 listType 排序。
    pub async fn get_apk_feeds(
        &self,
        package_name: &str,
        sort_type: &str,
        page: u32,
    ) -> Result<Value, String> {
        let sort = match sort_type {
            "lastupdate_desc" | "dateline_desc" | "popular" => sort_type,
            _ => "lastupdate_desc",
        };
        let raw = self
            .api_get(
                "/v6/apk/commentList",
                &[
                    ("id", package_name.to_string()),
                    ("listType", sort.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }
}
