use super::*;

#[tauri::command]
pub async fn get_index_v8_feeds(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_index_v8_feeds(page).await
}

#[tauri::command]
pub async fn get_index_v8_feeds_paged(
    state: State<'_, AppState>,
    page: u32,
    first_item: String,
    last_item: String,
) -> Result<Value, String> {
    state
        .client
        .get_index_v8_feeds_paged(page, &first_item, &last_item)
        .await
}

#[tauri::command]
pub async fn get_index_v8_entities_paged(
    state: State<'_, AppState>,
    page: u32,
    first_item: String,
    last_item: String,
) -> Result<Value, String> {
    state
        .client
        .get_index_v8_entities_paged(page, &first_item, &last_item)
        .await
}

#[tauri::command]
pub async fn get_hot_feeds(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_hot_feeds(page).await
}

#[tauri::command]
pub async fn get_rank_feeds(
    state: State<'_, AppState>,
    rank_type: String,
    page: u32,
    background: Option<bool>,
) -> Result<Value, String> {
    state
        .client
        .get_rank_feeds_with_mode(&rank_type, page, background.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn get_latest_feeds(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_latest_feeds(page).await
}

#[tauri::command]
pub async fn get_digest_feeds(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_digest_feeds(page).await
}

#[tauri::command]
pub async fn get_board_feeds(
    state: State<'_, AppState>,
    board_tag: String,
    page: u32,
    background: Option<bool>,
) -> Result<Value, String> {
    state
        .client
        .get_board_feeds_with_mode(&board_tag, page, background.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn get_favorite_list(
    state: State<'_, AppState>,
    fav_type: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_favorite_list(
            &fav_type,
            page,
            first_item.as_deref().unwrap_or(""),
            last_item.as_deref().unwrap_or(""),
        )
        .await
}

#[tauri::command]
pub async fn get_feed_collection_status(
    state: State<'_, AppState>,
    feed_id: String,
) -> Result<Value, String> {
    state.client.get_feed_collection_status(&feed_id).await
}

#[tauri::command]
pub async fn get_collection_list(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state.client.get_collection_list(&uid, page, first_item.as_deref().unwrap_or(""), last_item.as_deref().unwrap_or("")).await
}

#[tauri::command]
pub async fn get_collection_item_list(
    state: State<'_, AppState>,
    collection_id: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_collection_item_list(
            &collection_id,
            page,
            first_item.as_deref().unwrap_or(""),
            last_item.as_deref().unwrap_or(""),
        )
        .await
}

#[tauri::command]
pub async fn get_collection_detail(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Value, String> {
    state.client.get_collection_detail(&collection_id).await
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    title: String,
    description: String,
    cover: String,
    is_open: i32,
    source_id: String,
) -> Result<Value, String> {
    state
        .client
        .create_collection(&title, &description, &cover, is_open, &source_id)
        .await
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    state.client.delete_collection(&id).await
}

#[tauri::command]
pub async fn remove_collection_item(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Value, String> {
    state.client.remove_collection_item(&item_id).await
}

#[tauri::command]
pub async fn clear_collection_invalid_items(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Value, String> {
    state
        .client
        .clear_collection_invalid_items(&collection_id)
        .await
}

#[tauri::command]
pub async fn like_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Value, String> {
    state.client.like_collection(&collection_id).await
}

#[tauri::command]
pub async fn unlike_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Value, String> {
    state.client.unlike_collection(&collection_id).await
}

#[tauri::command]
pub async fn get_feed_forward_list(
    state: State<'_, AppState>,
    feed_id: String,
    feed_type: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_feed_forward_list(&feed_id, &feed_type, page)
        .await
}

#[tauri::command]
pub async fn get_feed_like_list(
    state: State<'_, AppState>,
    feed_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_feed_like_list(&feed_id, page).await
}

#[tauri::command]
pub async fn get_feed_change_history(
    state: State<'_, AppState>,
    feed_id: String,
) -> Result<Value, String> {
    state.client.get_feed_change_history(&feed_id).await
}

#[tauri::command]
pub async fn get_vote_comments(
    state: State<'_, AppState>,
    feed_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_vote_comments(&feed_id, page).await
}

#[tauri::command]
pub async fn get_hit_history(
    state: State<'_, AppState>,
    page: u32,
    history_type: String,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state.client.get_hit_history(page, &history_type, first_item.as_deref(), last_item.as_deref()).await
}

#[tauri::command]
pub async fn get_recent_history(state: State<'_, AppState>, page: u32, first_item: Option<String>, last_item: Option<String>) -> Result<Value, String> {
    state.client.get_recent_history(page, first_item.as_deref(), last_item.as_deref()).await
}

#[tauri::command]
pub async fn get_spam_feed_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_spam_feed_list(page).await
}

#[tauri::command]
pub async fn get_feed_detail(state: State<'_, AppState>, feed_id: String, post_token: Option<String>, post_token_field: Option<String>) -> Result<Value, String> {
    state.client.get_feed_detail(&feed_id, post_token.as_deref(), post_token_field.as_deref()).await
}

#[tauri::command]
pub async fn get_public_feed_detail(state: State<'_, AppState>, feed_id: String) -> Result<Value, String> {
    state.client.get_public_feed_detail(&feed_id).await
}

#[tauri::command]
pub async fn get_reply_detail(
    state: State<'_, AppState>,
    reply_id: String,
) -> Result<Value, String> {
    state.client.get_reply_detail(&reply_id).await
}

#[tauri::command]
pub async fn get_feed_replies(
    state: State<'_, AppState>,
    feed_id: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
    list_type: Option<String>,
    from_feed_author: Option<u32>,
    post_token: Option<String>,
    post_token_field: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_feed_replies_with_captcha(
            &feed_id,
            page,
            first_item.as_deref().unwrap_or(""),
            last_item.as_deref().unwrap_or(""),
            list_type.as_deref().unwrap_or("lastupdate_desc"),
            from_feed_author.unwrap_or(0),
            post_token.as_deref(),
            post_token_field.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn get_hot_replies(
    state: State<'_, AppState>,
    feed_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_hot_replies(&feed_id, page).await
}

#[tauri::command]
pub async fn favorite_feed(state: State<'_, AppState>, feed_id: String) -> Result<Value, String> {
    state.client.favorite_feed(&feed_id).await
}

#[tauri::command]
pub async fn unfavorite_feed(state: State<'_, AppState>, feed_id: String) -> Result<Value, String> {
    state.client.unfavorite_feed(&feed_id).await
}

#[tauri::command]
pub async fn delete_feed(state: State<'_, AppState>, feed_id: String) -> Result<Value, String> {
    state.client.delete_feed(&feed_id).await
}

#[tauri::command]
pub async fn delete_reply(state: State<'_, AppState>, reply_id: String) -> Result<Value, String> {
    state.client.delete_reply(&reply_id).await
}

#[tauri::command]
pub async fn create_forward(
    state: State<'_, AppState>,
    feed_id: String,
    message: String,
    pic: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .create_forward(&feed_id, &message, pic.as_deref())
        .await
}

#[tauri::command]
pub async fn like_feed(state: State<'_, AppState>, feed_id: String) -> Result<Value, String> {
    state.client.like_feed(&feed_id).await
}

#[tauri::command]
pub async fn unlike_feed(state: State<'_, AppState>, feed_id: String) -> Result<Value, String> {
    state.client.unlike_feed(&feed_id).await
}

#[tauri::command]
pub async fn like_reply(state: State<'_, AppState>, reply_id: String) -> Result<Value, String> {
    state.client.like_reply(&reply_id).await
}

#[tauri::command]
pub async fn unlike_reply(state: State<'_, AppState>, reply_id: String) -> Result<Value, String> {
    state.client.unlike_reply(&reply_id).await
}

#[tauri::command]
pub async fn reply_feed(
    state: State<'_, AppState>,
    feed_id: String,
    message: String,
    rid: Option<String>,
    pic: Option<String>,
    post_token: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .reply_feed(
            &feed_id,
            &message,
            rid.as_deref(),
            pic.as_deref(),
            post_token.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn create_feed(
    state: State<'_, AppState>,
    message: String,
    pic: Option<String>,
    post_token: Option<String>,
    options: Option<crate::coolapk::client::PublishOptions>,
) -> Result<Value, String> {
    state
        .client
        .create_feed_with_options(&message, pic.as_deref(), post_token.as_deref(), options.as_ref())
        .await
}

#[tauri::command]
pub async fn submit_feed_report(state: State<'_, AppState>, id: String, report_type: String, reason: String, custom_reason: String, request_hash: String, pictures: Vec<String>) -> Result<Value, String> {
    state.client.submit_feed_report(&id, &report_type, &reason, &custom_reason, &request_hash, &pictures).await
}

// === 头条/编辑精选 ===
#[tauri::command]
pub async fn get_headline_feeds(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_headline_feeds(page).await
}

#[tauri::command]
pub async fn get_editor_choice_feeds(
    state: State<'_, AppState>,
    page: u32,
) -> Result<Value, String> {
    state.client.get_editor_choice_feeds(page).await
}
