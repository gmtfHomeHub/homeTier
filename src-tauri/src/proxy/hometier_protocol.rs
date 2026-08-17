use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use http::Uri;
use reqwest::blocking::Client;
use tauri::{AppHandle, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::db::{models::ProxyCookieRow, Database};

/// 代理服务器端口，由 lib.rs 在启动后设置
static PROXY_PORT: OnceLock<u16> = OnceLock::new();

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn set_proxy_port(port: u16) {
    let _ = PROXY_PORT.set(port);
}

pub fn proxy_port() -> u16 {
    PROXY_PORT.get().copied().unwrap_or(1420)
}

/// 全局共享的 CookieJar（HTTP 代理 + WS 代理共用）
static COOKIE_JARS: OnceLock<Mutex<HashMap<String, PerHostCookieJar>>> = OnceLock::new();

pub fn cookie_jars() -> &'static Mutex<HashMap<String, PerHostCookieJar>> {
    COOKIE_JARS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 全局 Cookie 持久化数据库（由 setup 注入）
static COOKIE_DB: OnceLock<Arc<Database>> = OnceLock::new();

pub fn set_cookie_db(db: Arc<Database>) {
    if COOKIE_DB.set(db).is_ok() {
        seed_jars_from_db();
    }
}

/// 上游协议注册表：host:port → "http"/"https"（解决 build_forward_url 硬编码 http:// 问题）
static UPSTREAM_SCHEMES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub fn upstream_schemes() -> &'static Mutex<HashMap<String, String>> {
    UPSTREAM_SCHEMES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 注册上游协议；redirect_url 形如 "http://host:port/..." 或 "https://host:port/..."
pub fn register_upstream_scheme(host_key: &str, redirect_url: &str) {
    let scheme = if redirect_url.starts_with("https://") {
        "https"
    } else {
        "http"
    };
    upstream_schemes()
        .lock()
        .unwrap()
        .insert(host_key.to_string(), scheme.to_string());
}

/// 设备仿真模式（desktop / mobile），本地 HTTP 代理与 hometier 引擎共用
static DEVICE_MODE: OnceLock<Mutex<String>> = OnceLock::new();

pub fn set_device_mode(mode: &str) {
    let m = if mode.eq_ignore_ascii_case("mobile") {
        "mobile"
    } else {
        "desktop"
    };
    *DEVICE_MODE
        .get_or_init(|| Mutex::new("desktop".to_string()))
        .lock()
        .unwrap() = m.to_string();
}

pub fn device_mode() -> String {
    DEVICE_MODE
        .get()
        .map(|m| m.lock().unwrap().clone())
        .unwrap_or_else(|| "desktop".to_string())
}

/// 移动端仿真 UA（Android Chrome）
pub const MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36";

/// 下载目录（{app_data_dir}/downloads，由 setup 注入）
static DOWNLOAD_DIR: OnceLock<String> = OnceLock::new();

pub fn set_download_dir(dir: &str) {
    let _ = DOWNLOAD_DIR.set(dir.to_string());
}

pub fn download_dir() -> Option<String> {
    DOWNLOAD_DIR.get().cloned()
}

/// 待前端拉取的通知队列（下载完成事件）
static PENDING_DOWNLOADS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub fn pending_downloads() -> &'static Mutex<Vec<String>> {
    PENDING_DOWNLOADS.get_or_init(|| Mutex::new(Vec::new()))
}

fn lookup_upstream_scheme(host_key: &str) -> String {
    upstream_schemes()
        .lock()
        .unwrap()
        .get(host_key)
        .cloned()
        .unwrap_or_else(|| "http".to_string())
}

/// 从数据库加载全部 Cookie 到内存 jar（启动时调用一次）
fn seed_jars_from_db() {
    let Some(db) = COOKIE_DB.get() else { return };
    let rows = match db.list_proxy_cookies() {
        Ok(rows) => rows,
        Err(e) => {
            crate::log_error!("加载代理 Cookie 失败: {}", e);
            return;
        }
    };
    let mut jars = cookie_jars().lock().unwrap();
    for row in rows {
        let jar = jars
            .entry(row.host_key.clone())
            .or_insert_with(PerHostCookieJar::new);
        jar.0.push(StoredCookie {
            name: row.name,
            value: row.value,
            path: row.path,
            domain: row.domain,
            expires_at: row.expires_at.map(|v| v as u64),
            http_only: row.http_only,
            secure: row.secure,
            same_site: row.same_site,
        });
    }
}

/// 将一条上游 Set-Cookie 持久化到数据库（供 http_forward 引擎复用）
pub fn persist_cookie_to_db(host_key: &str, cookie: &StoredCookie, now_epoch: u64) {
    let Some(db) = COOKIE_DB.get() else { return };
    if let Some(exp) = cookie.expires_at {
        if exp <= now_epoch {
            let _ = db.delete_proxy_cookie(host_key, &cookie.name, &cookie.path);
            return;
        }
    }
    let _ = db.upsert_proxy_cookie(&ProxyCookieRow {
        host_key: host_key.to_string(),
        name: cookie.name.clone(),
        value: cookie.value.clone(),
        path: cookie.path.clone(),
        domain: cookie.domain.clone(),
        expires_at: cookie.expires_at.map(|v| v as i64),
        http_only: cookie.http_only,
        secure: cookie.secure,
        same_site: cookie.same_site.clone(),
    });
}

struct ForwardTarget {
    host: String,
    port: Option<u16>,
    path_and_query: String,
}

fn parse_target(uri: &Uri) -> Result<ForwardTarget, String> {
    let host = uri
        .host()
        .ok_or_else(|| format!("Missing host in URI: {}", uri))?;
    let port = uri.port_u16();
    let path_and_query = uri
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    Ok(ForwardTarget {
        host: host.to_string(),
        port,
        path_and_query,
    })
}

fn build_origin_str(host: &str, port: Option<u16>) -> String {
    match port {
        Some(p) => format!("{}:{}", host, p),
        None => host.to_string(),
    }
}

fn build_forward_url(target: &ForwardTarget) -> String {
    let scheme = lookup_upstream_scheme(&build_origin_str(&target.host, target.port));
    match target.port {
        Some(p) => format!(
            "{}://{}:{}{}",
            scheme, target.host, p, target.path_and_query
        ),
        None => format!("{}://{}{}", scheme, target.host, target.path_and_query),
    }
}

fn get_or_create_client(
    clients: &Arc<Mutex<HashMap<String, Client>>>,
    host: &str,
) -> Client {
    let mut map = clients.lock().unwrap();
    if let Some(client) = map.get(host) {
        return client.clone();
    }
    let mut builder = Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(60))
        .connect_timeout(std::time::Duration::from_secs(15));
    for cert in crate::proxy::proxy_ca_certs() {
        builder = builder.add_root_certificate(cert);
    }
    let client = builder.build().unwrap_or_default();
    map.insert(host.to_string(), client.clone());
    client
}

/// 通用 CSP 放宽：移除 frame-ancestors，为 connect-src/default-src 追加 sources，
/// 为 script-src 追加注入脚本 hash（无 script-src 时合成）；返回 None 表示无需修改
pub fn relax_csp(csp: &str, script_hash: &str, added_sources: &[String]) -> Option<String> {
    let mut directives: Vec<String> = csp
        .split(';')
        .map(|s| s.trim().to_string())
        .collect();
    let mut modified = false;
    let mut has_script_src = false;

    for directive in directives.iter_mut() {
        let trimmed = directive.trim().to_string();
        if trimmed.starts_with("frame-ancestors") {
            directive.clear();
            modified = true;
            continue;
        }
        if trimmed.starts_with("script-src") {
            has_script_src = true;
            if !script_hash.is_empty() && !trimmed.contains(script_hash) {
                *directive = format!("{} {}", trimmed, script_hash);
                modified = true;
            }
        }
        for src in added_sources {
            if trimmed.starts_with("connect-src") && !trimmed.contains(src) {
                *directive = format!("{} {}", trimmed, src);
                modified = true;
            }
            if trimmed.starts_with("default-src") && !trimmed.contains(src) {
                *directive = format!("{} {}", trimmed, src);
                modified = true;
            }
        }
    }

    if !has_script_src && !script_hash.is_empty() {
        directives.push(format!("script-src {}", script_hash));
        modified = true;
    }
    if !modified {
        return None;
    }

    let joined: Vec<&str> = directives
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .collect();
    let joined = joined.join("; ");
    Some(joined)
}

/// 导航桥接 JS：iframe 内自维护会话历史栈，通过 postMessage 与宿主工具栏通信。
/// 宿主发 {__ht_nav_cmd:{cmd:"back"|"forward"|"go",url}}，本脚本回发 {__ht_nav:{idx,len,url}}。
/// 用 location.replace 在栈内跳转，避免生成会“逃逸”代理的浏览器历史条目。
const NAV_BRIDGE_JS: &str = r#"
;(function(){
var st=[],ix=-1;
function push(){var u=location.href;if(st.length&&st[ix]===u)return;st=st.slice(0,ix+1);st.push(u);ix=st.length-1;notify()}
function notify(){try{parent.postMessage({__ht_nav:1,idx:ix,len:st.length,url:location.href},"*")}catch(e){}}
function back(){if(ix>0){ix--;location.replace(st[ix])}}
function fwd(){if(ix<st.length-1){ix++;location.replace(st[ix])}}
function go(u){if(typeof u==="string"&&u.indexOf(location.origin)===0){location.replace(u);push()}}
setInterval(push,800);
window.addEventListener("message",function(e){var d=e.data;if(!d||!d.__ht_nav_cmd)return;if(d.__ht_nav_cmd.cmd==="back")back();else if(d.__ht_nav_cmd.cmd==="forward")fwd();else if(d.__ht_nav_cmd.cmd==="go")go(d.__ht_nav_cmd.url)},false);
})()"#;

/// 移动仿真 JS 片段：DPR=3、触摸能力、matchMedia 误报。
/// 在当前架构下（页面运行在 WebView 的 iframe 内，非真实浏览器），
/// 站点通过 JS 探测设备特性的结果由本脚本覆盖，注入位置在页面最早执行。
const EMULATION_JS: &str = r#"
;(function(){
try{Object.defineProperty(window,"devicePixelRatio",{value:3})}catch(e){}
try{Object.defineProperty(navigator,"maxTouchPoints",{value:5})}catch(e){}
try{window.ontouchstart=null;window.ontouchend=null}catch(e){}
var OM=window.matchMedia?window.matchMedia.bind(window):function(q){return mq(String(q),false)};
function mq(q,m){var l={media:q,matches:m,addListener:function(){},removeListener:function(){},addEventListener:function(){},removeEventListener:function(){},onchange:null,dispatchEvent:function(){return false}};return l}
var MM={"pointer: coarse":true,"pointer: fine":false,"hover: hover":false,"hover: none":true,"any-pointer: coarse":true,"any-pointer: fine":false,"any-hover: none":true,"any-hover: hover":false};
window.matchMedia=function(q){q=String(q).replace(/\s+/g," ");for(var k in MM){if(q.indexOf(k)>=0)return mq(q,MM[k])}return OM(q)};
})()"#;

/// 跨源 iframe autofocus 抑制：删除 autofocus 属性并拦截无手势的 focus()，
/// 消除 WebKitGTK「Blocked autofocusing on a form control in a cross-origin subframe.」报错。
/// 用户主动 click/mousedown 后放行真实 focus（保持表单可用）。
const AUTOFOCUS_JS: &str = r#"
;(function(){
function allow(){window.__ht_gesture=1;window.removeEventListener("mousedown",allow,true);window.removeEventListener("keydown",allow,true);window.removeEventListener("touchstart",allow,true)}
window.addEventListener("mousedown",allow,true);window.addEventListener("keydown",allow,true);window.addEventListener("touchstart",allow,true);
var _focus=HTMLElement.prototype.focus;HTMLElement.prototype.focus=function(o){try{if(window.__ht_gesture||document.hasFocus()){return _focus.call(this,o)}}catch(e){}};
function strip(){try{var a=document.querySelectorAll("[autofocus],input[autofocus],textarea[autofocus],select[autofocus],button[autofocus]");for(var i=0;i<a.length;i++){a[i].removeAttribute("autofocus")}}catch(e){}}
if(document.readyState==="loading"){document.addEventListener("DOMContentLoaded",strip,false)}else{strip()}
})()"#;

/// 注入移动端 viewport meta（缺失时）
fn inject_viewport_meta(html: &mut String) {
    let lower = html.to_lowercase();
    if lower.contains("viewport") {
        return;
    }
    if let Some(pos) = lower.find("<head") {
        let after = pos + 5;
        if let Some(close) = lower[after..].find('>') {
            let at = after + close + 1;
            html.insert_str(
                at,
                "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            );
        }
    }
}

/// 从 URL 提取 origin（scheme://host[:port]）
fn extract_origin(url: &str) -> &str {
    let Some(pos) = url.find("://") else {
        return url.trim_end_matches('/');
    };
    let after = &url[pos + 3..];
    match after.find('/') {
        Some(slash) => &url[..pos + 3 + slash],
        None => url,
    }
}

/// 本地 HTTP 代理（local-http）模式的页面注入脚本：动态 URL 重写（fetch/XHR/元素属性/WS）。
/// 页面 origin 已是 http://127.0.0.1:{proxy_port}，静态改写覆盖 HTML 属性/CSS/JS 字面量；
/// 本脚本兜底拦截站点 JS **运行时**用 location/source origin 拼出的绝对 URL（CasaOS 类 SPA 必现）。
/// 返回 (注入后 HTML, CSP script hash)
pub fn inject_local_http_script(
    html_bytes: Vec<u8>,
    is_mobile: bool,
    proxy_key: &str,
    source_url: &str,
) -> (Vec<u8>, String) {
    let mut html = match String::from_utf8(html_bytes.clone()) {
        Ok(h) => h,
        Err(_) => return (html_bytes, String::new()),
    };

    let source_origin = extract_origin(source_url).to_string();
    let mut js_content = format!(
        r#"(function(){{
var K="{}",O="{}",F=location.origin;
function R(u){{if(typeof u!=="string"||!u)return u;if(u.indexOf("/__proxy__")>=0)return u;if(O&&u.indexOf(O)===0)return F+"/__proxy__"+K+u.slice(O.length);if(u.indexOf(F)===0)return F+"/__proxy__"+K+u.slice(F.length);if(u.charAt(0)==="/")return F+"/__proxy__"+K+u;return u}}
function RS(u){{if(typeof u!=="string"||!u)return u;return u.replace(/url\(\s*["']?([^"')]+)["']?\s*\)/g,function(m,s){{return "url("+R(s)+")"}})}}
var _f=window.fetch;if(_f)window.fetch=function(u,i){{if(typeof u==="string"){{u=R(u)}}else if(u&&u.url){{var n=R(u.url);if(n!==u.url)u=new Request(n,u)}}return _f.call(this,u,i)}};
var _xo=XMLHttpRequest.prototype.open;XMLHttpRequest.prototype.open=function(m,u){{if(typeof u==="string"){{arguments[1]=R(u)}}return _xo.apply(this,arguments)}};
var _sa=Element.prototype.setAttribute;Element.prototype.setAttribute=function(n,v){{if(typeof n==="string"&&(n==="src"||n==="href"||n==="srcset"||n==="poster"||n==="data-src"||n==="data-href"||n==="data-url")){{v=R(String(v))}}else if(typeof n==="string"&&(n==="style"||n==="cssText")){{v=RS(String(v))}}return _sa.call(this,n,v)}};
["src","href","srcset"].forEach(function(a){{"HTMLLinkElement,HTMLScriptElement,HTMLImageElement,HTMLIFrameElement,HTMLSourceElement,HTMLVideoElement,HTMLAudioElement".split(",").forEach(function(T){{try{{var P=window[T]&&window[T].prototype;if(!P)return;var d=Object.getOwnPropertyDescriptor(P,a);if(!d||!d.set)return;Object.defineProperty(P,a,{{get:d.get,set:function(v){{v=R(String(v));d.set.call(this,v)}},configurable:true}})}}catch(e){{}}}})}});
(function(){{try{{var P=window.CSSStyleDeclaration&&window.CSSStyleDeclaration.prototype;if(!P)return;["background","backgroundImage","listStyleImage","cursor","borderImage"].forEach(function(a){{var d=Object.getOwnPropertyDescriptor(P,a);if(!d||!d.set)return;Object.defineProperty(P,a,{{get:d.get,set:function(v){{d.set.call(this,RS(String(v)))}},configurable:true}})}})}}catch(e){{}}}})();
(function(){{try{{var P=window.HTMLStyleElement&&window.HTMLStyleElement.prototype;if(!P)return;["textContent","innerHTML"].forEach(function(a){{var d=Object.getOwnPropertyDescriptor(P,a);if(!d||!d.set)return;Object.defineProperty(P,a,{{get:d.get,set:function(v){{d.set.call(this,RS(String(v)))}},configurable:true}})}})}}catch(e){{}}}})();
(function(){{try{{var P=window.CSSStyleSheet&&window.CSSStyleSheet.prototype;if(!P)return;var _ir=P.insertRule;if(_ir)P.insertRule=function(c,i){{if(typeof c==="string")c=RS(c);return _ir.call(this,c,i)}};var _ar=P.addRule;if(_ar)P.addRule=function(s,d,i){{if(typeof d==="string")d=RS(d);return _ar.call(this,s,d,i)}}}}catch(e){{}}}})();
var _WS=window.WebSocket;if(_WS)window.WebSocket=function(u,p){{if(typeof u=="string"){{u=r_ws(u)}}return new _WS(u,p)}};window.WebSocket.prototype=_WS.prototype;window.WebSocket.CONNECTING=0;window.WebSocket.OPEN=1;window.WebSocket.CLOSING=2;window.WebSocket.CLOSED=3;
function r_ws(u){{if(typeof u!=="string")return u;if(u.indexOf("/__proxy__")>=0)return u;var m=u.match(/^(wss?):\/\/([^\/?#]*)(.*)$/i);if(!m)return u;var h=m[2],rest=m[3]||"/",oh="";if(O){{var oo=O.split("://")[1]||"";oh=oo.split("/")[0]}}if(h===location.host||(oh&&h===oh))return "ws://"+location.host+"/__proxy__"+K+"?"+m[1].toLowerCase()+"="+encodeURIComponent(h+rest);return "ws://"+location.host+"?"+m[1].toLowerCase()+"="+encodeURIComponent(h+rest)}}
function w(u){{if(typeof u!=="string"||!u)return u;if(u.indexOf("/__proxy__")>=0)return u;if(O&&u.indexOf(O)===0)return F+"/__proxy__"+K+u.slice(O.length);if(u.charAt(0)==="/")return F+"/__proxy__"+K+u;return u}}
(function(){{if(window.__htLoc)return;window.__htLoc=1;try{{var P=(window.Location||{{}}).prototype;if(!P)return;var Hd=Object.getOwnPropertyDescriptor(P,"href");if(Hd&&typeof Hd.set==="function"){{Object.defineProperty(P,"href",{{get:Hd.get,set:function(v){{Hd.set.call(this,w(String(v)))}},configurable:true}})}};var _a=P.assign;if(typeof _a==="function"){{P.assign=function(u){{u=w(String(u));return _a.call(this,u)}}}};var _r=P.replace;if(typeof _r==="function"){{P.replace=function(u){{u=w(String(u));return _r.call(this,u)}}}}}}catch(e){{}}}})();
}})()"#,
        proxy_key, source_origin
    );
if is_mobile {
        js_content.push_str("\n");
        js_content.push_str(EMULATION_JS);
        inject_viewport_meta(&mut html);
    }
    js_content.push_str("\n");
    js_content.push_str(NAV_BRIDGE_JS);
    js_content.push_str("\n");
    js_content.push_str(AUTOFOCUS_JS);

    debug_assert!(!regex::Regex::new(r"\}\)\s*\(\s*function")
            .unwrap()
            .is_match(&js_content),
            "JS concatenation missing semicolon separator, IIFE adjacency causes syntax error"
        );

    let hash = crate::crypto::sha256(js_content.as_bytes());
    let encoded = base64::engine::general_purpose::STANDARD.encode(hash);
    let csp_hash = format!("'sha256-{}'", encoded);
    let script_tag = format!("<script id=\"__ht\">{}</script>", js_content);

    let lower = html.to_lowercase();
    if let Some(pos) = lower.find("<head") {
        let after = pos + 5;
        let rest = &lower[after..];
        if let Some(close) = rest.find('>') {
            let inject_at = after + close + 1;
            html.insert_str(inject_at, &script_tag);
            return (html.into_bytes(), csp_hash);
        }
    }
    if let Some(pos) = lower.find("</head>") {
        html.insert_str(pos, &script_tag);
    } else if let Some(pos) = lower.find("<body") {
        html.insert_str(pos, &script_tag);
    } else {
        html.insert_str(0, &script_tag);
    }
    (html.into_bytes(), csp_hash)
}

fn strip_iframe_restrictions(resp: &mut http::Response<Vec<u8>>, script_hash: &str) {
    let headers = resp.headers_mut();
    headers.remove("x-frame-options");

    if script_hash.is_empty() {
        return;
    }

    // 处理 CSP：移除 frame-ancestors，添加 hometierproxy: 到 connect-src / default-src
    // 添加 script-src hash 放行注入脚本
    if let Some(csp) = headers.get("content-security-policy") {
        if let Ok(val) = csp.to_str() {
            let sources = ["hometierproxy:".to_string()];
            if let Some(joined) = relax_csp(val, script_hash, &sources) {
                if joined.is_empty() {
                    headers.remove("content-security-policy");
                } else if let Ok(hv) = joined.parse() {
                    headers.insert("content-security-policy", hv);
                }
            }
        }
    }
}

fn rewrite_html_body(body: &[u8], origin_str: &str) -> Vec<u8> {
    let old_prefix = format!("http://{}", origin_str);
    let new_prefix = format!("hometierproxy://{}", origin_str);
    match String::from_utf8(body.to_vec()) {
        Ok(html) => html.replace(&old_prefix, &new_prefix).into_bytes(),
        Err(_) => Vec::new(),
    }
}

/// 注入代理修复脚本：修复 location 属性 + 拦截 fetch/XHR 重写 URL + 可选移动仿真
fn inject_proxy_script(html_bytes: Vec<u8>, host_key: &str, is_mobile: bool) -> (Vec<u8>, String) {
    let mut html = match String::from_utf8(html_bytes.clone()) {
        Ok(h) => h,
        Err(_) => return (html_bytes, String::new()),
    };

    let proxy_port = PROXY_PORT.get().copied().unwrap_or(1420);
    let mut js_content = format!(
        r#"(function(){{
var H="{}",P="{}";
var _f=window.fetch;if(_f)window.fetch=function(u,i){{if(typeof u=="string"){{u=r(u)}}else if(u&&u.url){{var nu=r(u.url);if(nu!==u.url)u=new Request(nu,u)}};return _f.call(this,u,i)}};
var _o=XMLHttpRequest.prototype.open;XMLHttpRequest.prototype.open=function(m,u){{if(typeof u=="string"){{arguments[1]=r(u)}}return _o.apply(this,arguments)}};
var _sa=Element.prototype.setAttribute;Element.prototype.setAttribute=function(n,v){{if(typeof n=="string"&&(n=="src"||n=="href"||n=="srcset"||n=="poster"||n=="data-src"||n=="data-href"||n=="data-url")){{v=r(String(v))}}else if(typeof n=="string"&&(n=="style"||n=="cssText")){{v=RS(String(v))}}return _sa.call(this,n,v)}};
["src","href","srcset"].forEach(function(a){{"HTMLLinkElement,HTMLScriptElement,HTMLImageElement,HTMLIFrameElement,HTMLSourceElement,HTMLVideoElement,HTMLAudioElement".split(",").forEach(function(T){{try{{var P=window[T]&&window[T].prototype;if(!P)return;var d=Object.getOwnPropertyDescriptor(P,a);if(!d||!d.set)return;Object.defineProperty(P,a,{{get:d.get,set:function(v){{v=r(String(v));d.set.call(this,v)}},configurable:true}})}}catch(e){{}}}})}});
(function(){{try{{var P=window.CSSStyleDeclaration&&window.CSSStyleDeclaration.prototype;if(!P)return;["background","backgroundImage","listStyleImage","cursor","borderImage"].forEach(function(a){{var d=Object.getOwnPropertyDescriptor(P,a);if(!d||!d.set)return;Object.defineProperty(P,a,{{get:d.get,set:function(v){{d.set.call(this,RS(String(v)))}},configurable:true}})}})}}catch(e){{}}}})();
(function(){{try{{var P=window.HTMLStyleElement&&window.HTMLStyleElement.prototype;if(!P)return;["textContent","innerHTML"].forEach(function(a){{var d=Object.getOwnPropertyDescriptor(P,a);if(!d||!d.set)return;Object.defineProperty(P,a,{{get:d.get,set:function(v){{d.set.call(this,RS(String(v)))}},configurable:true}})}})}}catch(e){{}}}})();
(function(){{try{{var P=window.CSSStyleSheet&&window.CSSStyleSheet.prototype;if(!P)return;var _ir=P.insertRule;if(_ir)P.insertRule=function(c,i){{if(typeof c==="string")c=RS(c);return _ir.call(this,c,i)}};var _ar=P.addRule;if(_ar)P.addRule=function(s,d,i){{if(typeof d==="string")d=RS(d);return _ar.call(this,s,d,i)}}}}catch(e){{}}}})();
var _WS=window.WebSocket;if(_WS)window.WebSocket=function(u,p){{if(typeof u=="string"){{u=r_ws(u)}}return new _WS(u,p)}};window.WebSocket.prototype=_WS.prototype;window.WebSocket.CONNECTING=0;window.WebSocket.OPEN=1;window.WebSocket.CLOSING=2;window.WebSocket.CLOSED=3;
function r_ws(u){{if(typeof u!=="string")return u;if(u.indexOf("/__proxy__")>=0)return u;var m=u.match(/^(wss?):\/\/([^\/?#]*)(.*)$/i);if(!m)return u;var h=m[2],rest=m[3]||"/";if(h==="127.0.0.1:"+P||h===H||h.indexOf("hometierproxy")===0){{return "ws://127.0.0.1:"+P+"?"+m[1].toLowerCase()+"="+encodeURIComponent(H+rest)}}return "ws://127.0.0.1:"+P+"?"+m[1].toLowerCase()+"="+encodeURIComponent(m[2].replace('hometierproxy',H)+rest)}};
function r(u){{if(u.indexOf("hometierproxy://")===0)return u;if(u.charAt(0)==='/')return "hometierproxy://"+H+"/"+u.replace(/^\/+/,"");var m=u.match(/^https?:\/\/hometierproxy(?::\d+)?(?=\/|\?|#|$)/i);if(m)return u.replace(/^https?:\/\/[^\/]+/,"hometierproxy://"+H);return u.replace(RegExp("^https?://"+H.replace(/\./g,"\\.")+"(?=/|\\?|#|$)","i"),"hometierproxy://"+H)}};
function RS(u){{if(typeof u!=="string"||!u)return u;return u.replace(/url\(\s*["']?([^"')]+)["']?\s*\)/g,function(m,s){{return "url("+r(s)+")"}})}};
function w(u){{if(typeof u!=="string"||!u)return u;if(u.indexOf("hometierproxy://")===0)return u;return r(u)}}
(function(){{if(window.__htLoc)return;window.__htLoc=1;try{{var P=(window.Location||{{}}).prototype;if(!P)return;var Hd=Object.getOwnPropertyDescriptor(P,"href");if(Hd&&typeof Hd.set==="function"){{Object.defineProperty(P,"href",{{get:Hd.get,set:function(v){{Hd.set.call(this,w(String(v)))}},configurable:true}})}};var _a=P.assign;if(typeof _a==="function"){{P.assign=function(u){{u=w(String(u));return _a.call(this,u)}}}};var _r=P.replace;if(typeof _r==="function"){{P.replace=function(u){{u=w(String(u));return _r.call(this,u)}}}}}}catch(e){{}}}})();
}})()"#,
        host_key, proxy_port
    );
    if is_mobile {
        js_content.push_str("\n");
        js_content.push_str(EMULATION_JS);
        inject_viewport_meta(&mut html);
    }
    js_content.push_str("\n");
    js_content.push_str(NAV_BRIDGE_JS);
    js_content.push_str("\n");
    js_content.push_str(AUTOFOCUS_JS);

// function r(l){{var u=l;if(u.match(/hometierproxy/g).length>1){{var u1=u.slice(u.lastIndexOf('hometierproxy'));u = u1;}}if(u.indexOf("hometierproxy://")>= 0) {{u=u.slice(u.indexOf("hometierproxy://"));}} if(u.indexOf("hometierproxy")>=0){{var u2=u.slice(u.indexOf("hometierproxy")); if(u2.indexOf("://"+H)===0) {{return u2;}} if(u2.indexOf(H)<0&&u2.indexOf("://")<0) return u2.replace("hometierproxy","hometierproxy://"+H); return "hometierproxy://"+H+u2.slice(H);}}if(u.charAt(0) === "/") return "hometierproxy://" + H + "/" + u.replace(/^\//, "");var m = u.match(/^https?:\/\/hometierproxy(?::\d+)?(?=\/|\?|#|$)/i);if(m)return u.replace(/^https?:\/\/[^\/]+/, "hometierproxy://" + H); return u.replace(RegExp("^https?://"+H.replace(/\./g,"\\.")+"(?=/|\\?|#|$)","i"),"hometierproxy://"+H);}}
    debug_assert!(
        !regex::Regex::new(r"\}\)\s*\(\s*function")
            .unwrap()
            .is_match(&js_content),
        "JS concatenation missing semicolon separator, IIFE adjacency causes syntax error"
    );
    let hash = crate::crypto::sha256(js_content.as_bytes());
    let encoded = base64::engine::general_purpose::STANDARD.encode(hash);
    let csp_hash = format!("'sha256-{}'", encoded);
    let script_tag = format!("<script id=\"__ht\">{}</script>", js_content);

    // 注入到 <head> 之后（最早执行，拦截页面脚本）
    let lower = html.to_lowercase();
    if let Some(pos) = lower.find("<head") {
        let after = pos + 5;
        let rest = &lower[after..];
        if let Some(close) = rest.find('>') {
            let inject_at = after + close + 1;
            html.insert_str(inject_at, &script_tag);
            return (html.into_bytes(), csp_hash);
        }
    }
    // fallback: 在 </head> 前注入
    if let Some(pos) = lower.find("</head>") {
        html.insert_str(pos, &script_tag);
    } else if let Some(pos) = lower.find("<body") {
        html.insert_str(pos, &script_tag);
    } else {
        html.insert_str(0, &script_tag);
    }
    (html.into_bytes(), csp_hash)
}

fn is_html_content(content_type: Option<&http::HeaderValue>) -> bool {
    match content_type.and_then(|v| v.to_str().ok()) {
        Some(ct) => {
            let ct = ct.to_lowercase();
            ct.contains("text/html") || ct.contains("application/xhtml")
        }
        None => false,
    }
}

/// 检查响应体是否以 HTML 文档标记开头（区分真正的 HTML 页面与 text/html 的 JSON API）
pub fn looks_like_html_body(body: &[u8]) -> bool {
    let start = if body.len() > 15 { &body[..15] } else { body };
    let lower = start.to_ascii_lowercase();
    lower.starts_with(b"<!doctype") || lower.starts_with(b"<html")
}

// --- Cookie Jar ---
//
// 完整 Set-Cookie 属性解析（Path/Domain/Expires/Max-Age/HttpOnly/Secure/SameSite），
// 内存 jar 作为读写热路径，DB（proxy_cookies 表）作为持久化镜像：
//   - 会话 Cookie（无 Expires/Max-Age）仅存内存，不落库（重启即失效，与浏览器一致）
//   - 持久 Cookie 落库；重启后 seed_jars_from_db() 恢复
//   - 过期/删除（Max-Age<=0 或过期）从内存与 DB 同时清除

pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub path: String,
    pub domain: Option<String>,
    pub expires_at: Option<u64>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: Option<String>,
}

impl Clone for StoredCookie {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            value: self.value.clone(),
            path: self.path.clone(),
            domain: self.domain.clone(),
            expires_at: self.expires_at,
            http_only: self.http_only,
            secure: self.secure,
            same_site: self.same_site.clone(),
        }
    }
}

pub struct PerHostCookieJar(Vec<StoredCookie>);

impl PerHostCookieJar {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// 解析一条 Set-Cookie 并写入 jar；返回落库所需信息（None = 已删除/过期）
    pub fn add_set_cookie(&mut self, header: &str) -> Option<StoredCookie> {
        let cookie = parse_set_cookie(header)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 删除语义：Max-Age<=0 或 Expires 已过期
        let expired = cookie
            .expires_at
            .map(|exp| exp <= now)
            .unwrap_or(false);
        if expired {
            self.remove(&cookie.name, &cookie.path);
            return None;
        }

        self.insert(cookie.clone());
        Some(cookie)
    }

    fn insert(&mut self, cookie: StoredCookie) {
        // Set-Cookie 以 (name, path, domain) 为身份覆盖旧值
        self.0.retain(|c| {
            c.name != cookie.name || c.path != cookie.path || c.domain != cookie.domain
        });
        self.0.push(cookie);
    }

    fn remove(&mut self, name: &str, path: &str) {
        self.0.retain(|c| c.name != name || c.path != path);
    }

    /// 按 RFC 6265 路径匹配生成 Cookie 头（domain 匹配：host 末尾匹配 domain 或精确相等）
    pub fn build_cookie_header(&mut self, host: &str, request_path: &str) -> Option<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.0.retain(|c| match c.expires_at {
            Some(exp) => exp > now,
            None => true,
        });

        if self.0.is_empty() {
            return None;
        }

        let matched: Vec<String> = self
            .0
            .iter()
            .filter(|c| {
                // Domain 匹配：无 domain 属性 = host-only（精确 host）
                let domain_ok = match &c.domain {
                    Some(dom) => {
                        let dom = dom.trim_start_matches('.');
                        host == dom || host.ends_with(&format!(".{}", dom))
                    }
                    None => true,
                };
                if !domain_ok {
                    return false;
                }
                // Path 匹配（RFC 6265 5.1.4）
                let cp = c.path.trim_end_matches('/');
                if cp.is_empty() || cp == "/" {
                    true
                } else if request_path.starts_with(cp) {
                    let rest = &request_path[cp.len()..];
                    rest.is_empty() || rest.starts_with('/')
                } else {
                    false
                }
            })
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();

        if matched.is_empty() {
            None
        } else {
            Some(matched.join("; "))
        }
    }
}

/// 完整解析一条 Set-Cookie 头（RFC 6265）
fn parse_set_cookie(header: &str) -> Option<StoredCookie> {
    let parts: Vec<&str> = header.split(';').collect();
    if parts.is_empty() {
        return None;
    }

    let first_eq = parts[0].find('=')?;
    let name = parts[0][..first_eq].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let value = parts[0][first_eq + 1..].trim().to_string();

    let mut path = "/".to_string();
    let mut domain: Option<String> = None;
    let mut expires_at: Option<u64> = None;
    let mut http_only = false;
    let mut secure = false;
    let mut same_site: Option<String> = None;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for part in &parts[1..] {
        let part = part.trim();
        let (key, val) = match part.find('=') {
            Some(eq) => {
                let k = part[..eq].trim().to_ascii_lowercase();
                let v = part[eq + 1..].trim().to_string();
                (k, v)
            }
            None => (part.to_ascii_lowercase(), String::new()),
        };

        match key.as_str() {
            "path" => {
                if !val.is_empty() {
                    path = val;
                }
            }
            "domain" => {
                if !val.is_empty() {
                    // 去除前导点，统一匹配逻辑
                    domain = Some(val.trim_start_matches('.').to_string());
                }
            }
            "max-age" => {
                if let Ok(secs) = val.parse::<i64>() {
                    expires_at = if secs <= 0 {
                        Some(0)
                    } else {
                        Some(now + secs as u64)
                    };
                }
            }
            "expires" => {
                if let Some(ts) = parse_http_date(&val) {
                    expires_at = Some(ts);
                }
            }
            "httponly" => http_only = true,
            "secure" => secure = true,
            "samesite" => {
                let v = val.to_ascii_lowercase();
                if v == "lax" || v == "strict" || v == "none" {
                    same_site = Some(v);
                }
            }
            _ => {}
        }
    }

    Some(StoredCookie {
        name,
        value,
        path,
        domain,
        expires_at,
        http_only,
        secure,
        same_site,
    })
}

/// 解析 HTTP-date（Expires 属性）：支持 RFC 1123 / RFC 850 / asctime 三种格式
fn parse_http_date(s: &str) -> Option<u64> {
    let s = s.trim();

    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return Some(dt.timestamp().max(0) as u64);
    }

    // RFC 850: Sunday, 06-Nov-94 08:49:37 GMT
    let s2 = s.replacen(',', "", 1);
    let s2 = s2.trim();
    let fmt = "%A %d-%b-%y %H:%M:%S GMT";
    if let Ok(n) = chrono::NaiveDateTime::parse_from_str(s2, fmt) {
        return Some(n.and_utc().timestamp().max(0) as u64);
    }

    // asctime: Sun Nov  6 08:49:37 1994
    let fmt = "%a %b %e %H:%M:%S %Y";
    if let Ok(n) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
        return Some(n.and_utc().timestamp().max(0) as u64);
    }

    None
}

// --- Request handling ---

fn handle_request<R: Runtime>(
    _app_handle: &AppHandle<R>,
    clients: &Arc<Mutex<HashMap<String, Client>>>,
    request: &http::Request<Vec<u8>>,
) -> Result<http::Response<Vec<u8>>, String> {
    let target = parse_target(request.uri())?;
    let forward_url = build_forward_url(&target);
    let host_key = build_origin_str(&target.host, target.port);
    let client = get_or_create_client(clients, &target.host);
    let method = request.method().clone();
    let req_headers = request.headers().clone();
    let body = request.body().clone();

    // 1. Build forwarded request with cookie injection
    let request_host = format!(
        "{}{}",
        target.host,
        target.port.map(|p| format!(":{}", p)).unwrap_or_default()
    );
    let request_path = target
        .path_and_query
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    let cookie_header = {
        let mut jars = cookie_jars().lock().unwrap();
        let jar = jars.entry(host_key.clone()).or_insert_with(PerHostCookieJar::new);
        jar.build_cookie_header(&request_host, &request_path)
    };

    let mut req_builder = match method {
        http::Method::GET => client.get(&forward_url),
        http::Method::POST => client.post(&forward_url).body(body.clone()),
        http::Method::PUT => client.put(&forward_url).body(body.clone()),
        http::Method::PATCH => client.patch(&forward_url).body(body.clone()),
        http::Method::DELETE => client.delete(&forward_url),
        http::Method::HEAD => client.head(&forward_url),
        http::Method::OPTIONS => client.request(http::Method::OPTIONS, &forward_url),
        _ => client.get(&forward_url),
    };

    let hop_by_hop = [
        "host", "connection", "keep-alive", "proxy-authenticate",
        "proxy-authorization", "te", "trailers", "transfer-encoding", "upgrade",
    ];

    // Forward browser headers, replacing cookie with our jar value; strip origin/referer/& compat
    // because they are added explicitly below, and Sec-Fetch-* to avoid upstream namespace pollution.
    let strip_filter = [
        "origin", "referer",
        "pragma", "cache-control",
        "sec-fetch-site", "sec-fetch-mode", "sec-fetch-dest",
    ];

    for (key, value) in &req_headers {
        let key_lower = key.as_str().to_lowercase();
        if !hop_by_hop.contains(&key_lower.as_str()) && key_lower != "cookie"
            && !strip_filter.contains(&key_lower.as_str())
        {
            if let Ok(val_str) = value.to_str() {
                req_builder = req_builder.header(key.as_str(), val_str);
            }
        }
    }
    if let Some(ref cookies) = cookie_header {
        req_builder = req_builder.header("Cookie", cookies.as_str());
    }

    // Fix Origin/Referer: replace hometierproxy:// with upstream scheme for upstream
    let origin_target_str = match target.port {
        Some(p) => format!("{}:{}", target.host, p),
        None => target.host.clone(),
    };
    let upstream_scheme = lookup_upstream_scheme(&origin_target_str);
    let upstream_base = format!("{}://{}", upstream_scheme, origin_target_str);
    if let Some(origin_val) = req_headers.get("origin") {
        if let Ok(origin_str) = origin_val.to_str() {
            if origin_str.starts_with("hometierproxy://") {
                req_builder = req_builder.header("origin", &upstream_base);
            }
        }
    }
    if let Some(referer_val) = req_headers.get("referer") {
        if let Ok(referer_str) = referer_val.to_str() {
            if referer_str.starts_with("hometierproxy://") {
                // 精确去掉协议前缀，避免从第二个斜杠切出 "http://host:port//host:port" 的损坏 URL
                let rest = &referer_str["hometierproxy://".len()..];
                if let Some(slash_pos) = rest.find('/') {
                    let new_referer = format!("{}{}", upstream_base, &rest[slash_pos..]);
                    req_builder = req_builder.header("referer", new_referer);
                } else {
                    req_builder = req_builder.header("referer", &upstream_base);
                }
            }
        }
    }

    // 移动仿真模式：向上游伪装移动端 UA
    if device_mode() == "mobile" {
        req_builder = req_builder.header("user-agent", MOBILE_UA);
    }

    // 2. Send upstream
    let upstream = req_builder
        .send()
        .map_err(|e| format!("Forward request failed: {}", e))?;

    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let content_type = upstream_headers.get("content-type");
    let body_bytes = upstream
        .bytes()
        .map_err(|e| format!("Failed to read upstream body: {}", e))?
        .to_vec();

    // 3. Capture Set-Cookie into jar（并持久化到期/持久 Cookie 到 DB）
    for value in upstream_headers.get_all("set-cookie") {
        if let Ok(val) = value.to_str() {
            let mut jars = cookie_jars().lock().unwrap();
            let jar = jars.entry(host_key.clone()).or_insert_with(PerHostCookieJar::new);
            let stored = jar.add_set_cookie(val);
            drop(jars);
            if let Some(cookie) = stored {
                persist_cookie_to_db(&host_key, &cookie, now_epoch());
            }
        }
    }

    // 4. Build proxy response (strip Set-Cookie from downstream)
    let mut builder = http::Response::builder().status(status);

    for (key, value) in &upstream_headers {
        let key_lower = key.as_str().to_lowercase();
        if key_lower == "content-length"
            || key_lower == "transfer-encoding"
            || key_lower == "content-encoding"
            || key_lower == "set-cookie"
        {
            continue;
        }
        builder = builder.header(key.as_str(), value.clone());
    }

    let (body, script_hash) = if is_html_content(content_type) && looks_like_html_body(&body_bytes) {
        let rewritten = rewrite_html_body(&body_bytes, &host_key);
        let (injected, hash) =
            inject_proxy_script(rewritten, &host_key, device_mode() == "mobile");
        (injected, hash)
    } else {
        (body_bytes, String::new())
    };

    let mut response = builder
        .header("content-length", body.len().to_string())
        .header("access-control-allow-origin", "*")
        .body(body)
        .map_err(|e| format!("Failed to build response: {}", e))?;

    strip_iframe_restrictions(&mut response, &script_hash);

    Ok(response)
}

pub fn register_protocol<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    let clients: Arc<Mutex<HashMap<String, Client>>> =
        Arc::new(Mutex::new(HashMap::new()));

    builder.register_asynchronous_uri_scheme_protocol(
        "hometierproxy",
        move |ctx: UriSchemeContext<'_, R>,
              request: http::Request<Vec<u8>>,
              responder: UriSchemeResponder| {
            let app_handle = ctx.app_handle().clone();
            let clients = clients.clone();

            std::thread::spawn(move || {
                let result = handle_request(&app_handle, &clients, &request);
                match result {
                    Ok(response) => responder.respond(response),
                    Err(e) => {
                        let error_response = http::Response::builder()
                            .status(500)
                            .header("content-type", "text/plain; charset=utf-8")
                            .header("access-control-allow-origin", "*")
                            .body(e.into_bytes())
                            .unwrap();
                        responder.respond(error_response);
                    }
                }
            });
        },
    )
}
