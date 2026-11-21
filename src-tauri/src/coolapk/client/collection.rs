use super::*;

impl CoolapkClient {
    /// 收藏列表（需登录）
    /// 数据来源: GET /v6/favorite/list，type 支持 feed/apk/album
    pub async fn get_favorite_list(
        &self,
        fav_type: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let mut query = vec![("type", fav_type.to_string()), ("page", page.to_string())];
        if !first_item.is_empty() {
            query.push(("firstItem", first_item.to_string()));
        }
        if !last_item.is_empty() {
            query.push(("lastItem", last_item.to_string()));
        }
        let raw = self
            .api_get("/v6/favorite/list", &query)
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 查询动态所在的收藏单，供云端收藏/取消收藏使用。
    /// 数据来源: GET /v6/collection/list?uid=&id={feedId}&type=feed&showDefault=1
    pub async fn get_feed_collection_status(&self, feed_id: &str) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/collection/list",
                &[
                    ("uid", String::new()),
                    ("id", feed_id.to_string()),
                    ("type", "feed".to_string()),
                    ("showDefault", "1".to_string()),
                    ("page", "1".to_string()),
                    ("firstItem", String::new()),
                    ("lastItem", String::new()),
                ],
            )
            .await?;
        wrap_api_data(raw)
    }

    /// 把动态加入或移出收藏单。
    /// 数据来源: POST /v6/collection/addItem
    pub async fn update_collection_item(
        &self,
        collection_ids: &str,
        cancel_ids: &str,
        target_id: &str,
        feed_type: &str,
        trace: &str,
    ) -> Result<Value, String> {
        let form = [
            ("id", collection_ids.to_string()),
            ("cancelId", cancel_ids.to_string()),
            ("targetId", target_id.to_string()),
            ("type", feed_type.to_string()),
            ("trace", trace.to_string()),
        ];
        wrap_api_data(self.api_post("/v6/collection/addItem", &[], &form).await?)
    }

    /// 收藏单（收藏夹）列表
    /// 数据来源: GET /v6/collection/list?uid={uid}&showDefault=1&page={page}&firstItem={firstItem}&lastItem={lastItem}
    /// `showDefault=1` 用于把账号的系统默认收藏单一并返回；否则接口只返回用户创建的收藏单。
    /// APK 用首项和末项的 entityId 作为分页游标。
    pub async fn get_collection_list(&self, uid: &str, page: u32, first_item: &str, last_item: &str) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/collection/list",
                &build_collection_list_query(uid, page, first_item, last_item),
            )
            .await?;
        let mut collections = Vec::new();
        if let Some(arr) = raw.get("data").and_then(|v| v.as_array()) {
            for item in arr {
                let obj = match item.as_object() {
                    Some(o) => o,
                    None => continue,
                };
                if obj.get("entityType").and_then(|v| v.as_str()) != Some("collection") {
                    continue;
                }
                let id = get_str_by_keys(obj, &["id", "collectionId"]).unwrap_or_default();
                let title = get_str_by_keys(obj, &["title", "name"]).unwrap_or_default();
                if id.is_empty() || title.is_empty() {
                    continue;
                }
                let raw_cover = get_str_by_keys(obj, &["cover", "coverPic", "cover_pic", "pic", "logo"]).unwrap_or_default();
                let cover = if raw_cover.starts_with("http") {
                    raw_cover
                } else if !raw_cover.is_empty() {
                    format!(
                        "https://image.coolapk.com/{}",
                        raw_cover.trim_start_matches('/')
                    )
                } else {
                    String::new()
                };
                let collection_uid = get_str_by_keys(obj, &["uid", "userId", "user_id"]).unwrap_or_default();
                let source_id = get_str_by_keys(obj, &["sourceId", "source_id"]).unwrap_or_default();
                let is_open = obj
                    .get("isOpen")
                    .or_else(|| obj.get("is_open"))
                    .or_else(|| obj.get("open"))
                    .cloned()
                    .unwrap_or(json!(1));
                let default_collected = obj
                    .get("defaultCollected")
                    .or_else(|| obj.get("default_collected"))
                    .or_else(|| obj.get("isDefault"))
                    .or_else(|| obj.get("is_default"))
                    .cloned()
                    .unwrap_or(json!(0));
                collections.push(json!({
                    "id": id,
                    "entityId": get_str_by_keys(obj, &["entityId", "entity_id"]).unwrap_or_else(|| id.clone()),
                    "title": title,
                    "cover": cover.clone(),
                    "coverPic": cover,
                    "description": get_str_by_keys(obj, &["description", "summary"]).unwrap_or_default(),
                    "itemNum": get_u64_by_keys(obj, &["itemNum", "item_num", "itemnum", "feedNum", "feed_num", "count"]),
                    "favnum": get_u64_by_keys(obj, &["favnum", "fav_num", "likeNum", "like_num"]),
                    "follownum": get_u64_by_keys(obj, &["follownum", "followNum", "follow_num"]),
                    "uid": collection_uid,
                    "sourceId": source_id,
                    "isOpen": is_open,
                    "defaultCollected": default_collected
                }));
            }
        }
        Ok(json!({ "code": 200, "data": collections }))
    }

    /// 收藏单内容列表
    /// 数据来源: GET /v6/collection/itemList?id={collectionId}
    pub async fn get_collection_item_list(
        &self,
        collection_id: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/collection/itemlist",
                &[
                    ("id", collection_id.to_string()),
                    ("page", page.to_string()),
                    ("firstItem", first_item.to_string()),
                    ("lastItem", last_item.to_string()),
                    ("listType", "allFeedType".to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 收藏单详情
    /// 数据来源: GET /v6/collection/detail?id={collectionId}
    pub async fn get_collection_detail(&self, collection_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(
                "/v6/collection/detail",
                &[("id", collection_id.to_string())],
            )
            .await?,
        )
    }

    /// 创建收藏单，字段顺序和酷安 APK 的 multipart 请求保持一致。
    /// 数据来源: POST /v6/collection/create
    pub async fn create_collection(
        &self,
        title: &str,
        description: &str,
        cover: &str,
        is_open: i32,
        source_id: &str,
    ) -> Result<Value, String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("收藏单标题不能为空".to_string());
        }
        let form = reqwest::multipart::Form::new()
            .text("isOpen", if is_open == 0 { "0" } else { "1" })
            .text("pic", cover.trim().to_string())
            .text("description", description.trim().to_string())
            .text("title", title.to_string())
            .text("sourceId", source_id.trim().to_string());
        self.request_multipart_api("/v6/collection/create", form).await
    }

    /// 编辑收藏单，不能把 id 放到查询参数中，APK 会将它作为表单字段提交。
    /// 数据来源: POST /v6/collection/update
    pub async fn update_collection(
        &self,
        collection_id: &str,
        title: &str,
        description: &str,
        cover: &str,
        is_open: i32,
    ) -> Result<Value, String> {
        let collection_id = collection_id.trim();
        let title = title.trim();
        if collection_id.is_empty() || title.is_empty() {
            return Err("收藏单 ID 和标题不能为空".to_string());
        }
        let form = [
            ("id", collection_id.to_string()),
            ("title", title.to_string()),
            ("description", description.trim().to_string()),
            ("pic", cover.trim().to_string()),
            ("isOpen", if is_open == 0 { "0".to_string() } else { "1".to_string() }),
        ];
        wrap_api_data(self.api_post("/v6/collection/update", &[], &form).await?)
    }

    /// 删除收藏单；酷安服务端会同时取消其中内容的归属。
    /// 数据来源: POST /v6/collection/delete
    pub async fn delete_collection(&self, collection_id: &str) -> Result<Value, String> {
        let collection_id = collection_id.trim();
        if collection_id.is_empty() {
            return Err("收藏单 ID 不能为空".to_string());
        }
        wrap_api_data(
            self.api_post(
                "/v6/collection/delete",
                &[],
                &[("id", collection_id.to_string())],
            )
            .await?,
        )
    }

    /// 从收藏单中移除一条内容，itemId 不是动态本身的 id。
    /// 数据来源: POST /v6/collection/removeItem
    pub async fn remove_collection_item(&self, item_id: &str) -> Result<Value, String> {
        let item_id = item_id.trim();
        if item_id.is_empty() {
            return Err("收藏单条目 ID 不能为空".to_string());
        }
        wrap_api_data(
            self.api_post(
                "/v6/collection/removeItem",
                &[],
                &[("itemId", item_id.to_string())],
            )
            .await?,
        )
    }

    /// 清理收藏单中的失效内容，服务端会异步处理。
    /// 数据来源: POST /v6/collection/removeUnUseItem
    pub async fn clear_collection_invalid_items(&self, collection_id: &str) -> Result<Value, String> {
        let collection_id = collection_id.trim();
        if collection_id.is_empty() {
            return Err("收藏单 ID 不能为空".to_string());
        }
        wrap_api_data(
            self.api_post(
                "/v6/collection/removeUnUseItem",
                &[],
                &[("colId", collection_id.to_string())],
            )
            .await?,
        )
    }

    /// 关注/点赞收藏单（酷安 v6 写接口统一使用 GET）
    pub(super) async fn collection_action(&self, path: &str, collection_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get(path, &[("id", collection_id.to_string())])
                .await?,
        )
    }

    pub async fn follow_collection(&self, collection_id: &str) -> Result<Value, String> {
        self.collection_action("/v6/collection/follow", collection_id)
            .await
    }

    pub async fn unfollow_collection(&self, collection_id: &str) -> Result<Value, String> {
        self.collection_action("/v6/collection/unFollow", collection_id)
            .await
    }

    pub async fn like_collection(&self, collection_id: &str) -> Result<Value, String> {
        self.collection_action("/v6/collection/like", collection_id)
            .await
    }

    pub async fn unlike_collection(&self, collection_id: &str) -> Result<Value, String> {
        self.collection_action("/v6/collection/unLike", collection_id)
            .await
    }

    /// 关注/取消关注看看号
    /// 数据来源: GET /v6/dyh/follow?dyhId={dyhId}
    pub async fn follow_dyh(&self, dyh_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/dyh/follow", &[("dyhId", dyh_id.to_string())])
                .await?,
        )
    }

    pub async fn unfollow_dyh(&self, dyh_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/dyh/unFollow", &[("dyhId", dyh_id.to_string())])
                .await?,
        )
    }
}
