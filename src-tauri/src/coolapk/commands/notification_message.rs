use super::*;

#[tauri::command]
pub async fn get_notification_count(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_notification_count().await
}

#[tauri::command]
pub async fn get_emoji_pictures(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_emoji_pictures().await
}

#[tauri::command]
pub async fn add_emoji_picture(state: State<'_, AppState>, url: String) -> Result<Value, String> {
    state.client.add_emoji_picture(&url).await
}

#[tauri::command]
pub async fn edit_emoji_pictures(state: State<'_, AppState>, action: String, ids: Vec<String>) -> Result<Value, String> {
    state.client.edit_emoji_pictures(&action, &ids).await
}

#[tauri::command]
pub async fn clear_notification_count(
    state: State<'_, AppState>,
    notification_type: String,
) -> Result<Value, String> {
    state.client.clear_notification_count(&notification_type).await
}

#[tauri::command]
pub async fn delete_all_notifications(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.delete_all_notifications().await
}

#[tauri::command]
pub async fn report_notification_click(state: State<'_, AppState>, task_id: String) -> Result<Value, String> {
    state.client.report_notification_click(&task_id).await
}

#[tauri::command]
pub async fn read_notification(state: State<'_, AppState>, id: String, notification_type: String) -> Result<Value, String> {
    state.client.read_notification(&id, &notification_type).await
}

#[tauri::command]
pub async fn get_notifications(
    state: State<'_, AppState>,
    notification_type: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
    note_type: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_notifications(&notification_type, page, first_item.as_deref().unwrap_or(""), last_item.as_deref().unwrap_or(""), note_type.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn list_messages(state: State<'_, AppState>, page: u32, first_item: String, last_item: String) -> Result<Value, String> {
    state.client.list_messages(page, &first_item, &last_item).await
}

#[tauri::command]
pub async fn get_recent_chat_users(
    state: State<'_, AppState>,
    page: u32,
) -> Result<Value, String> {
    state.client.get_recent_chat_users(page).await
}

#[tauri::command]
pub async fn list_chat_history(
    state: State<'_, AppState>,
    ukey: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .list_chat_history(&ukey, page, first_item.as_deref().unwrap_or(""), last_item.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn delete_message_chat(
    state: State<'_, AppState>,
    ukey: String,
) -> Result<Value, String> {
    state.client.delete_message_chat(&ukey).await
}

#[tauri::command]
pub async fn send_private_message(
    state: State<'_, AppState>,
    uid: String,
    message: String,
) -> Result<Value, String> {
    state.client.send_private_message(&uid, &message).await
}

#[tauri::command]
pub async fn read_message(state: State<'_, AppState>, ukey: String) -> Result<Value, String> {
    state.client.read_message(&ukey).await
}
