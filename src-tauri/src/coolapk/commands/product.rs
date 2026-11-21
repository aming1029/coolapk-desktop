use super::*;

#[tauri::command]
pub async fn get_product_detail(
    state: State<'_, AppState>,
    product_id: String,
) -> Result<Value, String> {
    state.client.get_product_detail(&product_id).await
}

#[tauri::command]
pub async fn get_product_feeds(
    state: State<'_, AppState>,
    product_id: String,
    feed_type: String,
    list_type: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_product_feeds(&product_id, &feed_type, &list_type, page)
        .await
}

#[tauri::command]
pub async fn get_product_config(
    state: State<'_, AppState>,
    config_id: String,
) -> Result<Value, String> {
    state.client.get_product_config(&config_id).await
}

#[tauri::command]
pub async fn add_config_compare(
    state: State<'_, AppState>,
    config_id: String,
) -> Result<Value, String> {
    state.client.add_config_compare(&config_id).await
}

#[tauri::command]
pub async fn remove_config_compare(
    state: State<'_, AppState>,
    config_id: String,
) -> Result<Value, String> {
    state.client.remove_config_compare(&config_id).await
}

#[tauri::command]
pub async fn get_product_brand_list(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_product_brand_list().await
}

#[tauri::command]
pub async fn get_product_category_list(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_product_category_list().await
}

#[tauri::command]
pub async fn get_product_list(
    state: State<'_, AppState>,
    url: String,
    title: String,
    sub_title: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_product_list(&url, &title, &sub_title, page, first_item.as_deref().unwrap_or(""), last_item.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn get_product_brand_products(
    state: State<'_, AppState>,
    brand_id: String,
    brand_type: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_product_brand_products(&brand_id, &brand_type, page, first_item.as_deref().unwrap_or(""), last_item.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn get_secondhand_brand_list(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_secondhand_brand_list().await
}

#[tauri::command]
pub async fn get_secondhand_product_list(
    state: State<'_, AppState>,
    brand_id: String,
    list_type: String,
    page: u32,
    first_item: Option<String>,
    last_item: Option<String>,
) -> Result<Value, String> {
    state
        .client
        .get_secondhand_product_list(&brand_id, &list_type, page, first_item.as_deref().unwrap_or(""), last_item.as_deref().unwrap_or(""))
        .await
}

#[tauri::command]
pub async fn get_product_media_list(
    state: State<'_, AppState>,
    product_id: String,
    media_type: String,
    is_recommend: i32,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_product_media_list(&product_id, &media_type, is_recommend, page)
        .await
}

#[tauri::command]
pub async fn change_product_wish_status(
    state: State<'_, AppState>,
    product_id: String,
    status: i32,
) -> Result<Value, String> {
    state
        .client
        .change_product_wish_status(&product_id, status)
        .await
}

#[tauri::command]
pub async fn change_product_follow_status(
    state: State<'_, AppState>,
    product_id: String,
    status: i32,
) -> Result<Value, String> {
    state
        .client
        .change_product_follow_status(&product_id, status)
        .await
}

#[tauri::command]
pub async fn get_product_wish_list(
    state: State<'_, AppState>,
    product_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_product_wish_list(&product_id, page).await
}

#[tauri::command]
pub async fn get_product_buy_list(
    state: State<'_, AppState>,
    product_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_product_buy_list(&product_id, page).await
}

#[tauri::command]
pub async fn get_my_product_list(
    state: State<'_, AppState>,
    uid: String,
    product_type: String,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_my_product_list(&uid, &product_type, page)
        .await
}

#[tauri::command]
pub async fn get_product_rating_chart(
    state: State<'_, AppState>,
    product_id: String,
) -> Result<Value, String> {
    state.client.get_product_rating_chart(&product_id).await
}

#[tauri::command]
pub async fn get_product_subtab_feeds(
    state: State<'_, AppState>,
    product_id: String,
    sub_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_product_subtab_feeds(&product_id, &sub_id, page).await
}

#[tauri::command]
pub async fn create_product_rating(
    state: State<'_, AppState>,
    product_id: String,
    score: i32,
    message: String,
    buy_status: bool,
) -> Result<Value, String> {
    state.client.create_product_rating(&product_id, score, &message, buy_status).await
}

#[tauri::command]
pub async fn get_product_rating_list(
    state: State<'_, AppState>,
    product_id: String,
    star: i32,
    is_owner: i32,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .get_product_rating_list(&product_id, star, is_owner, page)
        .await
}

#[tauri::command]
pub async fn get_apk_rating_user_list(
    state: State<'_, AppState>,
    apk_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_apk_rating_user_list(&apk_id, page).await
}

#[tauri::command]
pub async fn change_rating_status(
    state: State<'_, AppState>,
    product_id: String,
    value: i32,
    uid: String,
    buy_status: Option<i32>,
    is_owner: Option<i32>,
) -> Result<Value, String> {
    state
        .client
        .change_rating_status(&product_id, value, &uid, buy_status, is_owner)
        .await
}

#[tauri::command]
pub async fn get_user_product_albums(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_user_product_albums(&uid, page).await
}

#[tauri::command]
pub async fn get_goods_list_items(
    state: State<'_, AppState>,
    uid: String,
    goods_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_goods_list_items(&uid, &goods_id, page).await
}

#[tauri::command]
pub async fn create_product_album(
    state: State<'_, AppState>,
    title: String,
    description: String,
    album_type: u32,
    target_type: String,
    target_id: String,
    product_items: String,
) -> Result<Value, String> {
    state
        .client
        .create_product_album(
            &title,
            &description,
            album_type,
            &target_type,
            &target_id,
            &product_items,
        )
        .await
}

#[tauri::command]
pub async fn get_secondhand_feeds(state: State<'_, AppState>, page: u32) -> Result<Value, String> {
    state.client.get_secondhand_feeds(page).await
}

#[tauri::command]
pub async fn get_product_versions(state: State<'_, AppState>, product_id: String) -> Result<Value, String> {
    state.client.get_product_versions(&product_id).await
}

#[tauri::command]
pub async fn get_product_detail_by_name(
    state: State<'_, AppState>,
    name: String,
) -> Result<Value, String> {
    state.client.get_product_detail_by_name(&name).await
}

#[tauri::command]
pub async fn select_equipment_product(app: tauri::AppHandle, state: State<'_, AppState>, window_label: String, product_id: String) -> Result<(), String> {
    if !window_label.starts_with("browser_window_") || product_id.is_empty() || !product_id.chars().all(|c| c.is_ascii_digit()) {
        return Err("无效的装备产品选择请求".to_string());
    }
    let window = app.get_webview_window(&window_label).ok_or("装备编辑窗口已关闭")?;
    if !is_equipment_action_url(&window.url().map_err(|e| e.to_string())?, "editProductOwner") {
        return Err("装备编辑窗口已离开编辑页面".to_string());
    }
    let response = state.client.get_product_detail(&product_id).await?;
    let product = response.get("data").ok_or("服务端未返回产品详情")?;
    let script = equipment_product_callback(product)?;
    // 请求期间用户可能关闭或导航到其他页面，执行前再次核对目标。
    if !is_equipment_action_url(&window.url().map_err(|e| e.to_string())?, "editProductOwner") {
        return Err("装备编辑窗口已离开编辑页面".to_string());
    }
    window.eval(&script).map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

// === 用户 ===
#[tauri::command]
pub async fn get_user_rating_list(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_user_rating_list(&uid, page).await
}

// === 好物 / 购物生态 ===
#[tauri::command]
pub async fn get_goods_search_hot_words(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_goods_search_hot_words().await
}

#[tauri::command]
pub async fn search_goods(
    state: State<'_, AppState>,
    keyword: String,
    sort_name: String,
    sort: String,
    is_coupon: u32,
    page: u32,
) -> Result<Value, String> {
    state
        .client
        .search_goods(&keyword, &sort_name, &sort, is_coupon, page)
        .await
}

#[tauri::command]
pub async fn prepare_goods_by_url(state: State<'_, AppState>, url: String) -> Result<Value, String> {
    state.client.prepare_goods_by_url(&url).await
}

#[tauri::command]
pub async fn get_goods_detail(
    state: State<'_, AppState>,
    goods_id: String,
) -> Result<Value, String> {
    state.client.get_goods_detail(&goods_id).await
}

#[tauri::command]
pub async fn get_goods_list_types(state: State<'_, AppState>) -> Result<Value, String> {
    state.client.get_goods_list_types().await
}

#[tauri::command]
pub async fn get_goods_list(
    state: State<'_, AppState>,
    uid: String,
    goods_id: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_goods_list(&uid, &goods_id, page).await
}

#[tauri::command]
pub async fn get_goods_store_items(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_goods_store_items(&uid, page).await
}

#[tauri::command]
pub async fn get_product_albums(
    state: State<'_, AppState>,
    uid: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_product_albums(&uid, page).await
}

#[tauri::command]
pub async fn get_my_goods_feeds(
    state: State<'_, AppState>,
    uid: String,
    goods_type: String,
    page: u32,
) -> Result<Value, String> {
    state.client.get_my_goods_feeds(&uid, &goods_type, page).await
}

#[tauri::command]
pub async fn create_goods_list(
    state: State<'_, AppState>,
    title: String,
    message: String,
    cover: String,
    top_limit: u32,
    is_open_vote: u32,
    list_type: String,
    target_id: String,
    target_type: String,
) -> Result<Value, String> {
    state
        .client
        .create_goods_list(
            &title,
            &message,
            &cover,
            top_limit,
            is_open_vote,
            &list_type,
            &target_id,
            &target_type,
        )
        .await
}

#[tauri::command]
pub async fn edit_goods_list(
    state: State<'_, AppState>,
    id: String,
    title: String,
    message: String,
    cover: String,
    top_limit: u32,
    is_open_vote: u32,
    list_type: String,
) -> Result<Value, String> {
    state
        .client
        .edit_goods_list(&id, &title, &message, &cover, top_limit, is_open_vote, &list_type)
        .await
}

#[tauri::command]
pub async fn add_goods_to_goods_list(
    state: State<'_, AppState>,
    feed_id: String,
    goods_id: String,
    note: String,
    pic: String,
) -> Result<Value, String> {
    state
        .client
        .add_goods_to_goods_list(&feed_id, &goods_id, &note, &pic)
        .await
}

#[tauri::command]
pub async fn delete_goods_list_items(
    state: State<'_, AppState>,
    cancel_feed_id: String,
    goods_id: String,
) -> Result<Value, String> {
    state
        .client
        .delete_goods_list_items(&cancel_feed_id, &goods_id)
        .await
}

#[tauri::command]
pub async fn edit_goods_list_item(
    state: State<'_, AppState>,
    feed_id: String,
    goods_id: String,
    note: String,
    pic: String,
) -> Result<Value, String> {
    state
        .client
        .edit_goods_list_item(&feed_id, &goods_id, &note, &pic)
        .await
}

#[tauri::command]
pub async fn vote_goods_list_item(
    state: State<'_, AppState>,
    id: String,
    item_id: String,
    value: i32,
) -> Result<Value, String> {
    state.client.vote_goods_list_item(&id, &item_id, value).await
}

#[tauri::command]
pub async fn bind_feed_to_goods_list(
    state: State<'_, AppState>,
    feed_id: String,
    goods_list_id: String,
) -> Result<Value, String> {
    state
        .client
        .bind_feed_to_goods_list(&feed_id, &goods_list_id)
        .await
}
