use super::*;

#[tauri::command]
pub async fn get_tab_config(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_tab_config().await
}

#[tauri::command]
pub async fn get_discovery_config(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_discovery_config().await
}

#[tauri::command]
pub async fn get_discovery_page_data(
    state: State<'_, AppState>,
    url: String,
    title: String,
    sub_title: String,
    page: u32,
    first_item: String,
    last_item: String,
    page_context: String,
    request_args_json: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_discovery_page_data(
            &url,
            &title,
            &sub_title,
            page,
            &first_item,
            &last_item,
            &page_context,
            request_args_json.as_deref().unwrap_or(""),
        )
        .await
}

#[tauri::command]
pub async fn get_load_config(state: State<'_, AppState>, refresh: Option<bool>) -> Result<Value, String> {
    state.client.get_my_profile_cards(refresh.unwrap_or(false)).await
}

#[tauri::command]
pub async fn get_home_tab_config(
    state: State<'_, AppState>,
    reset: bool,
) -> Result<Value, String> {
    state.client.get_home_tab_config(reset).await
}

#[tauri::command]
pub async fn get_editable_feed(state: State<'_, AppState>, feed_id: String) -> Result<Value, String> {
    state.client.get_editable_feed(&feed_id).await
}
