/// 从 HTML 中提取 <title> 文本（大小写不敏感，无正则依赖）
pub(super) fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower[start..].find("</title>")? + start;
    let title = html[start..end].trim().to_string();
    if title.is_empty() { None } else { Some(title) }
}

/// 剔除网页外壳噪音标签（导航/页脚/脚本/样式等），保留正文骨架。
/// 轻量实现：逐标签扫描跳过指定块，未闭合时跳到下一个标签处兜底。
pub(super) fn strip_noise_tags(html: &str, tags: &[&str]) -> String {
    let lower = html.to_lowercase();
    let mut result = String::with_capacity(html.len());
    let mut pos = 0;
    let n = lower.len();
    while pos < n {
        let Some(rel) = lower[pos..].find('<') else {
            result.push_str(&html[pos..]);
            break;
        };
        let start = pos + rel;
        // 先拷贝 '<' 之前的纯文本
        result.push_str(&html[pos..start]);
        let rest = &lower[start + 1..];
        let name_end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == ':'))
            .unwrap_or(rest.len());
        let name = &rest[..name_end];

        // 注释/声明等空名标签（<!--、<!DOCTYPE>）：仅保留 '<' 字符
        if name.is_empty() {
            result.push('<');
            pos = start + 1;
            continue;
        }

        if tags.contains(&name) {
            let close_tag = format!("</{name}");
            let after_open = start + 1 + name_end;
            let block_end = if let Some(relc) = lower[after_open..].find(&close_tag) {
                let mut end = after_open + relc + close_tag.len();
                if let Some(gt) = lower[end..].find('>') {
                    end += gt + 1;
                }
                end
            } else {
                // 自闭合或无闭合标签：直接跳过该标签，继续找下一个 <
                after_open
            };
            pos = block_end;
        } else {
            // 普通标签原样保留
            result.push('<');
            pos = start + 1;
        }
    }
    result
}

/// 提取单个标签内部的 HTML（取第一个匹配的开闭标签对）
pub(super) fn extract_tag_content(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let open = format!("<{tag}");
    let start = lower.find(&open)?;
    let open_end = lower[start..].find('>')? + start + 1;
    let close = format!("</{tag}>");
    let rel = lower[open_end..].find(&close)?;
    Some(html[open_end..open_end + rel].to_string())
}

/// 对外部网页做可读性提取：先剥外壳噪音，再优先取 <article>/<main> 正文容器
pub(super) fn extract_readable_content(html: &str) -> String {
    let cleaned = strip_noise_tags(
        html,
        &[
            "script", "style", "nav", "header", "footer", "aside", "iframe", "form", "noscript",
        ],
    );
    for tag in ["article", "main"] {
        if let Some(inner) = extract_tag_content(&cleaned, tag) {
            return inner;
        }
    }
    cleaned
}
