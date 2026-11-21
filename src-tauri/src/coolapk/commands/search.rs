use super::*;

#[tauri::command]
pub async fn get_search_suggestions(
    state: State<'_, AppState>,
    query: String,
) -> Result<Value, String> {
    state.client.get_search_suggestions(&query).await
}

#[tauri::command]
pub async fn search_users(
    state: State<'_, AppState>,
    query: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_users(&query, page).await
}

#[tauri::command]
pub async fn get_search_suggestions_app(
    state: State<'_, AppState>,
    query: String,
) -> Result<Value, String> {
    state.client.get_search_suggestions_app(&query).await
}

#[tauri::command]
pub async fn search_all(
    state: State<'_, AppState>,
    query: String,
    page: u32,
) -> Result<Value, String> {
    state.client.search_all(&query, page).await
}

#[tauri::command]
pub async fn search_by_type(
    state: State<'_, AppState>,
    search_type: String,
    query: String,
    page: u32,
    first_item: String,
    last_item: String,
    page_type: String,
    page_param: String,
    feed_type: String,
    sort: String,
    is_strict: u32,
    category: String,
    page_context: String,
) -> Result<Value, String> {
    state
        .client
        .search_by_type(
            &search_type,
            &query,
            page,
            &first_item,
            &last_item,
            &page_type,
            &page_param,
            &feed_type,
            &sort,
            is_strict,
            &category,
            &page_context,
        )
        .await
}

#[tauri::command]
pub async fn get_hot_searches(state: State<'_, AppState>, refresh: bool) -> Result<Value, String> {
    state.client.get_hot_searches(refresh).await
}

#[tauri::command]
pub async fn search_feeds(
    state: State<'_, AppState>,
    query: String,
    page: u32,
    sort_type: String,
) -> Result<Value, String> {
    state.client.search_feeds(&query, page, &sort_type).await
}
