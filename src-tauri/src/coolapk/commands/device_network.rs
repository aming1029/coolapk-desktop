use super::*;

#[tauri::command]
pub async fn get_device_feed_list(
    state: State<'_, AppState>,
    tag: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_device_feed_list(&tag, page, first_item.as_deref().unwrap_or(""), last_item.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub fn set_network_proxy(state: State<'_, AppState>, proxy_url: Option<String>) -> Result<(), String> {
    state.client.set_network_proxy(proxy_url)
}

#[tauri::command]
pub async fn test_network_proxy(proxy_url: String) -> Result<crate::coolapk::network_proxy::ProxyTestResult, String> {
    crate::coolapk::network_proxy::test_connection(proxy_url.trim()).await
}

#[tauri::command]
pub fn get_device_info(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_device_info()
}

#[tauri::command]
pub async fn initialize_device_identity(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.initialize_device_identity().await
}

#[tauri::command]
pub async fn generate_device_identity(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.generate_device_identity().await
}

#[tauri::command]
pub fn use_saved_device_identity(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.use_saved_device_identity()
}
