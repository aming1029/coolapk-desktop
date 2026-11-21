use super::*;

#[tauri::command]
pub async fn get_apk_feeds(
    state: State<'_, AppState>,
    package_name: String,
    sort_type: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_apk_feeds(&package_name, &sort_type, page)
        .await
}

#[tauri::command]
pub async fn get_cool_picture_rank(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_cool_picture_rank(page).await
}

#[tauri::command]
pub async fn get_app_detail(
    state: State<'_, AppState>,
    package_name: String,
) -> Result<Value, String> {
    state.client.get_app_detail(&package_name).await
}

#[tauri::command]
pub async fn get_apk_comments(
    state: State<'_, AppState>,
    app_id: String,
    list_type: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_apk_comments(&app_id, &list_type, page)
        .await
}

#[tauri::command]
pub async fn favorite_apk(
    state: State<'_, AppState>,
    package_name: String,
) -> Result<Value, String> {
    state.client.favorite_apk(&package_name).await
}

#[tauri::command]
pub async fn get_apk_collection_status(state: State<'_, AppState>, apk_id: String) -> Result<Value, String> {
    state.client.get_apk_collection_status(&apk_id).await
}

#[tauri::command]
pub async fn unfavorite_apk(
    state: State<'_, AppState>,
    package_name: String,
) -> Result<Value, String> {
    state.client.unfavorite_apk(&package_name).await
}

#[tauri::command]
pub async fn get_apk_url(
    state: State<'_, AppState>,
    package_name: String,
) -> Result<Value, String> {
    state.client.get_apk_url(&package_name).await
}

#[tauri::command]
pub async fn get_apk_qr(state: State<'_, AppState>, package_name: String) -> Result<Value, String> {
    state.client.get_apk_qr(&package_name).await
}

#[tauri::command]
pub async fn comment_apk(
    state: State<'_, AppState>,
    app_id: String,
    message: String,
) -> Result<Value, String> {
    state.client.comment_apk(&app_id, &message).await
}

#[tauri::command]
pub async fn get_game_list(
    state: State<'_, AppState>,
    page: u32,
    game_type: String,
) -> Result<Value, String> {
    state.client.get_game_list(page, &game_type).await
}

#[tauri::command]
pub async fn search_apks(
    state: State<'_, AppState>,
    query: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_apks(&query, page).await
}

#[tauri::command]
pub async fn search_games(
    state: State<'_, AppState>,
    query: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_games(&query, page).await
}

#[tauri::command]
pub async fn get_app_list(
    state: State<'_, AppState>,
    page: u32,
    cat: String,
) -> Result<Value, String> {
    state.client.get_app_list(page, &cat).await
}

// === 应用集 ===
#[tauri::command]
pub async fn get_album_list(
    state: State<'_, AppState>,
    list_type: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_album_list(&list_type, page).await
}

#[tauri::command]
pub async fn search_albums(
    state: State<'_, AppState>,
    query: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_albums(&query, page).await
}

#[tauri::command]
pub async fn get_album_detail(
    state: State<'_, AppState>,
    album_id: String,
) -> Result<Value, String> {
    state.client.get_album_detail(&album_id).await
}

#[tauri::command]
pub async fn get_user_album_list(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_user_album_list(&uid, page).await
}

#[tauri::command]
pub async fn create_album(
    state: State<'_, AppState>,
    title: String,
    intro: String,
    cover: String,
) -> Result<Value, String> {
    state.client.create_album(&title, &intro, &cover).await
}

#[tauri::command]
pub async fn edit_album(
    state: State<'_, AppState>,
    album_id: String,
    title: String,
    intro: String,
    cover: String,
) -> Result<Value, String> {
    state.client.edit_album(&album_id, &title, &intro, &cover).await
}

#[tauri::command]
pub async fn add_album_apk(
    state: State<'_, AppState>,
    album_id: String,
    package_name: String,
    title: String,
    url: String,
    note: String,
    display_order: i32,
    logo: String,
) -> Result<Value, String> {
    state
        .client
        .add_album_apk(&album_id, &package_name, &title, &url, &note, display_order, &logo)
        .await
}

#[tauri::command]
pub async fn delete_album_apk(
    state: State<'_, AppState>,
    album_id: String,
    package_name: String,
) -> Result<Value, String> {
    state.client.delete_album_apk(&album_id, &package_name).await
}

#[tauri::command]
pub async fn get_album_replies(
    state: State<'_, AppState>,
    album_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_album_replies(&album_id, page).await
}

// === 应用额外 ===
#[tauri::command]
pub async fn get_apk_discoverers(
    state: State<'_, AppState>,
    package_name: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_apk_discoverers(&package_name, page).await
}

#[tauri::command]
pub async fn get_apk_recommend_list(
    state: State<'_, AppState>,
    apk_type: String,
    title: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_apk_recommend_list(&apk_type, &title, page)
        .await
}

#[tauri::command]
pub async fn get_apk_related_apps(
    state: State<'_, AppState>,
    package_name: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_apk_related_apps(&package_name, page)
        .await
}

#[tauri::command]
pub async fn get_apk_gift_list(
    state: State<'_, AppState>,
    apk_id: Option<String>,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_apk_gift_list(apk_id.as_deref(), page)
        .await
}

// === 图片 ===
#[tauri::command]
pub async fn get_picture_list(
    state: State<'_, AppState>,
    tag: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_picture_list(&tag, page).await
}

// === 搜索 ===
#[tauri::command]
pub async fn search_apks_by_developer(
    state: State<'_, AppState>,
    developer: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .search_apks_by_developer(&developer, page)
        .await
}
