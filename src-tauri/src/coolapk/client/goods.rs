use super::*;

impl CoolapkClient {
    /// 好物搜索热词
    /// 数据来源: GET /v6/goods/searchHotWords
    pub async fn get_goods_search_hot_words(&self) -> Result<Value, String> {
        wrap_api_data(self.api_get("/v6/goods/searchHotWords", &[]).await?)
    }

    /// 搜索商品/好物（京东/淘宝/拼多多等 pear_goods）
    /// 数据来源: GET /v6/goods/search
    pub async fn search_goods(
        &self,
        keyword: &str,
        sort_name: &str,
        sort: &str,
        is_coupon: u32,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/goods/search",
                &[
                    ("keyword", keyword.to_string()),
                    ("sortName", sort_name.to_string()),
                    ("sort", sort.to_string()),
                    ("isCoupon", is_coupon.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_entity_rows(&raw) }))
    }

    /// 对照 APK goods/addGoods，将商城商品链接转换为可附加到动态的 FeedGoods。
    pub async fn prepare_goods_by_url(&self, url: &str) -> Result<Value, String> {
        if url.trim().is_empty() { return Err("商品缺少商城链接".to_string()); }
        wrap_api_data(self.api_post("/v6/goods/addGoods", &[], &[("url", url.to_string())]).await?)
    }

    /// 商品/好物详情（FeedGoods）
    /// 数据来源: GET /v6/goods/detail
    pub async fn get_goods_detail(&self, goods_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/goods/detail", &[("id", goods_id.to_string())])
                .await?,
        )
    }

    /// 好物清单分类（list_type）
    /// 数据来源: GET /v6/goodsList/listType
    pub async fn get_goods_list_types(&self) -> Result<Value, String> {
        let raw = self.api_get("/v6/goodsList/listType", &[]).await?;
        Ok(json!({ "code": 200, "data": Self::extract_entity_rows(&raw) }))
    }

    /// 好物清单/好物榜条目列表
    /// uid 指定某用户创建的清单；goods_id 指定清单内的商品条目。
    /// 数据来源: GET /v6/goodsList/list
    pub async fn get_goods_list(
        &self,
        uid: &str,
        goods_id: &str,
        page: u32,
    ) -> Result<Value, String> {
        let mut query: Vec<(&str, String)> = vec![("page", page.to_string())];
        if !uid.is_empty() {
            query.push(("uid", uid.to_string()));
        }
        if !goods_id.is_empty() {
            query.push(("goodsId", goods_id.to_string()));
        }
        let raw = self.api_get("/v6/goodsList/list", &query).await?;
        Ok(json!({ "code": 200, "data": Self::extract_entity_rows(&raw) }))
    }

    /// 用户商品店铺条目列表
    /// 数据来源: GET /v6/goods/goodsStoreItemList
    pub async fn get_goods_store_items(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/goods/goodsStoreItemList",
                &[("uid", uid.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_entity_rows(&raw) }))
    }

    /// 用户产品专辑列表
    /// 数据来源: GET /v6/user/productAlbumList
    pub async fn get_product_albums(&self, uid: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/user/productAlbumList",
                &[("uid", uid.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_entity_rows(&raw) }))
    }

    /// 我的好物动态（全部/想买/买过）
    /// 数据来源: GET /v6/page/dataList?url=/goods/goodsFeedList?uid=..&type=..
    pub async fn get_my_goods_feeds(
        &self,
        uid: &str,
        goods_type: &str,
        page: u32,
    ) -> Result<Value, String> {
        let type_query = if goods_type.is_empty() || goods_type == "all" {
            String::new()
        } else {
            format!("&type={}", goods_type)
        };
        let raw = self
            .api_get(
                "/v6/page/dataList",
                &[
                    (
                        "url",
                        format!("/goods/goodsFeedList?uid={}{}", uid, type_query),
                    ),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_entity_rows(&raw) }))
    }

    /// 创建好物清单
    /// 数据来源: POST /v6/goodsList/create
    #[allow(clippy::too_many_arguments)]
    pub async fn create_goods_list(
        &self,
        title: &str,
        message: &str,
        cover: &str,
        top_limit: u32,
        is_open_vote: u32,
        list_type: &str,
        target_id: &str,
        target_type: &str,
    ) -> Result<Value, String> {
        let mut form: Vec<(&str, String)> = vec![
            ("title", title.to_string()),
            ("message", message.to_string()),
            ("cover", cover.to_string()),
            ("top_limit", top_limit.to_string()),
            ("is_open_vote", is_open_vote.to_string()),
            ("list_type", list_type.to_string()),
        ];
        if !target_id.is_empty() {
            form.push(("targetId", target_id.to_string()));
        }
        if !target_type.is_empty() {
            form.push(("targetType", target_type.to_string()));
        }
        wrap_api_data(self.api_post("/v6/goodsList/create", &[], &form).await?)
    }

    /// 编辑好物清单
    /// 数据来源: POST /v6/goodsList/edit
    pub async fn edit_goods_list(
        &self,
        id: &str,
        title: &str,
        message: &str,
        cover: &str,
        top_limit: u32,
        is_open_vote: u32,
        list_type: &str,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/goodsList/edit",
                &[],
                &[
                    ("id", id.to_string()),
                    ("title", title.to_string()),
                    ("message", message.to_string()),
                    ("cover", cover.to_string()),
                    ("top_limit", top_limit.to_string()),
                    ("is_open_vote", is_open_vote.to_string()),
                    ("list_type", list_type.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 向好物清单添加商品
    /// 数据来源: POST /v6/goodsList/addGoods
    pub async fn add_goods_to_goods_list(
        &self,
        feed_id: &str,
        goods_id: &str,
        note: &str,
        pic: &str,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/goodsList/addGoods",
                &[],
                &[
                    ("feedId", feed_id.to_string()),
                    ("goodsId", goods_id.to_string()),
                    ("note", note.to_string()),
                    ("pic", pic.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 删除好物清单条目
    /// 数据来源: POST /v6/goodsList/deleteItems
    pub async fn delete_goods_list_items(
        &self,
        cancel_feed_id: &str,
        goods_id: &str,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/goodsList/deleteItems",
                &[],
                &[
                    ("cancelFeedId", cancel_feed_id.to_string()),
                    ("goodsId", goods_id.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 编辑好物清单中的商品条目
    /// 数据来源: POST /v6/goodsList/editGoodsItem
    pub async fn edit_goods_list_item(
        &self,
        feed_id: &str,
        goods_id: &str,
        note: &str,
        pic: &str,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/goodsList/editGoodsItem",
                &[],
                &[
                    ("feedId", feed_id.to_string()),
                    ("goodsId", goods_id.to_string()),
                    ("note", note.to_string()),
                    ("pic", pic.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 好物清单投票
    /// 数据来源: POST /v6/goodsList/vote
    pub async fn vote_goods_list_item(
        &self,
        id: &str,
        item_id: &str,
        value: i32,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/goodsList/vote",
                &[],
                &[
                    ("id", id.to_string()),
                    ("item_id", item_id.to_string()),
                    ("value", value.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 将动态绑定到好物清单
    /// 数据来源: POST /v6/goodsList/bindFeedToGoodsList
    pub async fn bind_feed_to_goods_list(
        &self,
        feed_id: &str,
        goods_list_id: &str,
    ) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/goodsList/bindFeedToGoodsList",
                &[],
                &[
                    ("feedId", feed_id.to_string()),
                    ("goodsListId", goods_list_id.to_string()),
                ],
            )
            .await?,
        )
    }
}
