use super::*;

pub(super) fn topic_hub_cursor(value: &Value) -> String {
    value
        .as_object()
        .and_then(|obj| get_str_by_keys(obj, &["entityId", "id"]))
        .unwrap_or_default()
}

pub(super) fn topic_hub_tab_target(obj: &serde_json::Map<String, Value>) -> Option<String> {
    let target_keys = [
        "url",
        "link",
        "pageUrl",
        "page_url",
        "pageName",
        "page_name",
        "requestArg",
        "request_arg",
        "requestUrl",
        "request_url",
        "apiUrl",
        "api_url",
    ];
    get_str_by_keys(obj, &target_keys).or_else(|| {
        obj.get("extraData")
            .or_else(|| obj.get("extra_data"))
            .and_then(Value::as_object)
            .and_then(|extra| get_str_by_keys(extra, &target_keys))
    })
}

pub(super) fn append_topic_hub_tab_entities(value: &Value, output: &mut Vec<Value>) {
    let Some(items) = value.as_array() else {
        return;
    };
    for entity in items {
        let Some(entity_obj) = entity.as_object() else {
            continue;
        };
        let title = get_str_by_keys(entity_obj, &["title", "title_txt", "name", "label"]);
        if title.is_some() && topic_hub_tab_target(entity_obj).is_some() {
            output.push(entity.clone());
        }
    }
}

pub(super) fn collect_topic_hub_tabs(value: &Value, output: &mut Vec<Value>) {
    let Some(obj) = value.as_object() else {
        return;
    };
    let template = obj
        .get("entityTemplate")
        .or_else(|| obj.get("entity_template"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_tab_card = template.contains("icontablink")
        || template.contains("iconlinkgrid")
        || template.contains("verticalcolumnsfullpagecard")
        || template.contains("selectorlinkgrid");
    if is_tab_card {
        if let Some(entities) = obj.get("entities").and_then(Value::as_array) {
            append_topic_hub_tab_entities(&Value::Array(entities.clone()), output);
        }
        if !output.is_empty() {
            return;
        }
    }
    for key in ["entities", "data", "rows", "card", "category", "tabs", "tabList", "tab_list"] {
        let Some(nested) = obj.get(key) else {
            continue;
        };
        if matches!(key, "tabs" | "tabList" | "tab_list") {
            append_topic_hub_tab_entities(nested, output);
        }
        if !output.is_empty() {
            return;
        }
        if let Some(items) = nested.as_array() {
            for entity in items {
                collect_topic_hub_tabs(entity, output);
                if !output.is_empty() {
                    return;
                }
            }
        } else {
            collect_topic_hub_tabs(nested, output);
            if !output.is_empty() {
                return;
            }
        }
    }
}

pub(super) fn extract_topic_hub_tabs(raw: &Value) -> Value {
    let mut tabs = Vec::new();
    for key in ["tabs", "tabList", "tab_list", "categories", "category"] {
        if let Some(value) = raw.get(key) {
            append_topic_hub_tab_entities(value, &mut tabs);
            if !tabs.is_empty() {
                return Value::Array(tabs);
            }
        }
    }
    if let Some(data) = raw.get("data").and_then(Value::as_array) {
        for item in data {
            collect_topic_hub_tabs(item, &mut tabs);
            if !tabs.is_empty() {
                break;
            }
        }
    }
    Value::Array(tabs)
}

pub(super) fn topic_hub_selected_tab(value: &Value) -> Option<String> {
    match value {
        Value::Array(items) => items.iter().find_map(topic_hub_selected_tab),
        Value::Object(obj) => {
            for key in ["extraData", "extra_data"] {
                let Some(extra) = obj.get(key) else {
                    continue;
                };
                if let Some(selected) = topic_hub_selected_tab(extra) {
                    return Some(selected);
                }
            }
            if let Some(selected) = get_str_by_keys(obj, &["selectedTab", "selected_tab"]) {
                return Some(selected);
            }
            obj.values().find_map(topic_hub_selected_tab)
        }
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|parsed| topic_hub_selected_tab(&parsed)),
        _ => None,
    }
}

pub(super) fn topic_hub_selected_category(raw: &Value) -> Option<(String, String)> {
    let tabs = extract_topic_hub_tabs(raw);
    let items = tabs.as_array()?;
    let selected_title = topic_hub_selected_tab(raw).unwrap_or_default();
    let selected = items
        .iter()
        .find(|item| get_str_by_keys(item.as_object().unwrap_or(&serde_json::Map::new()), &["title", "name", "label"]).as_deref() == Some(selected_title.as_str()))
        .or_else(|| items.first())?;
    let selected_obj = selected.as_object()?;
    let title = get_str_by_keys(selected_obj, &["title", "name", "label"])?;
    let url = topic_hub_tab_target(selected_obj)?;
    Some((title, url))
}

pub(super) fn topic_hub_result(raw: &Value) -> Value {
    let data = raw.get("data").cloned().unwrap_or_else(|| json!([]));
    let (first_item, last_item) = data
        .as_array()
        .map(|items| {
            (
                items.first().map(topic_hub_cursor).unwrap_or_default(),
                items.last().map(topic_hub_cursor).unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    json!({ "code": 200, "data": data, "firstItem": first_item, "lastItem": last_item, "tabs": extract_topic_hub_tabs(raw) })
}
