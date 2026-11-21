pub mod agent_bridge;
pub mod coolapk;
pub mod diagnostics;
mod performance_diagnostics;
pub mod download_manager;
mod windows_tile;
mod system_theme;
mod portable_update;
mod window_state;
pub use portable_update::try_run_portable_update_helper;
use window_state::*;
mod system_commands;
pub use system_commands::*;

use coolapk::client::CoolapkClient;
use coolapk::commands::{get_user_plugins, save_user_plugins, claim_user_plugin};
use coolapk::commands::{
    AppState, add_config_compare, add_goods_to_goods_list, add_to_black_list, add_to_ignore_list,
    bind_feed_to_goods_list, change_product_follow_status, change_product_wish_status, change_rating_status, create_product_rating, check_login_info,
    check_login_status, clean_expired_cache, clear_app_cache, clear_user_cookie,
    close_login_window, create_answer, create_export_directory, create_feed, create_forward, create_goods_list, create_product_album,
    set_user_feed_top, delete_feed, delete_goods_list_items, delete_reply, download_update, start_apk_download,
    pause_apk_download, cancel_apk_download, delete_apk_download_file, open_apk_download_directory, install_apk_download,
    edit_goods_list,
    edit_goods_list_item, export_json_file, favorite_apk, favorite_feed, fetch_external_page,
    follow_collection, follow_dyh, follow_live, follow_tag, follow_user, get_album_detail, get_album_list,
    create_album, edit_album, add_album_apk, delete_album_apk,
    get_album_replies, get_apk_discoverers, get_apk_feeds, get_apk_gift_list, get_apk_qr,
    get_apk_rating_user_list, get_apk_recommend_list, get_apk_url, get_apk_related_apps,
    get_app_detail, get_apk_comments, get_app_list, get_black_list, get_board_feeds, get_cache_info,
    get_collection_detail, get_collection_item_list, get_collection_list, create_collection,
    update_collection, delete_collection, remove_collection_item, clear_collection_invalid_items,
    cleanup_update_packages, is_update_package_available, take_update_install_error,
    get_feed_collection_status, get_apk_collection_status, get_cool_picture_rank, get_download_directory,
    get_device_feed_list, get_device_info, get_digest_feeds, get_download_version_list,
    get_discovery_config, get_discovery_page_data, get_live_detail,
    get_dyh_detail, get_dyh_feeds, get_dyh_list, get_dyh_follow_list, get_dyh_subscribe_list,
    get_feed_share_dyh_list, share_feed_to_dyh, get_dyh_editor_list, get_editor_choice_feeds, get_event_detail, get_event_list, get_fans_user_list,
    get_favorite_list, get_feed_change_history, get_feed_detail, get_public_feed_detail, get_editable_feed, update_feed, get_feed_forward_list,
    get_feed_like_list, get_feed_replies, get_follow_user_list, get_following_feeds, get_game_list,
    get_goods_detail, get_goods_list, get_goods_list_types, get_goods_search_hot_words,
    get_goods_store_items, get_headline_feeds, get_hit_history, get_hot_feeds, get_hot_replies,
    get_spam_feed_list, get_hidden_replies, get_followed_topics,
    get_hot_topics,
    get_ignore_list, get_image_data_url, get_index_v8_entities_paged, get_index_v8_feeds, get_index_v8_feeds_paged, get_latest_feeds, get_limit_list, get_live_photo_video_header,
    get_goods_list_items, get_home_tab_config, get_load_config, get_my_goods_feeds,
    get_my_product_list, get_node_feeds, get_notification_count, get_emoji_pictures, clear_notification_count, delete_all_notifications, get_notifications, get_picture_list,
    get_product_albums, get_product_brand_list, get_product_category_list, get_product_config,
    get_product_detail, get_product_detail_by_name, get_product_feeds, get_product_list, get_product_brand_products, get_secondhand_brand_list, get_secondhand_product_list,
    get_product_media_list, get_product_rating_chart, get_product_rating_list, get_product_subtab_feeds, get_question_answers,
    follow_question, unfollow_question, invite_question_answer,
    get_user_product_albums,
    get_hot_searches, get_rank_feeds, get_recent_history, get_reply_detail, get_search_suggestions,
    resolve_live_photo_video, resolve_video_url,
    get_search_suggestions_app, get_secondhand_feeds, get_sub_replies, get_tab_config,
    update_home_tab_config, update_user_profile, update_user_cover, change_avatar,
    get_topic_detail, get_topic_detail_v7, get_topic_feeds, get_topic_tab_data, get_topic_hub_data, get_topic_rating_list, get_topic_rating_chart, get_update_list,
    get_user_cookie, get_user_feeds, get_user_follow_nodes, get_user_forum_follow_list, get_user_like_list, get_user_album_list, get_user_profile, get_user_rating_list,
    get_user_qr_image, get_user_space, get_public_user_space, get_public_user_profile, get_user_tab_data, get_user_remark_list, get_vote_comments, create_user_vote, get_update_distribution, install_update, like_collection, like_feed, like_reply, list_accounts,
    list_chat_history, delete_message_chat, list_messages, get_recent_chat_users, login_as, login_by_account, login_by_mobile,
    open_cache_directory, open_image_in_system_viewer, open_login_webview, open_url, persist_current_account, quit_app, sync_login_webview,
    read_message, remove_account, remove_from_black_list, remove_from_ignore_list, reply_feed,
    comment_apk,
    cancel_follower, special_follow_user, update_user_remark,
    save_account, save_cookie_securely, save_file, save_image, save_image_data_url, search_albums, search_all, search_apks, search_by_type,
    search_apks_by_developer, search_apks_by_tag, search_feed_topics, search_feeds, search_games,
    search_goods, search_tags, search_users, send_private_image, send_private_message, send_sms_vcode,
    remove_config_compare, get_product_wish_list, get_product_buy_list,
    unfavorite_apk, unfavorite_feed, unfollow_collection, unfollow_dyh, unfollow_live, unfollow_tag,
    update_collection_item,
    unfollow_user, unlike_collection, unlike_feed, unlike_reply, update_device_profile, upload_image,
    vote_goods_list_item,
};
use download_manager::DownloadManager;
use diagnostics::{get_diagnostic_logs, clear_diagnostic_logs, get_diagnostic_verbose, set_diagnostic_verbose};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::Manager;
#[cfg(desktop)]
use tauri::WindowEvent;



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    #[cfg(desktop)]
    let context = {
        let mut context = context;
        // 桌面主窗口先隐藏，恢复记忆尺寸和位置后再显示，避免默认尺寸闪现。
        if let Some(window) = context.config_mut().app.windows.iter_mut().find(|window| window.label == "main") {
            window.visible = false;
        }
        context
    };
    diagnostics::install_panic_hook();
    let client = CoolapkClient::new();
    let state = AppState {
        client,
        downloads: DownloadManager::new(),
        login_session: std::sync::Mutex::new(None),
    };

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new()
            .format(|out, message, record| {
                out.finish(format_args!("[{}][{}][{}] {}", chrono::Utc::now().to_rfc3339(), record.level(), record.target(), message))
            })
            .clear_targets()
            .target(tauri_plugin_log::Target::new(
                tauri_plugin_log::TargetKind::LogDir { file_name: Some("coolapk-diagnostics".to_string()) },
            ))
            .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout))
            .level(log::LevelFilter::Debug)
            .filter(|metadata| {
                (metadata.target().starts_with("coolapk_desktop_lib") || metadata.target().starts_with("webview") || metadata.target() == "android.foreground")
                    && (metadata.level() <= log::Level::Info || diagnostics::verbose_enabled())
            })
            .max_file_size(2_000_000)
            .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(4))
            .build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build());

    #[cfg(desktop)]
    let builder = builder
        // 单实例插件必须先注册，才能把外部 deep link 转发到已运行的实例。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 重复启动时聚焦已有实例的主窗口；深链事件由插件转发给前端。
            show_main_window(app);
        }))
        // 解锁 WKWebView 对 rAF 的半刷新率钳制（144Hz 屏 72→144），失败时仅告警降级
        .plugin(tauri_plugin_macos_fps::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ));

    builder
        .register_asynchronous_uri_scheme_protocol("coolapk-video", |ctx, request, responder| {
            let target_url = reqwest::Url::parse(&request.uri().to_string())
                .ok()
                .and_then(|url| {
                    url.query_pairs()
                        .find(|(key, _)| key == "url")
                        .map(|(_, value)| value.into_owned())
                });
            let range = request
                .headers()
                .get("range")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let app_handle = ctx.app_handle().clone();

            tauri::async_runtime::spawn(async move {
                let Some(target_url) = target_url else {
                    responder.respond(
                        tauri::http::Response::builder()
                            .status(400)
                            .header("Content-Type", "text/plain; charset=utf-8")
                            .body(b"missing video url".to_vec())
                            .unwrap(),
                    );
                    return;
                };

                let result = app_handle
                    .state::<AppState>()
                    .client
                    .proxy_weibo_video(&target_url, range.as_deref())
                    .await;
                match result {
                    Ok(video) => {
                        let mut builder = tauri::http::Response::builder()
                            .status(video.status)
                            .header("Content-Type", video.content_type)
                            .header("Accept-Ranges", "bytes")
                            .header("Access-Control-Allow-Origin", "*");
                        if let Some(content_length) = video.content_length {
                            builder = builder.header("Content-Length", content_length.to_string());
                        }
                        if let Some(content_range) = video.content_range {
                            builder = builder.header("Content-Range", content_range);
                        }
                        responder.respond(builder.body(video.body).unwrap());
                    }
                    Err(error) => {
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(502)
                                .header("Content-Type", "text/plain; charset=utf-8")
                                .body(error.into_bytes())
                                .unwrap(),
                        );
                    }
                }
            });
        })
        // 兜底防线：主窗口若被导航到本地源以外的页面（如某个 v-html 遗漏的裸链接），
        // 加载完成后立即返回上一页，避免外部页面驻留在主窗口。
        // 点击路径已由前端全局 <a> 拦截 + 页面级 handleAnchorClick 处理。
        .on_page_load(|window, payload| {
            if window.label() == "main" && !is_main_window_navigation_allowed(payload.url()) {
                let _ = window.eval("history.go(-1)");
            }
        })
        .manage(state)
        .manage(agent_bridge::AgentBridgeRuntime::default())
        .setup(|app| {
            diagnostics::begin_session(app.handle());
            #[cfg(target_os = "linux")]
            system_theme::monitor(app.handle().clone());
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                // 启动时同步当前程序的协议注册，确保正式程序和本地桌面版本都能被冷启动唤起。
                app.deep_link().register_all()?;
            }

            let handle = app.handle().clone();
            let runtime = app.state::<agent_bridge::AgentBridgeRuntime>().inner().clone();
            tauri::async_runtime::spawn(async move {
                // 仅调试构建允许用环境变量覆盖保存的偏好，便于本地联调。
                #[cfg(debug_assertions)]
                if std::env::var("COOLAPK_AGENT_BRIDGE_AUTOSTART").is_ok() {
                    // 调试联调开关：SCOPES 逗号分隔、WRITE 取 disabled/dry-run/confirm/auto。
                    let scopes = std::env::var("COOLAPK_AGENT_BRIDGE_SCOPES")
                        .map(|raw| {
                            let list: Vec<String> = raw
                                .split(',')
                                .map(str::trim)
                                .filter(|item| !item.is_empty())
                                .map(str::to_string)
                                .collect();
                            if list.is_empty() { vec!["read".to_string()] } else { list }
                        })
                        .unwrap_or_else(|_| vec!["read".to_string()]);
                    let options = agent_bridge::StartOptions {
                        scopes,
                        transport: Some("tcp".to_string()),
                        write_mode: std::env::var("COOLAPK_AGENT_BRIDGE_WRITE").ok(),
                        allowed_accounts: Vec::new(),
                        // 调试构建默认同时开启 MCP；设 COOLAPK_AGENT_BRIDGE_MCP=0 可关闭。
                        mcp: std::env::var("COOLAPK_AGENT_BRIDGE_MCP").map(|value| value != "0").unwrap_or(true),
                        token: None,
                    };
                    match agent_bridge::start(handle.clone(), &runtime, options).await {
                        Ok(value) => log::info!("agent_bridge.autostart endpoint={}", value["data"]["endpoint"].as_str().unwrap_or_default()),
                        Err(error) => log::error!("agent_bridge.autostart_failed {error}"),
                    }
                    return;
                }
                match agent_bridge::restore_preferences(handle.clone(), &runtime).await {
                    Ok(Some(value)) => log::info!("agent_bridge.restored endpoint={}", value["data"]["endpoint"].as_str().unwrap_or_default()),
                    Ok(None) => {},
                    Err(error) => log::error!("agent_bridge.restore_failed {error}"),
                }
            });

            #[cfg(windows)]
            if let Err(error) = register_windows_notification_identity(app.app_handle()) {
                eprintln!("注册酷安通知身份失败：{error}");
            }

            #[cfg(desktop)]
            {
            let mut restore_maximized = false;
            // 读取启动参数并应用：窗口置顶 / 记忆上次窗口大小位置 / 静默启动到托盘
            if let Some(path) = startup_state_path(app.app_handle()) {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(flags) = serde_json::from_str::<serde_json::Value>(&raw) {
                        let w = app.get_webview_window("main");
                        if let Some(w) = w {
                            if flags["always_on_top"].as_bool().unwrap_or(false) {
                                ALWAYS_ON_TOP.store(true, Ordering::SeqCst);
                                let _ = w.set_always_on_top(true);
                            }
                            let remember_window_state =
                                flags["remember_window_state"].as_bool().unwrap_or(true);
                            REMEMBER_WINDOW_STATE.store(remember_window_state, Ordering::SeqCst);
                            if remember_window_state {
                                // 旧版记录不含此字段时继续按普通窗口恢复。
                                restore_maximized = flags["maximized"].as_bool().unwrap_or(false);
                                if let Some(saved_state) = parse_saved_window_state(&flags) {
                                    let screens: Vec<ScreenRect> = w
                                        .available_monitors()
                                        .unwrap_or_default()
                                        .into_iter()
                                        .map(|monitor| ScreenRect {
                                            x: monitor.position().x,
                                            y: monitor.position().y,
                                            w: monitor.size().width,
                                            h: monitor.size().height,
                                        })
                                        .collect();
                                    let primary = w.primary_monitor().ok().flatten().map(|monitor| {
                                        ScreenRect {
                                            x: monitor.position().x,
                                            y: monitor.position().y,
                                            w: monitor.size().width,
                                            h: monitor.size().height,
                                        }
                                    });
                                    let state = restore_window_rect(saved_state, &screens, primary);
                                    // set_size 使用内容区尺寸，因此保存时也必须读取 inner_size。
                                    let _ = w.set_size(tauri::PhysicalSize::new(state.w, state.h));
                                    // 先调整尺寸再恢复位置，避免窗口管理器按旧尺寸裁剪坐标。
                                    let _ = w.set_position(tauri::PhysicalPosition::new(state.x, state.y));
                                    if let Ok(mut guard) = WINDOW_STATE.lock() {
                                        *guard = Some(state);
                                    }
                                }
                            }
                            if flags["start_minimized"].as_bool().unwrap_or(false) {
                                START_MINIMIZED.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                }
            }

            // 即使启动时没有发生移动/缩放事件，也缓存一次真实几何信息，
            // 供关闭或 app.exit() 时的持久化兜底使用。
            if let Some(w) = app.get_webview_window("main") {
                if let (Ok(pos), Ok(size)) = (w.outer_position(), w.inner_size()) {
                    if size.width >= MIN_WINDOW_W && size.height >= MIN_WINDOW_H {
                        if let Ok(mut guard) = WINDOW_STATE.lock() {
                            *guard = Some(WindowState {
                                x: pos.x,
                                y: pos.y,
                                w: size.width,
                                h: size.height,
                            });
                        }
                    }
                }
                // 先保存普通窗口尺寸，再在首次显示前最大化，确保点击还原能恢复普通尺寸。
                WINDOW_MAXIMIZED.store(restore_maximized, Ordering::SeqCst);
                if restore_maximized { w.maximize()?; }
                // 完成窗口状态恢复后才首次显示；静默启动时继续隐藏到托盘。
                if !START_MINIMIZED.load(Ordering::SeqCst) { w.show()?; }
            }

            }

            // 将登录凭据持久化到应用数据目录，重启后自动恢复登录态
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&dir);
                let state = app.state::<AppState>();
                state
                    .client
                    .persist_cookie_to(dir.join("session_cookie.txt"));
            }

            #[cfg(desktop)]
            {
            // 系统托盘图标：常驻后台、快捷恢复窗口与退出
            // Linux 托盘库由运行环境动态加载；缺失时仍要让主窗口正常启动。
            #[cfg(target_os = "linux")]
            let tray_available = ["libayatana-appindicator3.so.1", "libappindicator3.so.1", "libayatana-appindicator3.so", "libappindicator3.so"].iter().any(|name| unsafe { libloading::Library::new(*name).is_ok() });
            #[cfg(not(target_os = "linux"))]
            let tray_available = true;
            if !tray_available { log::warn!("runtime.tray_unavailable: 未找到可加载的 AppIndicator 库，跳过托盘初始化"); }
            if let Some(icon) = app.default_window_icon().cloned().filter(|_| tray_available) {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;

                let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show, &quit])?;

                let tray_builder = TrayIconBuilder::with_id("main-tray")
                    .icon(icon)
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            show_main_window(app);
                        }
                        "quit" => {
                            // 退出前持久化窗口几何信息（托盘退出不触发 CloseRequested）
                            persist_current_window_geometry(app);
                            app.exit(0)
                        }
                        _ => {}
                    });

                #[cfg(any(target_os = "windows", target_os = "macos"))]
                let tray_builder = tray_builder
                    .show_menu_on_left_click(false)
                    .on_tray_icon_event(|tray, event| {
                        use tauri::tray::{MouseButton, MouseButtonState};
                        if let tauri::tray::TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            show_main_window(&app);
                        }
                    });

                // Linux 保留托盘实现的默认左键菜单行为，避免菜单弹出与窗口恢复同时触发。
                let _tray = tray_builder.build(app)?;
            }
            }

            // 默认关闭：不创建磁贴刷新任务或定时器。普通 NSIS/便携版没有
            // 包标识，即使用户打开设置开关也不会启动磁贴后台任务。
            let initial_tile_enabled = crate::windows_tile::read_live_tile_enabled(app.handle());
            crate::windows_tile::set_live_tile_enabled_flag(initial_tile_enabled);

            // 排查用：一条日志同时记下「能力状态」与「用户开关」。
            // 两者是**正交**的 —— 磁贴不显示可能是能力不具备（版本 / 身份包），
            // 也可能是用户自己关了开关；只有合起来看才能定位是哪一种。
            // 用 debug 而非 info：绝大多数用户属于 `IdentityMissing`
            // （正常安装、没跑启用脚本），记 info 就是噪音。
            let tile_status = crate::windows_tile::live_tile_status();
            log::debug!(
                "动态磁贴：{}（{}），用户开关={}",
                tile_status.as_str(),
                tile_status.reason(),
                initial_tile_enabled
            );

            if initial_tile_enabled {
                // 首次刷新异步执行，避免网络请求阻塞应用启动。
                if let Err(e) =
                    crate::windows_tile::start_tile_refresh_task(app.handle().clone(), true)
                {
                    log::warn!("启动动态磁贴刷新任务失败: {e}");
                }
            } else if crate::windows_tile::supports_live_tile() {
                // 已注册身份包时清理可能残留的旧磁贴；不开启周期任务。
                if let Err(e) = crate::windows_tile::clear_tile() {
                    log::warn!("清空动态磁贴队列失败: {e}");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(mobile)]
            let _ = (window, event);

            #[cfg(desktop)]
            {
            if window.label() != "main" {
                return;
            }
            // 移动和缩放事件同时更新状态，但最大化或最小化期间不覆盖普通窗口尺寸。
            if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                if window.is_minimized().unwrap_or(false) { return; }
                if let Ok(maximized) = window.is_maximized() {
                    WINDOW_MAXIMIZED.store(maximized, Ordering::SeqCst);
                    if maximized { return; }
                } else { return; }
            }
            match event {
                WindowEvent::Moved(pos) => {
                    if let Ok(mut guard) = WINDOW_STATE.lock() {
                        let prev = guard.get_or_insert(WindowState {
                            x: 0,
                            y: 0,
                            w: 1440,
                            h: 900,
                        });
                        prev.x = pos.x;
                        prev.y = pos.y;
                    }
                    schedule_window_state_persistence(window.app_handle().clone());
                }
                WindowEvent::Resized(size) => {
                    if let Ok(mut guard) = WINDOW_STATE.lock() {
                        let prev = guard.get_or_insert(WindowState {
                            x: 0,
                            y: 0,
                            w: 1440,
                            h: 900,
                        });
                        prev.w = size.width;
                        prev.h = size.height;
                    }
                    schedule_window_state_persistence(window.app_handle().clone());
                }
                WindowEvent::CloseRequested { api, .. } => {
                    log::info!(
                        "runtime.main_window_close_requested to_tray={}",
                        CLOSE_TO_TRAY.load(Ordering::SeqCst)
                    );
                    // 记忆窗口状态：先在隐藏/关闭前读取真实几何信息，避免托盘模式下查询失败。
                    persist_current_window_geometry(&window.app_handle());
                    // 关闭到托盘：仅主窗口点击关闭时隐藏而非退出，其余窗口（如外部链接窗口）正常关闭
                    if CLOSE_TO_TRAY.load(Ordering::SeqCst) {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                WindowEvent::Destroyed => {
                    log::info!("runtime.main_window_destroyed");
                    // 兼容其他 app.exit() 调用：窗口已销毁后只能使用最近一次缓存。
                    if REMEMBER_WINDOW_STATE.load(Ordering::SeqCst) {
                        if let Some(state) = cached_window_state() {
                            persist_window_geometry(&window.app_handle(), state);
                        }
                    }
                }
                _ => {}
            }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_index_v8_feeds,
            get_index_v8_feeds_paged,
            get_index_v8_entities_paged,
            get_hot_feeds,
            get_rank_feeds,
            get_latest_feeds,
            get_digest_feeds,
            get_cool_picture_rank,
            get_board_feeds,
            get_secondhand_feeds,
            get_game_list,
            get_app_list,
            search_apks,
            search_games,
            get_tab_config,
            update_home_tab_config,
            get_discovery_config,
            get_discovery_page_data,
            get_live_detail,
            get_search_suggestions,
            get_hot_searches,
            get_topic_detail_v7,
            get_product_detail,
            get_product_feeds,
            get_product_config,
            add_config_compare,
            remove_config_compare,
            get_product_brand_list,
            get_product_category_list,
            get_product_list,
            get_product_brand_products,
            get_secondhand_brand_list,
            get_secondhand_product_list,
            get_product_media_list,
            change_product_follow_status,
            change_product_wish_status,
            get_product_wish_list,
            get_product_buy_list,
            get_my_product_list,
            get_product_rating_chart,
            get_product_subtab_feeds,
            create_product_rating,
            get_product_rating_list,
            get_apk_rating_user_list,
            change_rating_status,
            get_dyh_detail,
            get_dyh_list,
            get_dyh_feeds,
            get_dyh_follow_list,
            get_dyh_subscribe_list,
            get_dyh_editor_list,
            get_feed_share_dyh_list,
            share_feed_to_dyh,
            get_event_list,
            get_event_detail,
            get_user_product_albums,
            get_goods_list_items,
            create_product_album,
            get_node_feeds,
            get_apk_feeds,
            check_login_info,
            get_feed_detail,
            get_public_feed_detail,
            get_editable_feed,
            resolve_live_photo_video,
            get_live_photo_video_header,
            resolve_video_url,
            get_reply_detail,
            get_hot_replies,
            search_all,
            search_by_type,
            search_feeds,
            get_feed_replies,
            get_sub_replies,
            get_user_space,
            get_public_user_space,
            get_user_qr_image,
            get_user_tab_data,
            get_user_profile,
            get_user_plugins,
            save_user_plugins,
            claim_user_plugin,
            coolapk::commands::get_my_profile,
            get_public_user_profile,
            get_user_remark_list,
            update_user_profile,
            change_avatar,
            update_user_cover,
            get_user_follow_nodes,
            get_user_forum_follow_list,
            get_user_feeds,
            get_user_like_list,
            get_topic_detail,
            get_topic_rating_list,
            get_topic_rating_chart,
            get_topic_feeds,
            get_topic_tab_data,
            get_topic_hub_data,
            get_app_detail,
            get_apk_comments,
            get_notification_count,
            get_emoji_pictures,
            coolapk::commands::add_emoji_picture,
            coolapk::commands::edit_emoji_pictures,
            clear_notification_count,
            delete_all_notifications,
            coolapk::commands::report_notification_click,
            coolapk::commands::read_notification,
            get_notifications,
            list_messages,
            get_recent_chat_users,
            list_chat_history,
            delete_message_chat,
            send_private_message,
            like_feed,
            unlike_feed,
            like_reply,
            unlike_reply,
            reply_feed,
            comment_apk,
            follow_user,
            unfollow_user,
            special_follow_user,
            cancel_follower,
            update_user_remark,
            create_answer,
            create_feed,
            update_feed,
            save_cookie_securely,
            check_login_status,
            update_device_profile,
            coolapk::commands::set_network_proxy,
            coolapk::commands::test_network_proxy,
            get_device_info,
            coolapk::commands::initialize_device_identity,
            coolapk::commands::generate_device_identity,
            coolapk::commands::use_saved_device_identity,
            list_accounts,
            login_as,
            save_account,
            persist_current_account,
            remove_account,
            clear_user_cookie,
            get_user_cookie,
            login_by_account,
            send_sms_vcode,
            login_by_mobile,
            get_image_data_url,
            save_image,
            save_file,
            save_image_data_url,
            open_image_in_system_viewer,
            open_url,
            coolapk::commands::select_equipment_product,
            fetch_external_page,
            coolapk::commands::submit_feed_report,
            open_login_webview,
            sync_login_webview,
            close_login_window,
            get_following_feeds,
            get_follow_user_list,
            get_fans_user_list,
            get_platform_info,
            set_close_to_tray,
            set_window_theme,
            system_theme::get_linux_system_theme,
            set_android_system_bar_theme,
            get_android_safe_area_top,
            log_android_runtime_info,
            pick_font_family,
            set_live_tile_enabled,
            update_desktop_tile,
            set_startup_flags,
            send_desktop_notification,
            download_update,
            get_update_distribution,
            take_update_install_error,
            install_update,
            is_update_package_available,
            cleanup_update_packages,
            quit_app,
            create_export_directory,
            export_json_file,
            get_cache_info,
            get_download_directory,
            clear_app_cache,
            clean_expired_cache,
            open_cache_directory,
            get_diagnostic_logs,
            clear_diagnostic_logs,
            get_diagnostic_verbose,
            set_diagnostic_verbose,
            get_album_detail,
            get_album_list,
            get_album_replies,
            get_user_album_list,
            create_album,
            edit_album,
            add_album_apk,
            delete_album_apk,
            get_apk_discoverers,
            get_apk_gift_list,
            get_apk_recommend_list,
            get_apk_related_apps,
            get_download_version_list,
            get_editor_choice_feeds,
            get_collection_item_list,
            get_collection_list,
            get_favorite_list,
            get_headline_feeds,
            get_collection_detail,
            create_collection,
            update_collection,
            delete_collection,
            remove_collection_item,
            clear_collection_invalid_items,
            get_feed_collection_status,
            get_apk_collection_status,
            update_collection_item,
            follow_collection,
            unfollow_collection,
            like_collection,
            unlike_collection,
            follow_dyh,
            unfollow_dyh,
            follow_live,
            unfollow_live,
            get_feed_forward_list,
            get_feed_like_list,
            get_feed_change_history,
            search_tags,
            follow_tag,
            unfollow_tag,
            get_device_feed_list,
            get_question_answers,
            follow_question,
            unfollow_question,
            invite_question_answer,
            get_vote_comments,
            create_user_vote,
            get_hit_history,
            get_recent_history,
            get_spam_feed_list,
            get_hidden_replies,
            get_followed_topics,
            search_users,
            get_search_suggestions_app,
            search_feed_topics,
            coolapk::commands::search_publish_topics,
            coolapk::commands::get_product_versions,
            get_product_detail_by_name,
            get_load_config,
            coolapk::commands::get_my_card_manager,
            coolapk::commands::update_my_card_config,
            get_home_tab_config,
            send_private_image,
            read_message,
            favorite_feed,
            unfavorite_feed,
            favorite_apk,
            unfavorite_apk,
            set_user_feed_top,
            delete_feed,
            delete_reply,
            create_forward,
            upload_image,
            get_black_list,
            get_ignore_list,
            get_limit_list,
            add_to_black_list,
            remove_from_black_list,
            add_to_ignore_list,
            remove_from_ignore_list,
            get_apk_url,
            get_apk_qr,
            start_apk_download,
            pause_apk_download,
            cancel_apk_download,
            delete_apk_download_file,
            open_apk_download_directory,
            install_apk_download,
            coolapk::android_notifications::configure_android_background_notifications,
            get_hot_topics,
            get_picture_list,
            get_update_list,
            get_user_rating_list,
            search_albums,
            search_apks_by_developer,
            search_apks_by_tag,
            get_goods_search_hot_words,
            search_goods,
            coolapk::commands::prepare_goods_by_url,
            coolapk::commands::upload_publish_video,
            get_goods_detail,
            get_goods_list_types,
            get_goods_list,
            get_goods_store_items,
            get_product_albums,
            get_my_goods_feeds,
            create_goods_list,
            edit_goods_list,
            add_goods_to_goods_list,
            delete_goods_list_items,
            edit_goods_list_item,
            vote_goods_list_item,
            bind_feed_to_goods_list,
            agent_bridge::agent_bridge_status,
            agent_bridge::agent_bridge_start,
            agent_bridge::agent_bridge_stop,
            agent_bridge::agent_bridge_save_preferences,
            agent_bridge::agent_bridge_rotate_token,
            agent_bridge::agent_bridge_set_mcp,
            agent_bridge::agent_bridge_confirm_result,
            agent_bridge::agent_bridge_usage_guide,
        ])
        .build(context)
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(&event, tauri::RunEvent::Exit) {
                diagnostics::end_session(app);
                // 退出时关闭本地 Agent 接口并清理 socket/配置文件，避免残留 token。
                let runtime = app.state::<agent_bridge::AgentBridgeRuntime>();
                agent_bridge::stop(runtime.inner());
            }
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    // 主窗口关闭到托盘后，点击 macOS Dock 图标会触发 Reopen 事件。
                    show_main_window(app);
                }
            }

            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
