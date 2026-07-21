use encoding_rs::Encoding;
use regex::Regex;
use std::borrow::Cow;

pub enum RewriteTarget {
    Html,
    Css,
    Js,
    None,
}

/// Determine if content type should be rewritten, and which kind.
pub fn classify_content(content_type: &str) -> RewriteTarget {
    let ct = content_type.to_lowercase();
    if ct.contains("text/html") {
        RewriteTarget::Html
    } else if ct.contains("text/css") {
        RewriteTarget::Css
    } else if ct.contains("javascript") {
        RewriteTarget::Js
    } else {
        RewriteTarget::None
    }
}

/// Detect charset from Content-Type header.
pub fn detect_charset(content_type: &str) -> &'static Encoding {
    let re = Regex::new(r"charset\s*=\s*([^\s;]+)").unwrap();
    re.captures(content_type)
        .and_then(|c| c.get(1))
        .and_then(|m| Encoding::for_label(m.as_str().as_bytes()))
        .unwrap_or(encoding_rs::UTF_8)
}

/// Rewrite all sub-resource URLs in HTML/CSS to go through the proxy.
pub fn rewrite_urls<'a>(
    content: &'a str,
    base_url: &str,
    proxy_prefix: &str,
) -> Cow<'a, str> {
    let base_origin = {
        if let Some(pos) = base_url.find("://") {
            let after_scheme = &base_url[pos + 3..];
            match after_scheme.find('/') {
                Some(path_start) => base_url[..pos + 3 + path_start].to_string(),
                None => base_url.to_string(),
            }
        } else {
            base_url.trim_end_matches('/').to_string()
        }
    };

    let encode_proxy = |url: &str| -> String {
        format!("{}/proxy?url={}", proxy_prefix, urlencoding::encode(url))
    };

    let is_absolute = |url: &str| -> bool {
        url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//")
    };

    let resolve_url = |url: &str| -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else if url.starts_with("//") {
            let scheme = if base_url.starts_with("https") {
                "https:"
            } else {
                "http:"
            };
            format!("{}{}", scheme, url)
        } else if url.starts_with('/') {
            format!("{}{}", base_origin, url)
        } else {
            let base_dir = if base_url.ends_with('/') {
                base_url.to_string()
            } else {
                match base_url.rfind('/') {
                    Some(pos) => base_url[..=pos].to_string(),
                    None => format!("{}/", base_url),
                }
            };
            format!("{}{}", base_dir, url)
        }
    };

    let skip_proxied = |url: &str| -> bool {
        url.starts_with(proxy_prefix) || url.contains("/proxy?url=")
    };

    let mut result = content.to_string();

    // 1. Rewrite HTML tag attributes (src, href, action, poster, data-*)
    let re_attr = Regex::new(
        r#"(?i)(\b(?:src|href|action|poster|data-src|data-href|data-url)\s*=\s*)"([^"]+)"|(\b(?:src|href|action|poster|data-src|data-href|data-url)\s*=\s*)'([^']+)'"#
    ).unwrap();
    result = re_attr
        .replace_all(&result, |caps: &regex::Captures| {
            let double_quoted = caps.get(2).is_some();
            let (prefix, url, quote) = if double_quoted {
                (caps.get(1).unwrap().as_str().to_string(), caps.get(2).unwrap().as_str().to_string(), "\"")
            } else {
                (caps.get(3).unwrap().as_str().to_string(), caps.get(4).unwrap().as_str().to_string(), "'")
            };

            if skip_proxied(&url) {
                return format!("{}{}{}{}", prefix, quote, url, quote);
            }
            if url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with("//")
                || url.starts_with('/')
                || !url.contains("://")
            {
                let absolute = resolve_url(&url);
                if is_absolute(&absolute) {
                    return format!("{}{}{}{}", prefix, quote, encode_proxy(&absolute), quote);
                }
            }
            format!("{}{}{}{}", prefix, quote, url, quote)
        })
        .into_owned();

    // 2. Rewrite srcset attributes
    let re_srcset = Regex::new(
        r#"(?i)(\bsrcset\s*=\s*)"([^"]+)"|(\bsrcset\s*=\s*)'([^']+)'"#
    ).unwrap();
    result = re_srcset
        .replace_all(&result, |caps: &regex::Captures| {
            let double_quoted = caps.get(2).is_some();
            let (prefix, value, quote) = if double_quoted {
                (caps.get(1).unwrap().as_str().to_string(), caps.get(2).unwrap().as_str().to_string(), "\"")
            } else {
                (caps.get(3).unwrap().as_str().to_string(), caps.get(4).unwrap().as_str().to_string(), "'")
            };

            let rewritten = value
                .split(',')
                .map(|part| {
                    let part = part.trim();
                    let parts: Vec<&str> = part.splitn(2, |c: char| c.is_whitespace()).collect();
                    let url = parts[0].trim();
                    let descriptor = if parts.len() > 1 { parts[1] } else { "" };

                    if skip_proxied(&url) {
                        return part.to_string();
                    }
                    if url.starts_with("http://")
                        || url.starts_with("https://")
                        || url.starts_with("//")
                        || url.starts_with('/')
                        || !url.contains("://")
                    {
                        let absolute = resolve_url(&url);
                        if is_absolute(&absolute) {
                            if descriptor.is_empty() {
                                return encode_proxy(&absolute);
                            }
                            return format!("{} {}", encode_proxy(&absolute), descriptor);
                        }
                    }
                    part.to_string()
                })
                .collect::<Vec<_>>()
                .join(", ");

            format!("{}{}{}{}", prefix, quote, rewritten, quote)
        })
        .into_owned();

    // 3. Rewrite CSS url() references
    let re_css_url = Regex::new(r#"(?i)url\(\s*['"]?([^'")\s]+)['"]?\s*\)"#).unwrap();
    result = re_css_url
        .replace_all(&result, |caps: &regex::Captures| {
            let url = caps.get(1).unwrap().as_str().trim();
            if skip_proxied(&url) {
                return format!("url({})", url);
            }
            if url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with("//")
                || url.starts_with('/')
                || !url.contains("://")
            {
                let absolute = resolve_url(&url);
                if is_absolute(&absolute) {
                    return format!("url({})", encode_proxy(&absolute));
                }
            }
            format!("url({})", url)
        })
        .into_owned();

    // 4. Rewrite CSS @import statements
    let re_import = Regex::new(
        r#"(?i)(@import\s+)"([^"]+)"|(@import\s+)'([^']+)'"#,
    )
    .unwrap();
    result = re_import
        .replace_all(&result, |caps: &regex::Captures| {
            let double_quoted = caps.get(2).is_some();
            let (prefix, url, quote) = if double_quoted {
                (caps.get(1).unwrap().as_str().to_string(), caps.get(2).unwrap().as_str().to_string(), "\"")
            } else {
                (caps.get(3).unwrap().as_str().to_string(), caps.get(4).unwrap().as_str().to_string(), "'")
            };

            if skip_proxied(&url) {
                return format!("{}{}{}{}", prefix, quote, url, quote);
            }
            if url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with("//")
                || url.starts_with('/')
                || !url.contains("://")
            {
                let absolute = resolve_url(&url);
                if is_absolute(&absolute) {
                    return format!("{}{}{}{}", prefix, quote, encode_proxy(&absolute), quote);
                }
            }
            format!("{}{}{}{}", prefix, quote, url, quote)
        })
        .into_owned();

    // 5. Force <meta charset> to UTF-8
    let re_meta_charset =
        Regex::new(r#"(?i)(<meta\s+[^>]*?charset\s*=\s*['"])([^'">\s]+)(['"])"#).unwrap();
    result = re_meta_charset
        .replace_all(&result, "${1}UTF-8${3}")
        .into_owned();

    // 6. Force <meta http-equiv="Content-Type" content="...charset=XXX..."> to UTF-8
    let re_meta_hc = Regex::new(
        r#"(?i)(<meta\s+[^>]*?http-equiv\s*=\s*['"]?content-type['"]?\s+[^>]*?charset\s*=\s*)([^\s"';>]+)"#
    ).unwrap();
    result = re_meta_hc
        .replace_all(&result, "${1}UTF-8")
        .into_owned();

    // 7. Rewrite JavaScript import/export/require module paths
    let re_js_module = Regex::new(
        r#"(?i)((?:import|export)\s+(?:\{[^}]*\}\s*from\s*|[\w_$*{}\s,]+\s+from\s*)?['"]|require\s*\(\s*['"])([^'"]+)(['"])"#
    ).unwrap();
    result = re_js_module
        .replace_all(&result, |caps: &regex::Captures| {
            let prefix = caps.get(1).unwrap().as_str();
            let url = caps.get(2).unwrap().as_str();
            let suffix = caps.get(3).unwrap().as_str();

            if skip_proxied(url) {
                return format!("{}{}{}", prefix, url, suffix);
            }
            if url.starts_with('/') || url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with("./") || url.starts_with("../") {
                let absolute = resolve_url(url);
                if is_absolute(&absolute) {
                    return format!("{}{}{}", prefix, encode_proxy(&absolute), suffix);
                }
            }
            format!("{}{}{}", prefix, url, suffix)
        })
        .into_owned();

    // 8. Rewrite fetch('...') / importScripts('...')
    let re_js_func = Regex::new(
        r#"(?i)((?:fetch|importScripts)\s*\(\s*['""])([^'""]+)(['""])"#
    ).unwrap();
    result = re_js_func
        .replace_all(&result, |caps: &regex::Captures| {
            let prefix = caps.get(1).unwrap().as_str();
            let url = caps.get(2).unwrap().as_str();
            let suffix = caps.get(3).unwrap().as_str();

            if skip_proxied(url) {
                return format!("{}{}{}", prefix, url, suffix);
            }
            if url.starts_with('/') || url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with("./") || url.starts_with("../") {
                let absolute = resolve_url(url);
                if is_absolute(&absolute) {
                    return format!("{}{}{}", prefix, encode_proxy(&absolute), suffix);
                }
            }
            format!("{}{}{}", prefix, url, suffix)
        })
        .into_owned();

    // 9. Rewrite new Worker('...') / new SharedWorker('...')
    let re_js_worker = Regex::new(
        r#"(?i)(new\s+(?:Worker|SharedWorker)\s*\(\s*['""])([^'""]+)(['""])"#
    ).unwrap();
    result = re_js_worker
        .replace_all(&result, |caps: &regex::Captures| {
            let prefix = caps.get(1).unwrap().as_str();
            let url = caps.get(2).unwrap().as_str();
            let suffix = caps.get(3).unwrap().as_str();

            if skip_proxied(url) {
                return format!("{}{}{}", prefix, url, suffix);
            }
            if url.starts_with('/') || url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with("./") || url.starts_with("../") {
                let absolute = resolve_url(url);
                if is_absolute(&absolute) {
                    return format!("{}{}{}", prefix, encode_proxy(&absolute), suffix);
                }
            }
            format!("{}{}{}", prefix, url, suffix)
        })
        .into_owned();

    // 10. Rewrite XHR .open('METHOD', '...')
    let re_js_xhr = Regex::new(
        r#"(?i)(\.open\s*\(\s*['""](?:GET|POST|PUT|DELETE|PATCH)['""]\s*,\s*['""])([^'""]+)(['""])"#
    ).unwrap();
    result = re_js_xhr
        .replace_all(&result, |caps: &regex::Captures| {
            let prefix = caps.get(1).unwrap().as_str();
            let url = caps.get(2).unwrap().as_str();
            let suffix = caps.get(3).unwrap().as_str();

            if skip_proxied(url) {
                return format!("{}{}{}", prefix, url, suffix);
            }
            if url.starts_with('/') || url.starts_with("http://") || url.starts_with("https://") || url.starts_with("//") || url.starts_with("./") || url.starts_with("../") {
                let absolute = resolve_url(url);
                if is_absolute(&absolute) {
                    return format!("{}{}{}", prefix, encode_proxy(&absolute), suffix);
                }
            }
            format!("{}{}{}", prefix, url, suffix)
        })
        .into_owned();

    Cow::Owned(result)
}
