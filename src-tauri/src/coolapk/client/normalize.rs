use super::*;

impl CoolapkClient {
    pub(super) fn clean_single_feed(item: &Value, idx: usize) -> Option<Value> {
        let obj = item.as_object()?;

        let user_info = obj.get("userInfo").or_else(|| obj.get("user"));
        let username = obj
            .get("username")
            .and_then(|v| v.as_str())
            .or_else(|| {
                user_info
                    .and_then(|u| u.get("username"))
                    .and_then(|v| v.as_str())
            })
            .or_else(|| {
                user_info
                    .and_then(|u| u.get("name"))
                    .and_then(|v| v.as_str())
            })
            .or_else(|| obj.get("user_name").and_then(|v| v.as_str()));

        let uid = get_str_by_keys(obj, &["uid", "userId", "user_id", "authorUid", "author_uid"])
            .or_else(|| user_info.and_then(|u| u.as_object()).and_then(|info| get_str_by_keys(info, &["uid", "userId", "user_id", "authorUid", "author_uid", "id"])));

        let entity_type = obj
            .get("entityType")
            .or_else(|| obj.get("entity_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 过滤 Banner、Card 广告与结构占位卡 (如 "今日酷安" Banner 广告卡、搜索分组头)
        if entity_type == "card"
            || entity_type == "header"
            || entity_type == "card_title"
            || entity_type == "banner"
        {
            return None;
        }

        let is_news_type =
            entity_type == "dyh" || entity_type == "article" || entity_type == "news";

        let raw_username = match username {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => {
                let dyh = obj.get("dyh_name").and_then(|v| v.as_str());
                let author = obj.get("author").and_then(|v| v.as_str());
                let source = obj.get("source").and_then(|v| v.as_str());

                if let Some(name) = dyh.or(author).or(source) {
                    name.to_string()
                } else if is_news_type {
                    "酷安快讯".to_string()
                } else {
                    // 普通 Feed 贴文必须有真实发帖人 Username，禁止向推荐流注入盲目“酷安快讯”
                    return None;
                }
            }
        };

        let raw_uid = match uid {
            Some(u) if !u.is_empty() => u,
            _ => "0".to_string(),
        };

        let message = obj
            .get("message")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("description").and_then(|v| v.as_str()))
            .or_else(|| obj.get("subTitle").and_then(|v| v.as_str()))
            .unwrap_or("");

        let title = obj
            .get("title")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("entityTitle").and_then(|v| v.as_str()))
            .or_else(|| obj.get("message_title").and_then(|v| v.as_str()))
            .unwrap_or("");

        let has_pics = ["picArr", "imageUriList", "image_uri_list"]
            .iter()
            .any(|key| {
                obj.get(*key)
                    .and_then(|value| value.as_array())
                    .map_or(false, |array| !array.is_empty())
            });
        let single_pic = obj
            .get("pic")
            .and_then(|v| v.as_str())
            .map_or(false, |p| !p.is_empty());
        let has_video = has_any_non_empty_field(
            obj,
            &[
                "videoUrl",
                "video_url",
                "videoURL",
                "videoSrc",
                "video_src",
                "videoPic",
                "video_pic",
                "videoCover",
                "video_cover",
                "videoThumbnail",
                "video_thumbnail",
                "videoDuration",
                "video_duration",
                "mediaUrl",
                "media_url",
                "mediaURL",
                "mediaPic",
                "media_pic",
                "mediaInfo",
                "media_info",
                "mediaType",
                "media_type",
                "video",
                "videoInfo",
                "video_info",
                "media",
            ],
        );
        let has_relation = has_any_non_empty_field(
            obj,
            &[
                "targetRow",
                "target_row",
                "relationRows",
                "relation_rows",
                "extraRows",
                "extra_rows",
                "productRows",
                "product_rows",
                "forwardSourceFeed",
                "forward_source_feed",
            ],
        );
        let has_rating = has_any_non_empty_field(
            obj,
            &[
                "rating_score",
                "ratingScore",
                "rating_item_info",
                "ratingItemInfo",
                "rating_type",
                "ratingType",
                "filter_rating",
                "filterRating",
                "v4_rating_message",
                "v4RatingMessage",
                "comment_addition",
                "commentAddition",
                "comment_good",
                "commentGood",
                "comment_general",
                "commentGeneral",
                "comment_bad",
                "commentBad",
                "comment_good_pic",
                "commentGoodPic",
                "comment_general_pic",
                "commentGeneralPic",
                "comment_bad_pic",
                "commentBadPic",
            ],
        );

        if message.is_empty() && title.is_empty() && !has_pics && !single_pic && !has_video && !has_relation && !has_rating {
            return None;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let feed_id = obj
            .get("id")
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_u64().map(|n| n.to_string()))
            })
            .or_else(|| {
                obj.get("entityId").and_then(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v.as_u64().map(|n| n.to_string()))
                })
            })
            .unwrap_or_else(|| format!("feed_{}_{}", idx, timestamp));

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

        let mut pics = Vec::new();
        let raw_image_list = ["imageUriList", "image_uri_list", "picArr"]
            .iter()
            .filter_map(|key| obj.get(*key))
            .find(|value| {
                value
                    .as_array()
                    .map_or_else(|| has_non_empty_json_value(value), |array| !array.is_empty())
            });
        if let Some(arr) = raw_image_list.and_then(|value| value.as_array()) {
            for image in arr {
                if let Some(url) = image_source_url(image) {
                    pics.push(url);
                }
            }
        } else if let Some(url) = obj.get("pic").and_then(|value| image_source_url(value)) {
            pics.push(url);
        }

        let device_title = obj
            .get("device_title")
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("device_name").and_then(|v| v.as_str()))
            .unwrap_or("");

        let verify_title = user_info
            .and_then(|u| u.get("verify_title"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let is_top = get_u64_by_keys(obj, &["is_top", "isTop", "top"]);
        let likenum = get_u64_by_keys(obj, &["likenum", "like_num", "likeNum", "likenum_count"]);
        let replynum = get_u64_by_keys(
            obj,
            &[
                "replynum",
                "reply_num",
                "replyNum",
                "commentnum",
                "comment_num",
                "replynum_count",
            ],
        );
        let fav_num = get_u64_by_keys(obj, &["favnum", "fav_num", "favorite_num"]);
        let share_num = get_u64_by_keys(obj, &["sharenum", "share_num"]);
        // 官方不同列表会使用 readNum 或 viewnum；优先保留实际返回的非零浏览量。
        let hit_num = ["readNum", "readnum", "read_num", "viewnum", "viewNum", "view_num", "hitnum", "hit_num", "clicknum"]
            .iter().filter_map(|key| obj.get(*key).and_then(parse_u64_val)).find(|count| *count > 0).unwrap_or(0);
        let is_modified = get_u64_by_keys(obj, &["isModified", "is_modified"]);
        let change_count = get_u64_by_keys(obj, &["change_count", "changeCount"]);
        let last_change_time = get_u64_by_keys(obj, &["last_change_time", "lastChangeTime"]);

        let user_level =
            get_str_by_keys(obj, &["userLevel", "level", "user_level"]).unwrap_or_default();

        let target_type = first_value_by_keys(obj, &["targetRow", "target_row"])
            .and_then(|v| v.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let trace = obj
            .get("trace")
            .or_else(|| obj.get("extra_key"))
            .or_else(|| obj.get("extraKey"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let user_action = obj
            .get("userAction")
            .or_else(|| obj.get("user_action"))
            .cloned()
            .unwrap_or_else(|| json!({}));

        let mut cleaned = json!({
            "id": feed_id,
            "username": raw_username,
            "userAvatar": avatar,
            "userLevel": user_level,
            "verifyTitle": verify_title,
            "deviceTitle": device_title,
            "title": title,
            "message": message,
            "pics": pics,
            "infoHtml": obj.get("infoHtml").and_then(|v| v.as_str()).unwrap_or(""),
            "likenum": likenum,
            "replynum": replynum,
            "hitnum": hit_num,
            "readNum": hit_num,
            "favnum": fav_num,
            "sharenum": share_num,
            "isTop": is_top,
            "isModified": is_modified,
            "changeCount": change_count,
            "lastChangeTime": last_change_time,
            "userAction": user_action,
            "entityType": if entity_type.is_empty() { "feed" } else { entity_type },
            "trace": trace,
            "targetType": target_type,
            "uid": raw_uid,
            "dateline": get_u64_by_keys(obj, &["dateline", "create_time", "lastupdate", "createTime"])
        });

        // 保留官方不同上下文中的置顶字段，不能只读取 isTop。
        copy_first_field(&mut cleaned, obj, "isStickTop", &["isStickTop"]);
        copy_first_field(&mut cleaned, obj, "is_editor_title_top", &["is_editor_title_top"]);
        copy_first_field(&mut cleaned, obj, "collectionItem", &["collectionItem"]);
        copy_first_field(&mut cleaned, obj, "topInfo", &["topInfo"]);
        // APK 列表分页使用 Entity.entityId 生成 firstItem 和 lastItem，清洗时需要保留该游标字段。
        copy_first_field(&mut cleaned, obj, "entityId", &["entityId", "entity_id"]);
        copy_first_field(&mut cleaned, obj, "enableModify", &["enableModify", "enable_modify"]);
        // 官方 publish_status=1 表示仅自己可见；不能用审核/发布状态 status 代替。
        copy_first_field(&mut cleaned, obj, "publishStatus", &["publishStatus", "publish_status"]);

        // 列表接口会把关联标的和视频字段放在这些扩展字段中，必须在归一化时保留下来。
        copy_first_field(&mut cleaned, obj, "targetRow", &["targetRow", "target_row"]);
        copy_first_field(&mut cleaned, obj, "relationRows", &["relationRows", "relation_rows"]);
        copy_first_field(&mut cleaned, obj, "extraRows", &["extraRows", "extra_rows"]);
        copy_first_field(&mut cleaned, obj, "productRows", &["productRows", "product_rows"]);
        // 转发动态的原文由服务端放在 forwardSourceFeed，列表清洗时需要保留。
        copy_first_field(&mut cleaned, obj, "forwardSourceFeed", &["forwardSourceFeed", "forward_source_feed"]);
        copy_first_field(&mut cleaned, obj, "forwardSourceType", &["forwardSourceType", "forward_source_type"]);
        copy_first_field(&mut cleaned, obj, "forwardId", &["forwardid", "forwardId", "forward_id"]);
        // 回答动态的 fid 是所属问题 ID；首页清洗时必须保留，否则点击回答只能退化到 /feed/:id。
        copy_first_field(&mut cleaned, obj, "questionId", &["questionId", "question_id", "fid", "f_id"]);
        copy_first_field(&mut cleaned, obj, "answerId", &["answerId", "answer_id"]);
        copy_first_field(&mut cleaned, obj, "imageUriList", &["imageUriList", "image_uri_list"]);
        if cleaned.get("imageUriList").is_none() {
            if let Some(arr) = obj.get("picArr").and_then(|value| value.as_array()) {
                if arr.iter().any(|value| value.is_object()) {
                    if let Some(cleaned_obj) = cleaned.as_object_mut() {
                        cleaned_obj.insert("imageUriList".to_string(), Value::Array(arr.clone()));
                    }
                }
            }
        }
        copy_first_field(&mut cleaned, obj, "videoUrl", &["videoUrl", "video_url", "videoURL", "videoSrc", "video_src"]);
        copy_first_field(&mut cleaned, obj, "videoPic", &["videoPic", "video_pic", "videoCover", "video_cover", "videoThumbnail", "video_thumbnail"]);
        copy_first_field(&mut cleaned, obj, "videoDuration", &["videoDuration", "video_duration"]);
        copy_first_field(&mut cleaned, obj, "mediaUrl", &["mediaUrl", "media_url", "mediaURL"]);
        copy_first_field(&mut cleaned, obj, "mediaPic", &["mediaPic", "media_pic"]);
        copy_first_field(&mut cleaned, obj, "mediaInfo", &["mediaInfo", "media_info"]);
        copy_first_field(&mut cleaned, obj, "mediaType", &["mediaType", "media_type"]);
        copy_first_field(&mut cleaned, obj, "feedType", &["feedType", "feed_type", "type"]);
        copy_first_field(&mut cleaned, obj, "feedTypeName", &["feedTypeName", "feed_type_name"]);
        // 交易列表与详情使用同一商品卡片，不能在列表清洗时丢失价格、面议状态和外链。
        copy_first_field(&mut cleaned, obj, "ershou_info", &["ershou_info", "ershouInfo", "second_hand_info", "secondHandInfo"]);
        copy_first_field(&mut cleaned, obj, "type", &["type"]);
        copy_first_field(&mut cleaned, obj, "message_title", &["message_title", "messageTitle"]);
        copy_first_field(&mut cleaned, obj, "message_raw_output", &["message_raw_output", "messageRawOutput"]);
        // APK 点评卡依赖这些评分字段；列表归一化时保留原始结构，前端才能完整展示点评内容。
        copy_first_field(&mut cleaned, obj, "ratingScore", &["rating_score", "ratingScore"]);
        copy_first_field(&mut cleaned, obj, "ratingScore1", &["rating_score_1", "ratingScore1"]);
        copy_first_field(&mut cleaned, obj, "ratingScore2", &["rating_score_2", "ratingScore2"]);
        copy_first_field(&mut cleaned, obj, "ratingScore3", &["rating_score_3", "ratingScore3"]);
        copy_first_field(&mut cleaned, obj, "ratingScore4", &["rating_score_4", "ratingScore4"]);
        copy_first_field(&mut cleaned, obj, "ratingScore5", &["rating_score_5", "ratingScore5"]);
        copy_first_field(&mut cleaned, obj, "ratingScore6", &["rating_score_6", "ratingScore6"]);
        copy_first_field(&mut cleaned, obj, "ratingScore7", &["rating_score_7", "ratingScore7"]);
        copy_first_field(&mut cleaned, obj, "ratingScore8", &["rating_score_8", "ratingScore8"]);
        copy_first_field(&mut cleaned, obj, "ratingScore9", &["rating_score_9", "ratingScore9"]);
        copy_first_field(&mut cleaned, obj, "ratingScore10", &["rating_score_10", "ratingScore10"]);
        copy_first_field(&mut cleaned, obj, "ratingItemInfo", &["rating_item_info", "ratingItemInfo"]);
        copy_first_field(&mut cleaned, obj, "ratingType", &["rating_type", "ratingType"]);
        copy_first_field(&mut cleaned, obj, "filterRating", &["filter_rating", "filterRating"]);
        copy_first_field(&mut cleaned, obj, "v4RatingMessage", &["v4_rating_message", "v4RatingMessage"]);
        copy_first_field(&mut cleaned, obj, "v4HasRatingScore", &["v4_has_rating_score", "v4HasRatingScore"]);
        copy_first_field(&mut cleaned, obj, "v4RatingTotalCount", &["v4_rating_total_count", "v4RatingTotalCount"]);
        copy_first_field(&mut cleaned, obj, "v4RatingOwnerTotalCount", &["v4_rating_owner_total_count", "v4RatingOwnerTotalCount"]);
        copy_first_field(&mut cleaned, obj, "isOwner", &["is_owner", "isOwner"]);
        copy_first_field(&mut cleaned, obj, "showOwner", &["show_owner", "showOwner"]);
        copy_first_field(&mut cleaned, obj, "commentAddition", &["comment_addition", "commentAddition"]);
        copy_first_field(&mut cleaned, obj, "commentGood", &["comment_good", "commentGood"]);
        copy_first_field(&mut cleaned, obj, "commentGoodPic", &["comment_good_pic", "commentGoodPic"]);
        copy_first_field(&mut cleaned, obj, "commentGoodSource", &["comment_good_source", "commentGoodSource"]);
        copy_first_field(&mut cleaned, obj, "commentGeneral", &["comment_general", "commentGeneral"]);
        copy_first_field(&mut cleaned, obj, "commentGeneralPic", &["comment_general_pic", "commentGeneralPic"]);
        copy_first_field(&mut cleaned, obj, "commentGeneralSource", &["comment_general_source", "commentGeneralSource"]);
        copy_first_field(&mut cleaned, obj, "commentBad", &["comment_bad", "commentBad"]);
        copy_first_field(&mut cleaned, obj, "commentBadPic", &["comment_bad_pic", "commentBadPic"]);
        copy_first_field(&mut cleaned, obj, "commentBadSource", &["comment_bad_source", "commentBadSource"]);
        // 投票动态的可选项由 vote 对象提供；列表归一化时必须保留，前端才能渲染并提交投票。
        copy_first_field(&mut cleaned, obj, "vote", &["vote"]);
        copy_first_field(&mut cleaned, obj, "video", &["video"]);
        copy_first_field(&mut cleaned, obj, "videoInfo", &["videoInfo", "video_info"]);
        copy_first_field(&mut cleaned, obj, "media", &["media"]);
        // 用户评论列表会把所属原动态放在 feed 字段中，FeedCard 用它展示引用上下文。
        copy_first_field(&mut cleaned, obj, "feed", &["feed"]);
        // 收藏单内容需要保留 collection_item_info，移除条目接口使用其中的独立 item id。
        copy_first_field(&mut cleaned, obj, "collectionItem", &["collection_item_info", "collectionItemInfo", "collectionItem", "collection_item"]);
        copy_first_field(&mut cleaned, obj, "collectionItemId", &["collectionItemId", "collection_item_id", "itemId", "item_id"]);

        Some(cleaned)
    }

    pub(super) fn extract_cleaned_list(json_data: &Value) -> Vec<Value> {
        let mut cleaned_list = Vec::new();
        if let Some(data_arr) = json_data.get("data").and_then(|v| v.as_array()) {
            for (idx, item) in data_arr.iter().enumerate() {
                if let Some(single) = Self::clean_single_feed(item, idx) {
                    cleaned_list.push(single);
                }
                if let Some(entities) = item.get("entities").and_then(|v| v.as_array()) {
                    for (sub_idx, sub) in entities.iter().enumerate() {
                        if let Some(sub_single) = Self::clean_single_feed(sub, sub_idx) {
                            cleaned_list.push(sub_single);
                        }
                    }
                }
            }
        }
        cleaned_list
    }

    // 话题子栏目可能下发 Feed、评分 Feed、卡片或嵌套 entities，统一展开但保留未知实体。
    pub(super) fn append_topic_tab_row(value: &Value, index: usize, output: &mut Vec<Value>) {
        let Some(obj) = value.as_object() else {
            return;
        };
        let entity_type = obj
            .get("entityType")
            .or_else(|| obj.get("entity_type"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        let is_feed = matches!(entity_type.as_str(), "feed" | "feed_reply" | "feedreply" | "article" | "news")
            || entity_type.starts_with("feed_")
            || obj.contains_key("message")
            || obj.contains_key("username")
            || obj.contains_key("userInfo")
            || obj.contains_key("user_info");

        if !is_feed {
            if let Some(nested) = obj
                .get("feed")
                .or_else(|| obj.get("ratingFeed"))
                .or_else(|| obj.get("rating_feed"))
            {
                Self::append_topic_tab_row(nested, index, output);
                return;
            }
            if let Some(entities) = obj.get("entities").and_then(Value::as_array) {
                for (child_index, child) in entities.iter().enumerate() {
                    Self::append_topic_tab_row(child, child_index, output);
                }
                if !entities.is_empty() {
                    return;
                }
            }
        }

        if let Some(cleaned) = Self::clean_single_feed(value, index) {
            output.push(cleaned);
            return;
        }

        let has_display_fields = [
            "title",
            "description",
            "subTitle",
            "sub_title",
            "message",
            "pic",
            "logo",
            "url",
        ]
        .iter()
        .any(|key| obj.get(*key).is_some_and(|item| !item.is_null()));
        if has_display_fields {
            output.push(value.clone());
        }
    }

    pub(super) fn extract_topic_tab_list(json_data: &Value) -> Vec<Value> {
        let mut rows = Vec::new();
        if let Some(data_arr) = json_data.get("data").and_then(Value::as_array) {
            for (index, item) in data_arr.iter().enumerate() {
                Self::append_topic_tab_row(item, index, &mut rows);
            }
        } else if let Some(entities) = json_data
            .get("data")
            .and_then(|value| value.get("entities"))
            .and_then(Value::as_array)
        {
            for (index, item) in entities.iter().enumerate() {
                Self::append_topic_tab_row(item, index, &mut rows);
            }
        }
        rows
    }

    /// 提取 APK 话题页服务端下发的排序卡片，避免桌面端把排序项写死。
    pub(super) fn extract_topic_sort_options(json_data: &Value) -> Vec<Value> {
        let mut options = Vec::new();
        if let Some(data_arr) = json_data.get("data").and_then(|v| v.as_array()) {
            for item in data_arr {
                let template = item
                    .get("entityTemplate")
                    .or_else(|| item.get("entity_template"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let entity_type = item
                    .get("entityType")
                    .or_else(|| item.get("entity_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if template != "sortSelectCard" && entity_type != "sortSelectCard" {
                    continue;
                }
                if let Some(entities) = item.get("entities").and_then(|v| v.as_array()) {
                    options.extend(
                        entities
                            .iter()
                            .filter(|entity| entity.get("title").and_then(Value::as_str).is_some())
                            .cloned(),
                    );
                }
            }
        }
        options
    }

    /// 品牌/分类/产品实体提取：原样保留 id/title/logo 等原始字段。
    /// 不能用 clean_single_feed：品牌分类实体没有 username/author/dyh_name，
    /// 会被当作「无真实发帖人」的无效动态整条丢弃，导致「数码分类」左侧列表为空。
    pub(super) fn clean_product_entity(item: &Value) -> Option<Value> {
        let obj = item.as_object()?;
        let entity_type = obj.get("entityType").or_else(|| obj.get("entity_type")).and_then(|v| v.as_str()).unwrap_or("");
        let entity_template = obj.get("entityTemplate").or_else(|| obj.get("entity_template")).and_then(|v| v.as_str()).unwrap_or("");
        if ["card", "header", "card_title", "banner"].iter().any(|value| entity_type.eq_ignore_ascii_case(value)) {
            return None;
        }
        let is_product_structure = ["productgrouptitle", "productgroupmore", "series_title", "series_more", "series-title", "series-more"]
            .iter()
            .any(|template| entity_template.eq_ignore_ascii_case(template) || entity_type.eq_ignore_ascii_case(template));
        let has_id = obj
            .get("id")
            .map_or(false, |v| !v.is_null())
            || obj
                .get("entityId")
                .or_else(|| obj.get("entity_id"))
                .map_or(false, |v| !v.is_null())
            || obj
                .get("productId")
                .or_else(|| obj.get("product_id"))
                .map_or(false, |v| !v.is_null());
        if !has_id && !is_product_structure {
            return None;
        }
        Some(item.clone())
    }

    pub(super) fn product_entity_page_response(json_data: &Value) -> Value {
        let mut response = json!({ "code": 200, "data": Self::extract_product_entity_list(json_data) });
        if let Some(output) = response.as_object_mut() {
            for key in [
                "firstItem",
                "first_item",
                "lastItem",
                "last_item",
                "hasMore",
                "has_more",
                "total",
                "current",
                "pagination",
                "pageInfo",
                "page_info",
            ] {
                if let Some(value) = json_data.get(key) {
                    output.insert(key.to_string(), value.clone());
                }
            }
        }
        response
    }

    pub(super) fn extract_product_entity_list(json_data: &Value) -> Vec<Value> {
        let mut result = Vec::new();
        if let Some(data_arr) = json_data.get("data").and_then(|v| v.as_array()) {
            for item in data_arr.iter() {
                Self::append_product_entities(item, &mut result);
            }
        }
        result
    }

    pub(super) fn append_product_entities(item: &Value, result: &mut Vec<Value>) {
        if let Some(entities) = item.get("entities").and_then(|v| v.as_array()) {
            for sub in entities {
                Self::append_product_entities(sub, result);
            }
            return;
        }
        if let Some(cleaned) = Self::clean_product_entity(item) {
            result.push(cleaned);
        }
    }

    /// 用户浏览历史 / 最近访问专用提取：保留 history / recentHistory 实体原始结构，
    /// 仅统一 url（补全前导斜杠）与 logo（http -> https / 相对路径补全），供前端直接渲染跳转。
    /// 不能用 clean_single_feed，因为历史实体没有 username/userInfo，会被当作无效动态丢弃。
    pub(super) fn extract_history_list(json_data: &Value) -> Vec<Value> {
        let mut list = Vec::new();
        if let Some(data_arr) = json_data.get("data").and_then(|v| v.as_array()) {
            for item in data_arr.iter() {
                let Some(obj) = item.as_object() else {
                    continue;
                };

                let mut cleaned = obj.clone();

                if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                    let url = url.trim();
                    if !url.is_empty() && !url.starts_with('/') {
                        cleaned.insert("url".to_string(), json!(format!("/{url}")));
                    }
                }

                if let Some(logo) = obj.get("logo").and_then(|v| v.as_str()) {
                    let logo = logo.trim();
                    let normalized = if logo.starts_with("//") {
                        format!("https:{logo}")
                    } else if logo.starts_with("http://") {
                        logo.replacen("http://", "https://", 1)
                    } else if !logo.is_empty() && !logo.starts_with('/') {
                        format!("https://image.coolapk.com/{}", logo.trim_start_matches('/'))
                    } else {
                        logo.to_string()
                    };
                    cleaned.insert("logo".to_string(), json!(normalized));
                }

                list.push(Value::Object(cleaned));
            }
        }
        list
    }

    // 提取并清洗单个 APK/游戏 实体
    pub(super) fn clean_single_apk(item: &Value) -> Option<Value> {
        let obj = item.as_object()?;

        // 提取标题与包名，若两者皆无则非合规应用实体
        let title = get_str_by_keys(
            obj,
            &["title", "shorttitle", "apkname", "label", "entityTitle"],
        )?;
        let package_name = get_str_by_keys(obj, &["packageName", "apkname", "package_name", "id"])?;

        let raw_icon = get_str_by_keys(
            obj,
            &[
                "apkRomIcon",
                "logo",
                "icon",
                "pic",
                "cover",
                "apkIcon",
                "apkLogo",
                "appIcon",
                "bigIcon",
            ],
        )
        .unwrap_or_default();
        let icon = if raw_icon.starts_with("http") {
            raw_icon
        } else if raw_icon.starts_with("//") {
            format!("https:{}", raw_icon)
        } else if !raw_icon.is_empty() {
            format!(
                "https://image.coolapk.com/{}",
                raw_icon.trim_start_matches('/')
            )
        } else {
            String::new()
        };

        let sub_title = get_str_by_keys(
            obj,
            &["subTitle", "description", "target_row_title", "comment"],
        )
        .unwrap_or_default();
        let score =
            get_str_by_keys(obj, &["score", "star", "rating"]).unwrap_or_else(|| "9.0".to_string());
        let apk_size = get_str_by_keys(obj, &["apksize", "apkSizeFormatted", "size", "apk_size"])
            .unwrap_or_default();
        let down_num = get_str_by_keys(
            obj,
            &[
                "downCount",
                "downCountFormatted",
                "downnum",
                "download_count",
            ],
        )
        .unwrap_or_default();
        let category = get_str_by_keys(
            obj,
            &[
                "catName",
                "category_title",
                "category_name",
                "category",
                "tag",
                "apkTypeName",
            ],
        )
        .unwrap_or_else(|| "应用".to_string());
        let version = get_str_by_keys(
            obj,
            &["apkversionname", "apkVersionName", "version", "versionName"],
        )
        .unwrap_or_default();

        // 酷安 APK 实体 apktype 字段：1=应用，2=游戏
        let apk_type = get_u64_by_keys(obj, &["apktype", "apkType", "apk_type", "type"]);
        let title_lower = title.to_lowercase();
        let cat_lower = category.to_lowercase();

        // 明确的游戏类型判定
        let is_explicit_game = apk_type == 2
            || cat_lower.contains("游戏")
            || cat_lower.contains("手游")
            || cat_lower.contains("动作")
            || cat_lower.contains("射击")
            || cat_lower.contains("角色")
            || cat_lower.contains("策略")
            || cat_lower.contains("卡牌")
            || cat_lower.contains("赛车")
            || cat_lower.contains("竞技")
            || cat_lower.contains("二次元")
            || cat_lower.contains("模拟器");

        // 明确的辅助工具/盒子黑名单判定
        let is_utility_tool = title_lower.contains("游戏盒")
            || title_lower.contains("游戏大厅")
            || title_lower.contains("游戏交易")
            || title_lower.contains("游戏翻译")
            || title_lower.contains("游戏串")
            || title_lower.contains("游戏助手")
            || title_lower.contains("单反相机")
            || cat_lower.contains("相机");

        Some(json!({
            "id": obj.get("id").map(value_to_string).unwrap_or_else(|| package_name.clone()),
            "title": title,
            "packageName": package_name,
            "icon": icon,
            "apkRomIcon": icon,
            "logo": icon,
            "subTitle": sub_title,
            "description": sub_title,
            "score": score,
            "version": version,
            "apkSizeFormatted": apk_size,
            "downCountFormatted": down_num,
            "category": category,
            "isExplicitGame": is_explicit_game,
            "isUtilityTool": is_utility_tool,
            "entityType": obj.get("entityType").and_then(|v| v.as_str()).unwrap_or("apk")
        }))
    }

    // 从酷安响应 JSON 中解构合规 APK/Game 实体列表，支持指定类型模式 (game/app/all)
    pub(super) fn extract_apk_list(json_data: &Value, filter_mode: &str) -> Vec<Value> {
        let mut apk_list = Vec::new();
        let items = if let Some(arr) = json_data.get("data").and_then(|v| v.as_array()) {
            arr
        } else if let Some(arr) = json_data.as_array() {
            arr
        } else {
            return apk_list;
        };

        for item in items {
            if let Some(obj) = item.as_object() {
                if let Some(clean_apk) = Self::clean_single_apk(item) {
                    let is_explicit_game = clean_apk
                        .get("isExplicitGame")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let is_utility_tool = clean_apk
                        .get("isUtilityTool")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let should_keep = match filter_mode {
                        "game" => !is_utility_tool,
                        "app" => !is_explicit_game,
                        _ => true,
                    };
                    if should_keep {
                        apk_list.push(clean_apk);
                    }
                }
                if let Some(entities) = obj.get("entities").and_then(|v| v.as_array()) {
                    for entity in entities {
                        if let Some(clean_apk) = Self::clean_single_apk(entity) {
                            let is_explicit_game = clean_apk
                                .get("isExplicitGame")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let is_utility_tool = clean_apk
                                .get("isUtilityTool")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            let should_keep = match filter_mode {
                                "game" => !is_utility_tool,
                                "app" => !is_explicit_game,
                                _ => true,
                            };
                            if should_keep {
                                apk_list.push(clean_apk);
                            }
                        }
                    }
                }
            }
        }
        apk_list
    }
}
