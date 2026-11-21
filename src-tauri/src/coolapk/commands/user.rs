use super::*;

#[tauri::command]
pub async fn follow_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Value, String> {
    state.client.follow_collection(&collection_id).await
}

#[tauri::command]
pub async fn unfollow_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Value, String> {
    state.client.unfollow_collection(&collection_id).await
}

#[tauri::command]
pub async fn create_user_vote(
    state: State<'_, AppState>,
    feed_id: String,
    option_ids: Vec<String>,
    anonymous_status: bool,
) -> Result<Value, String> {
    state
        .client
        .create_user_vote(&feed_id, &option_ids, anonymous_status)
        .await
}

#[tauri::command]
pub async fn get_my_card_manager(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_my_card_manager().await
}

#[tauri::command]
pub async fn get_user_space(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.get_user_space(&uid).await
}

#[tauri::command]
pub async fn get_public_user_space(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.get_public_user_space(&uid).await
}

#[tauri::command]
pub async fn get_user_profile(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.get_user_profile(&uid).await
}

#[tauri::command]
pub async fn get_user_plugins(state: State<'_, AppState>, store: bool, page: u32, plugin_type: u8) -> Result<Value, String> {
    state.client.get_user_plugins(store, page, plugin_type).await
}

#[tauri::command]
pub async fn save_user_plugins(state: State<'_, AppState>, avatar_id: u64, feed_id: u64) -> Result<Value, String> {
    state.client.save_user_plugins(avatar_id, feed_id).await
}

#[tauri::command]
pub async fn claim_user_plugin(state: State<'_, AppState>, id: u64) -> Result<Value, String> {
    state.client.claim_user_plugin(id).await
}

#[tauri::command]
pub async fn get_my_profile(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.get_my_profile(&uid).await
}

#[tauri::command]
pub async fn get_public_user_profile(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.get_public_user_profile(&uid).await
}

#[tauri::command]
pub async fn get_user_remark_list(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.get_user_remark_list(&uid).await
}

#[tauri::command]
pub async fn change_avatar(
    state: State<'_, AppState>,
    image_bytes: Vec<u8>,
    file_name: String,
    content_type: String,
) -> Result<Value, String> {
    state
        .client
        .change_avatar(&image_bytes, &file_name, &content_type)
        .await
}

#[tauri::command]
pub async fn get_user_forum_follow_list(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_user_forum_follow_list(&uid, page).await
}

#[tauri::command]
pub async fn get_user_feeds(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
    feed_type: String,
) -> Result<Value, String> {
    state.client.get_user_feeds(&uid, page, &feed_type).await
}

#[tauri::command]
pub async fn get_user_like_list(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_user_like_list(&uid, page).await
}

#[tauri::command]
pub async fn get_user_tab_data(
    state: State<'_, AppState>,
    uid: String,
    tab: String,
    page: u32,
    first_item: String,
    last_item: String,
    rating_target: String,
) -> Result<Value, String> {
    state
        .client
        .get_user_tab_data(
            &uid,
            &tab,
            page,
            &first_item,
            &last_item,
            &rating_target,
        )
        .await
}

#[tauri::command]
pub async fn set_user_feed_top(state: State<'_, AppState>, uid: String, feed_id: String, top: bool) -> Result<Value, String> {
    state.client.set_user_feed_top(&uid, &feed_id, top).await
}

#[tauri::command]
pub async fn get_black_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_black_list(page).await
}

#[tauri::command]
pub async fn get_ignore_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_ignore_list(page).await
}

#[tauri::command]
pub async fn get_limit_list(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_limit_list(page).await
}

#[tauri::command]
pub async fn add_to_black_list(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.add_to_black_list(&uid).await
}

#[tauri::command]
pub async fn remove_from_black_list(
    state: State<'_, AppState>,
    uid: String,
) -> Result<Value, String> {
    state.client.remove_from_black_list(&uid).await
}

#[tauri::command]
pub async fn add_to_ignore_list(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.add_to_ignore_list(&uid).await
}

#[tauri::command]
pub async fn remove_from_ignore_list(
    state: State<'_, AppState>,
    uid: String,
) -> Result<Value, String> {
    state.client.remove_from_ignore_list(&uid).await
}

#[tauri::command]
pub async fn follow_user(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.follow_user(&uid).await
}

#[tauri::command]
pub async fn unfollow_user(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.unfollow_user(&uid).await
}

#[tauri::command]
pub async fn special_follow_user(
    state: State<'_, AppState>,
    uid: String,
    special: bool,
) -> Result<Value, String> {
    state.client.special_follow_user(&uid, special).await
}

#[tauri::command]
pub async fn cancel_follower(state: State<'_, AppState>, uid: String) -> Result<Value, String> {
    state.client.cancel_follower(&uid).await
}

#[tauri::command]
pub async fn get_following_feeds(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_following_feeds(page).await
}

#[tauri::command]
pub async fn get_follow_user_list(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
    category: Option<String>,
) -> Result<Value, String> {
    state.client.get_follow_user_list(&uid, page, category.as_deref()).await
}

#[tauri::command]
pub async fn get_fans_user_list(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_fans_user_list(&uid, page).await
}
