use super::*;

impl CoolapkClient {
    /// 产品（数码）详情
    /// 数据来源: GET /v6/product/detail
    pub async fn get_product_detail(&self, product_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/product/detail", &[("id", product_id.to_string())])
                .await?,
        )
    }

    /// 产品（数码）所属动态列表（讨论/问答/图文/视频/交易）
    /// 数据来源: GET /v6/page/dataList?url=/page?url=/product/feedList&id={id}&type={type}&listType={list_type}
    pub async fn get_product_feeds(
        &self,
        product_id: &str,
        feed_type: &str,
        list_type: &str,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/page/dataList",
                &build_product_feeds_query(product_id, feed_type, list_type, page),
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 产品页服务端子栏目（样张、到手价等）对应的动态列表。
    pub async fn get_product_subtab_feeds(&self, product_id: &str, sub_id: &str, page: u32) -> Result<Value, String> {
        if product_id.trim().is_empty() || sub_id.trim().is_empty() || !sub_id.chars().all(|ch| ch.is_ascii_digit()) {
            return Err("产品或子栏目 ID 无效".to_string());
        }
        let raw = self.api_get("/v6/page/dataList", &build_product_subtab_query(product_id, sub_id, page)).await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 产品（数码）配置详情
    /// 数据来源: GET /v6/product/config?id={config_id}
    pub async fn get_product_config(&self, config_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/product/config", &[("id", config_id.to_string())])
                .await?,
        )
    }

    /// 将产品配置加入对比列表（需登录）
    /// 数据来源: POST /v6/product/addConfigCompare
    pub async fn add_config_compare(&self, config_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/product/addConfigCompare",
                &[],
                &[("config_id", config_id.to_string())],
            )
            .await?,
        )
    }

    /// 从产品配置对比列表移除（需登录）
    /// 数据来源: POST /v6/product/removeConfigCompare
    pub async fn remove_config_compare(&self, config_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/product/removeConfigCompare",
                &[],
                &[("config_id", config_id.to_string())],
            )
            .await?,
        )
    }

    /// 数码产品品牌列表
    /// 数据来源: GET /v6/product/brandList
    pub async fn get_product_brand_list(&self) -> Result<Value, String> {
        let raw = self.api_get("/v6/product/brandList", &[]).await?;
        Ok(Self::product_entity_page_response(&raw))
    }

    /// 数码产品分类列表
    /// 数据来源: GET /v6/product/categoryList
    pub async fn get_product_category_list(&self) -> Result<Value, String> {
        let raw = self.api_get("/v6/product/categoryList", &[]).await?;
        Ok(Self::product_entity_page_response(&raw))
    }

    /// 读取品牌/分类下的系列与产品列表。
    ///
    /// 分类实体自带的 url/title/subTitle 必须原样传给统一页面列表接口；
    /// 直接使用 /product/productList?id=... 会丢失分类上下文，服务端可能返回默认的手机列表。
    /// 数据来源: GET /v6/page/dataList?url={url}&title={title}&subTitle={sub_title}
    pub async fn get_product_list(
        &self,
        url: &str,
        title: &str,
        sub_title: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
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
        let raw = self.api_get("/v6/page/dataList", &query).await?;
        Ok(Self::product_entity_page_response(&raw))
    }

    /// 读取品牌下的系列与产品列表，保持 APK 的 product/productList 调用链。
    /// 数据来源: GET /v6/product/productList?id={brand_id}&type={brand_type}
    pub async fn get_product_brand_products(
        &self,
        brand_id: &str,
        brand_type: &str,
        page: u32,
        first_item: &str,
        last_item: &str,
    ) -> Result<Value, String> {
        let mut query = vec![("id", brand_id.to_string()), ("type", brand_type.to_string()), ("page", page.max(1).to_string())];
        if !first_item.trim().is_empty() {
            query.push(("firstItem", first_item.to_string()));
        }
        if !last_item.trim().is_empty() {
            query.push(("lastItem", last_item.to_string()));
        }
        let raw = self.api_get("/v6/product/productList", &query).await?;
        Ok(Self::product_entity_page_response(&raw))
    }

    /// APK 闲置品牌列表。
    /// 数据来源: GET /v6/erShou/brandList
    pub async fn get_secondhand_brand_list(&self) -> Result<Value, String> {
        let raw = self.api_get("/v6/erShou/brandList", &[]).await?;
        Ok(Self::product_entity_page_response(&raw))
    }

    /// APK 闲置品牌下的型号/系列列表。
    /// 数据来源: GET /v6/erShou/productList?id={brand_id}&listType={list_type}
    pub async fn get_secondhand_product_list(&self, brand_id: &str, list_type: &str, page: u32, first_item: &str, last_item: &str) -> Result<Value, String> {
        let query = build_secondhand_product_list_query(brand_id, list_type, page, first_item, last_item);
        let raw = self.api_get("/v6/erShou/productList", &query).await?;
        Ok(Self::product_entity_page_response(&raw))
    }

    /// 产品媒体/图集列表（图片/视频）
    /// 数据来源: GET /v6/product/mediaList?id={id}&type={type}&is_recommend={is_recommend}
    pub async fn get_product_media_list(
        &self,
        product_id: &str,
        media_type: &str,
        is_recommend: i32,
        page: u32,
    ) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/product/mediaList",
                &[
                    ("id", product_id.to_string()),
                    ("type", media_type.to_string()),
                    ("is_recommend", is_recommend.to_string()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 修改产品心愿状态（想要，需登录）
    /// 数据来源: POST /v6/product/changeWishStatus
    pub async fn change_product_wish_status(&self, product_id: &str, status: i32) -> Result<Value, String> {
        wrap_api_data(
            self.api_post(
                "/v6/product/changeWishStatus",
                &[],
                &[
                    ("id", product_id.to_string()),
                    ("status", status.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 修改产品关注状态（需登录）。
    /// 数据来源: POST /v6/product/changeFollowStatus?id={id}&status={status}
    pub async fn change_product_follow_status(&self, product_id: &str, status: i32) -> Result<Value, String> {
        let product_id = product_id.trim();
        if product_id.is_empty() {
            return Err("产品 ID 不能为空".to_string());
        }
        wrap_api_data(
            self.api_post(
                "/v6/product/changeFollowStatus",
                &[],
                &[
                    ("id", product_id.to_string()),
                    ("status", status.to_string()),
                ],
            )
            .await?,
        )
    }

    /// 产品心愿（想要该产品）用户列表（需登录）
    /// 数据来源: POST /v6/product/wishList
    pub async fn get_product_wish_list(&self, product_id: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_post(
                "/v6/product/wishList",
                &[
                    ("id", product_id.to_string()),
                    ("page", page.to_string()),
                ],
                &[],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 产品已购用户列表（需登录）
    /// 数据来源: POST /v6/product/buyList
    pub async fn get_product_buy_list(&self, product_id: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_post(
                "/v6/product/buyList",
                &[
                    ("id", product_id.to_string()),
                    ("page", page.to_string()),
                ],
                &[],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 读取用户相关产品列表（我的数码：想要/已购/拥有）
    /// 数据来源: GET /v6/page/dataList?url=#/product/productList?type={type}
    pub async fn get_my_product_list(&self, _uid: &str, product_type: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/page/dataList",
                &[
                    (
                        "url",
                        format!("#/product/productList?type={product_type}"),
                    ),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 产品评分趋势图数据（日/周/月）
    /// 数据来源: GET /v6/product/ratingChart?id={id}
    pub async fn get_product_rating_chart(&self, product_id: &str) -> Result<Value, String> {
        wrap_api_data(
            self.api_get("/v6/product/ratingChart", &[("id", product_id.to_string())])
                .await?,
        )
    }

    /// 官方产品点评使用 feed/createFeed 的 rating + product_phone 表单。
    pub async fn create_product_rating(
        &self,
        product_id: &str,
        score: i32,
        message: &str,
        buy_status: bool,
    ) -> Result<Value, String> {
        if product_id.trim().is_empty() || !(1..=5).contains(&score) {
            return Err("请选择 1 至 5 星并确认产品".to_string());
        }
        self.submit_create_feed_form(
            build_product_review_form(product_id.trim(), score, message.trim(), buy_status),
            "发布产品点评失败：",
        )
        .await
    }

    /// 产品用户评分列表（NodeRating）。
    ///
    /// 官方客户端复用节点评分列表接口，列表的 targetType 为 `7`；
    /// 不带 ratingType 和 isOwner=0，和官方默认请求一致。
    /// 数据来源: GET /v6/page/dataList?url=/feed/nodeRatingList&targetType=7&targetId={id}
    pub async fn get_product_rating_list(
        &self,
        product_id: &str,
        star: i32,
        is_owner: i32,
        page: u32,
    ) -> Result<Value, String> {
        let query = build_product_rating_list_query(product_id, star, is_owner, page);
        let raw = self.api_get("/v6/page/dataList", &query).await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 应用评分用户列表
    /// 数据来源: GET /v6/apk/ratingUserList?id={id}
    pub async fn get_apk_rating_user_list(&self, apk_id: &str, page: u32) -> Result<Value, String> {
        let raw = self
            .api_get(
                "/v6/apk/ratingUserList",
                &[("id", apk_id.to_string()), ("page", page.to_string())],
            )
            .await?;
        Ok(json!({ "code": 200, "data": Self::extract_cleaned_list(&raw) }))
    }

    /// 提交/取消产品评分（需登录；value=0 表示取消评分）。
    ///
    /// 产品评分与动态评分使用不同接口。APK 使用 GET /v6/apk/rating，
    /// 仅传产品 ID 和评分值；动态评分接口会将产品评分误判为无权限操作。
    pub async fn change_rating_status(
        &self,
        product_id: &str,
        value: i32,
        _uid: &str,
        _buy_status: Option<i32>,
        _is_owner: Option<i32>,
    ) -> Result<Value, String> {
        let query = build_product_rating_query(product_id, value);
        wrap_api_data(self.api_get("/v6/apk/rating", &query).await?)
    }
}
