use super::*;

pub(super) fn reply_target_params(feed_id: &str, rid: Option<&str>) -> (String, String) {
    rid.filter(|value| !value.trim().is_empty())
        .map(|reply_id| (reply_id.trim().to_string(), "reply".to_string()))
        .unwrap_or_else(|| (feed_id.trim().to_string(), "feed".to_string()))
}

pub(super) fn build_product_rating_query(product_id: &str, value: i32) -> Vec<(&'static str, String)> {
    vec![
        ("id", product_id.to_string()),
        ("value", value.to_string()),
    ]
}

pub(super) fn build_product_review_form(
    product_id: &str,
    score: i32,
    message: &str,
    buy_status: bool,
) -> Vec<(&'static str, String)> {
    let mut form = build_create_feed_form_for_type(message, None, None, "rating", "");
    for (key, value) in &mut form {
        match *key {
            "targetType" => *value = "product_phone".to_string(),
            "targetId" => *value = product_id.to_string(),
            _ => {}
        }
    }
    form.extend([
        ("rating_score_1", score.to_string()),
        ("buy_status", if buy_status { "1" } else { "0" }.to_string()),
        ("comment_good", String::new()),
        ("comment_general", String::new()),
        ("comment_bad", String::new()),
        ("comment_good_pic", String::new()),
        ("comment_general_pic", String::new()),
        ("comment_bad_pic", String::new()),
    ]);
    form
}

pub(super) fn build_product_rating_list_query(
    product_id: &str,
    star: i32,
    is_owner: i32,
    page: u32,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("url", "/feed/nodeRatingList".to_string()),
        ("targetType", "7".to_string()),
        ("targetId", product_id.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if is_owner == 1 {
        query.push(("isOwner", "1".to_string()));
    }
    if star > 0 {
        query.push(("star", star.to_string()));
    }
    query
}

// TopicUserRatingFragment / NodeRatingApi: targetType=3，默认不传筛选参数。
pub(super) fn build_topic_rating_list_query(
    topic_id: &str, page: u32, star: i32, rating_type: &str,
    detail_only: bool, first_item: &str, last_item: &str,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("url", "/feed/nodeRatingList".to_string()),
        ("targetType", "3".to_string()),
        ("targetId", topic_id.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if (1..=5).contains(&star) { query.push(("star", star.to_string())); }
    if matches!(rating_type, "0" | "1") { query.push(("ratingType", rating_type.to_string())); }
    if detail_only { query.push(("v4RatingItem", "1".to_string())); }
    if !first_item.is_empty() { query.push(("firstItem", first_item.to_string())); }
    if !last_item.is_empty() { query.push(("lastItem", last_item.to_string())); }
    query
}

pub(super) fn build_secondhand_product_list_query(
    brand_id: &str,
    list_type: &str,
    page: u32,
    first_item: &str,
    last_item: &str,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("id", brand_id.to_string()),
        ("listType", list_type.to_string()),
        ("page", page.max(1).to_string()),
    ];
    if !first_item.trim().is_empty() {
        query.push(("firstItem", first_item.to_string()));
    }
    if !last_item.trim().is_empty() {
        query.push(("lastItem", last_item.to_string()));
    }
    query
}

pub(super) fn build_collection_list_query(uid: &str, page: u32, first_item: &str, last_item: &str) -> Vec<(&'static str, String)> {
    vec![
        ("uid", uid.to_string()),
        ("showDefault", "1".to_string()),
        ("page", page.to_string()),
        ("firstItem", first_item.to_string()),
        ("lastItem", last_item.to_string()),
    ]
}

/// 应用收藏只选择系统默认收藏单；取消收藏移除全部已选收藏单，重复操作保持幂等。
pub(super) fn apk_collection_update_plan(collections: &[Value], favorited: bool) -> Result<Option<(String, String)>, String> {
    let flag = |value: Option<&Value>| value.is_some_and(|v| v.as_bool() == Some(true) || v.as_u64() == Some(1) || matches!(v.as_str(), Some("1" | "true")));
    let id = |row: &Value| row.as_object().and_then(|o| get_str_by_keys(o, &["id", "collectionId"]));
    let mut selected = Vec::new();
    for row in collections {
        if flag(row.get("isBeCollected").or_else(|| row.get("is_be_collected"))) {
            let value = id(row).ok_or("应用所在收藏单缺少有效 ID")?;
            if !selected.contains(&value) { selected.push(value); }
        }
    }
    if favorited {
        if !selected.is_empty() { return Ok(None); }
        let default = collections.iter().find(|row| flag(row.get("defaultCollected").or_else(|| row.get("default_collected")).or_else(|| row.get("isDefault"))));
        let collection_id = default.and_then(id).ok_or("未找到酷安默认收藏单，收藏失败")?;
        Ok(Some((collection_id, String::new())))
    } else if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some((String::new(), selected.join(","))))
    }
}

pub(super) fn build_product_feeds_query(
    product_id: &str,
    feed_type: &str,
    list_type: &str,
    page: u32,
) -> Vec<(&'static str, String)> {
    let mut query = vec![
        ("url", "/page?url=/product/feedList".to_string()),
        ("id", product_id.to_string()),
        ("type", feed_type.to_string()),
    ];
    if !list_type.trim().is_empty() {
        query.push(("listType", list_type.trim().to_string()));
    }
    query.push(("page", page.to_string()));
    query
}

pub(super) fn build_product_subtab_query(product_id: &str, sub_id: &str, page: u32) -> Vec<(&'static str, String)> {
    vec![
        ("url", "/page?url=/product/feedList".to_string()),
        ("cacheExpires", "60".to_string()),
        ("type", "subTabFeed".to_string()),
        ("withSortCard", "1".to_string()),
        ("withSubTabFeedCard", "1".to_string()),
        ("ignoreEntityById", "1".to_string()),
        ("id", product_id.to_string()),
        ("subId", sub_id.to_string()),
        ("page", page.max(1).to_string()),
    ]
}
