use super::*;

#[tauri::command]
pub async fn get_live_detail(state: State<'_, AppState>, live_id: String) -> Result<Value, String> {
    state.client.get_live_detail(&live_id).await
}

#[tauri::command]
pub async fn get_topic_detail_v7(state: State<'_, AppState>, tag: String) -> Result<Value, String> {
    state.client.get_topic_detail_v7(&tag).await
}

#[tauri::command]
pub async fn get_dyh_detail(state: State<'_, AppState>, dyh_id: String) -> Result<Value, String> {
    state.client.get_dyh_detail(&dyh_id).await
}

#[tauri::command]
pub async fn get_dyh_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_dyh_list(page).await
}

#[tauri::command]
pub async fn get_dyh_feeds(
    state: State<'_, AppState>,
    dyh_id: String,
    feed_type: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_dyh_feeds(&dyh_id, &feed_type, page).await
}

#[tauri::command]
pub async fn get_event_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_event_list(page).await
}

#[tauri::command]
pub async fn get_event_detail(
    state: State<'_, AppState>,
    event_id: String,
) -> Result<Value, String> {
    state.client.get_event_detail(&event_id).await
}

#[tauri::command]
pub async fn get_dyh_follow_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_dyh_follow_list(page).await
}

#[tauri::command]
pub async fn get_dyh_subscribe_list(
    state: State<'_, AppState>,
    page: u32,
) -> Result<Value, String> {
    state.client.get_dyh_subscribe_list(page).await
}

#[tauri::command]
pub async fn get_dyh_editor_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_dyh_editor_list(page).await
}

#[tauri::command]
pub async fn get_feed_share_dyh_list(state: State<'_, AppState>, share_type: u32, page: u32) -> Result<Value, String> {
    state.client.get_feed_share_dyh_list(share_type, page).await
}

#[tauri::command]
pub async fn share_feed_to_dyh(state: State<'_, AppState>, feed_id: String, dyh_ids: String, share_type: u32) -> Result<Value, String> {
    state.client.share_feed_to_dyh(&feed_id, &dyh_ids, share_type).await
}

#[tauri::command]
pub async fn get_node_feeds(
    state: State<'_, AppState>,
    node_type: String,
    node_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_node_feeds(&node_type, &node_id, page).await
}

#[tauri::command]
pub async fn get_hot_topics(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_hot_topics().await
}

#[tauri::command]
pub async fn follow_dyh(state: State<'_, AppState>, dyh_id: String) -> Result<Value, String> {
    state.client.follow_dyh(&dyh_id).await
}

#[tauri::command]
pub async fn unfollow_dyh(state: State<'_, AppState>, dyh_id: String) -> Result<Value, String> {
    state.client.unfollow_dyh(&dyh_id).await
}

#[tauri::command]
pub async fn follow_live(state: State<'_, AppState>, live_id: String) -> Result<Value, String> {
    state.client.follow_live(&live_id).await
}

#[tauri::command]
pub async fn unfollow_live(state: State<'_, AppState>, live_id: String) -> Result<Value, String> {
    state.client.unfollow_live(&live_id).await
}

#[tauri::command]
pub async fn search_tags(
    state: State<'_, AppState>,
    query: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_tags(&query, page).await
}

#[tauri::command]
pub async fn follow_tag(state: State<'_, AppState>, tag: String) -> Result<Value, String> {
    state.client.follow_tag(&tag).await
}

#[tauri::command]
pub async fn unfollow_tag(state: State<'_, AppState>, tag: String) -> Result<Value, String> {
    state.client.unfollow_tag(&tag).await
}

#[tauri::command]
pub async fn get_question_answers(
    state: State<'_, AppState>,
    feed_id: String,
    sort: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_question_answers(
            &feed_id,
            &sort,
            page,
            first_item.as_deref().unwrap_or(""),
            last_item.as_deref().unwrap_or(""),
        )
        .await
}

#[tauri::command]
pub async fn follow_question(state: State<'_, AppState>, question_id: String) -> Result<Value, String> {
    state.client.follow_question(&question_id).await
}

#[tauri::command]
pub async fn unfollow_question(state: State<'_, AppState>, question_id: String) -> Result<Value, String> {
    state.client.unfollow_question(&question_id).await
}

#[tauri::command]
pub async fn invite_question_answer(state: State<'_, AppState>, question_id: String, uid: String) -> Result<Value, String> {
    state.client.invite_question_answer(&question_id, &uid).await
}

#[tauri::command]
pub async fn get_followed_topics(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_followed_topics(page).await
}

#[tauri::command]
pub async fn search_feed_topics(
    state: State<'_, AppState>,
    query: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_feed_topics(&query, page).await
}

// 发帖搜索使用独立接口，避免改变发现页的话题搜索行为。
#[tauri::command]
pub async fn search_publish_topics(state: State<'_, AppState>, query: String, page: u32, recent_ids: String) -> Result<Value, String> {
    state.client.search_publish_topics(&query, page, &recent_ids).await
}

#[tauri::command]
pub async fn resolve_live_photo_video(
    state: State<'_, AppState>,
    image_url: String,
    content_id: String,
    content_type: String,
) -> Result<Value, String> {
    state
        .client
        .resolve_live_photo_video(&image_url, &content_id, &content_type)
        .await
}

#[tauri::command]
pub async fn get_live_photo_video_header(state: State<'_, AppState>, video_url: String) -> Result<String, String> {
    state.client.get_live_photo_video_header(&video_url).await
}

#[tauri::command]
pub async fn get_user_follow_nodes(
    state: State<'_, AppState>,
    uid: String,
) -> Result<Value, String> {
    state.client.get_user_follow_nodes(&uid).await
}

#[tauri::command]
pub async fn get_topic_detail(state: State<'_, AppState>, tag: String) -> Result<Value, String> {
    state.client.get_topic_detail(&tag).await
}

#[tauri::command]
pub async fn get_topic_rating_list(
    state: State<'_, AppState>, topic_id: String, page: u32, star: i32,
    rating_type: String, detail_only: bool, first_item: String, last_item: String,
) -> Result<Value, String> {
    state.client.get_topic_rating_list(&topic_id, page, star, &rating_type, detail_only, &first_item, &last_item).await
}

#[tauri::command]
pub async fn get_topic_rating_chart(state: State<'_, AppState>, topic_id: String) -> Result<Value, String> {
    state.client.get_topic_rating_chart(&topic_id).await
}

#[tauri::command]
pub async fn get_topic_feeds(
    state: State<'_, AppState>,
    tag: String,
    page: u32,
    list_type: String,
    first_item: String,
    last_item: String,
    block_status: i32,
) -> Result<Value, String> {
    state
        .client
        .get_topic_feeds(&tag, page, &list_type, &first_item, &last_item, block_status)
        .await
}

#[tauri::command]
pub async fn get_topic_tab_data(
    state: State<'_, AppState>,
    url: String,
    title: String,
    sub_title: String,
    page: u32,
    first_item: String,
    last_item: String,
    page_context: String,
) -> Result<Value, String> {
    state
        .client
        .get_topic_tab_data(&url, &title, &sub_title, page, &first_item, &last_item, &page_context)
        .await
}

#[tauri::command]
pub async fn get_topic_hub_data(
    state: State<'_, AppState>,
    sub_url: String,
    page: u32,
    first_item: String,
    last_item: String,
) -> Result<Value, String> {
    state
        .client
        .get_topic_hub_data(&sub_url, page, &first_item, &last_item)
        .await
}

#[tauri::command]
pub async fn create_answer(
    state: State<'_, AppState>,
    question_id: String,
    message: String,
    pic: Option<String>,
    post_token: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .create_answer(
            &question_id,
            &message,
            pic.as_deref(),
            post_token.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn search_apks_by_tag(
    state: State<'_, AppState>,
    tag: String,
    apk_type: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_apks_by_tag(&tag, &apk_type, page).await
}
