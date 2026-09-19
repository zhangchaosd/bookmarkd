use crate::{
    config::Config,
    model::{self, Bookmark, Folder, Library},
    store::Store,
    transfer::{self, Import},
};
use anyhow::{Result, bail, ensure};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use bookmarkd_auth::{
    AuthStore, Session, SqliteAuthStore, StoredCredential, digest,
    engine::{CeremonyState, Engine},
    now, random,
};
use rust_embed::RustEmbed;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
struct Assets;
struct Ceremony {
    state: CeremonyState,
    binding: String,
    expires: i64,
    grant: Option<String>,
    remember: bool,
    session: Option<String>,
    credentials: Vec<StoredCredential>,
}
pub struct App {
    pub config: Config,
    pub store: Store,
    pub auth: SqliteAuthStore,
    engine: Engine,
    ceremonies: Mutex<HashMap<String, Ceremony>>,
    grants: Mutex<HashMap<String, (String, i64)>>,
    imports: Mutex<HashMap<String, Import>>,
    rate: Mutex<HashMap<String, (i64, u32)>>,
}
impl App {
    pub fn new(config: Config) -> Result<Arc<Self>> {
        let identity = config.identity()?;
        let auth = SqliteAuthStore::open(&config.auth.store, &identity)?;
        let store = Store {
            path: config.storage.business_db.clone(),
        };
        store.read()?.validate()?;
        let engine = Engine::new(
            &identity,
            &config.auth.rp_name,
            &config.auth.user_name,
            &config.server.public_url,
        )?;
        Ok(Arc::new(Self {
            config,
            store,
            auth,
            engine,
            ceremonies: Mutex::new(HashMap::new()),
            grants: Mutex::new(HashMap::new()),
            imports: Mutex::new(HashMap::new()),
            rate: Mutex::new(HashMap::new()),
        }))
    }
}
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .fallback(handle)
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024 + 1024))
        .with_state(app)
}
fn text_header<'a>(h: &'a HeaderMap, key: &str) -> &'a str {
    h.get(key).and_then(|v| v.to_str().ok()).unwrap_or("")
}
fn cookie(h: &HeaderMap, name: &str) -> String {
    text_header(h, "cookie")
        .split(';')
        .filter_map(|s| s.trim().split_once('='))
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.to_owned())
        .unwrap_or_default()
}
fn json_response(v: Value) -> Response {
    Json(v).into_response()
}
fn fail(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"code":code,"message":message,"request_id":random()}})),
    )
        .into_response()
}
fn error(e: anyhow::Error) -> Response {
    let msg = e.to_string();
    if msg.starts_with("VERSION_CONFLICT:") {
        let details = serde_json::from_str::<Value>(msg.trim_start_matches("VERSION_CONFLICT:"))
            .unwrap_or(Value::Null);
        return (StatusCode::CONFLICT,Json(json!({"error":{"code":"VERSION_CONFLICT","message":"数据已在其他设备修改；请保留输入并重新加载。","details":details,"request_id":random()}}))).into_response();
    }
    if e.downcast_ref::<rusqlite::Error>().is_some() {
        return fail(
            StatusCode::SERVICE_UNAVAILABLE,
            "STORAGE_UNAVAILABLE",
            "存储暂不可用",
        );
    }
    let (s, c) = match msg.as_str() {
        "UNAUTHENTICATED" => (StatusCode::UNAUTHORIZED, "UNAUTHENTICATED"),
        "FORBIDDEN" => (StatusCode::FORBIDDEN, "FORBIDDEN"),
        "RECENT_AUTH_REQUIRED" => (StatusCode::FORBIDDEN, "RECENT_AUTH_REQUIRED"),
        "NOT_FOUND" => (StatusCode::NOT_FOUND, "NOT_FOUND"),
        "RATE_LIMITED" => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
        "IDEMPOTENCY_CONFLICT" => (StatusCode::CONFLICT, "IDEMPOTENCY_CONFLICT"),
        _ => (StatusCode::BAD_REQUEST, "INVALID_REQUEST"),
    };
    fail(s, c, &msg)
}
async fn handle(
    State(app): State<Arc<App>>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let result = tokio::task::spawn_blocking(move || {
        dispatch(&app, &method, &uri, &headers, &body, &peer.ip().to_string())
    })
    .await;
    let mut response = match result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => error(e),
        Err(_) => fail(
            StatusCode::SERVICE_UNAVAILABLE,
            "UNAVAILABLE",
            "服务暂不可用",
        ),
    };
    for (k, v) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("x-frame-options", "DENY"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
    ] {
        response.headers_mut().insert(
            header::HeaderName::from_static(k),
            header::HeaderValue::from_static(v),
        );
    }
    response
}
fn set_cookie(app: &App, response: &mut Response, name: &str, value: &str, age: Option<i64>) {
    let mut c = format!("{name}={value}; Path=/; HttpOnly; SameSite=Strict");
    if app.config.server.public_url.starts_with("https:") {
        c.push_str("; Secure");
    }
    if let Some(age) = age {
        c.push_str(&format!("; Max-Age={age}"));
    }
    response
        .headers_mut()
        .append(header::SET_COOKIE, c.parse().unwrap());
}
fn session(app: &App, h: &HeaderMap) -> Result<Option<Session>> {
    app.auth.session(
        &cookie(h, &app.config.cookie_name()),
        &app.config.auth.app_id,
        &app.config.server.public_url,
    )
}
fn recent(s: &Session) -> Result<()> {
    ensure!(s.recent_at + 300 > now(), "RECENT_AUTH_REQUIRED");
    Ok(())
}
fn dispatch(
    app: &App,
    method: &Method,
    uri: &Uri,
    h: &HeaderMap,
    body: &[u8],
    peer: &str,
) -> Result<Response> {
    let path = uri.path();
    let cfg = &app.config;
    let origin = &cfg.server.public_url;
    let public = url::Url::parse(origin)?;
    let authority = public[url::Position::BeforeHost..url::Position::AfterPort].to_string();
    ensure!(text_header(h, "host") == authority, "FORBIDDEN");
    if path == "/healthz" && *method == Method::GET {
        return Ok(json_response(json!({"status":"ok"})));
    }
    let write = !matches!(*method, Method::GET | Method::HEAD);
    if write {
        ensure!(text_header(h, "origin") == origin, "FORBIDDEN");
        ensure!(
            text_header(h, "content-type").starts_with("application/json"),
            "FORBIDDEN"
        );
    }
    if path.starts_with("/auth/") {
        ensure!(body.len() <= 256 * 1024, "请求超过 256 KiB");
        return auth_route(app, method, path, h, body, peer);
    }
    if path == "/api/v1/session" && *method == Method::GET {
        return Ok(json_response(match session(app, h)? {
            Some(s) => {
                json!({"authenticated":true,"csrf_token":s.csrf,"session":{"id":s.id,"expires_at":s.expires_at,"recent_at":s.recent_at}})
            }
            None => json!({"authenticated":false}),
        }));
    }
    if path.starts_with("/api/") || path == "/logout" {
        let s = session(app, h)?.ok_or_else(|| anyhow::anyhow!("UNAUTHENTICATED"))?;
        if write {
            ensure!(
                bookmarkd_auth::secure_eq(text_header(h, "x-csrf-token"), &s.csrf),
                "FORBIDDEN"
            );
        }
        if !path.starts_with("/api/v1/imports") {
            ensure!(body.len() <= 1024 * 1024, "请求超过 1 MiB");
        }
        let v = if body.is_empty() {
            json!({})
        } else {
            serde_json::from_slice(body)?
        };
        if path == "/logout" && *method == Method::POST {
            app.auth.revoke_session(&s.id, &cfg.auth.app_id, origin)?;
            let mut r = json_response(json!({"ok":true}));
            set_cookie(app, &mut r, &cfg.cookie_name(), "", Some(0));
            return Ok(r);
        }
        return api(app, method, path, uri, h, v, &s);
    }
    ensure!(
        *method == Method::GET || *method == Method::HEAD,
        "NOT_FOUND"
    );
    if path == "/setup" {
        ensure!(
            app.auth
                .setup_available(&cfg.auth.app_id, origin, &cfg.auth.setup_mode)?,
            "NOT_FOUND"
        );
    }
    let asset = if path == "/"
        || [
            "/login",
            "/setup",
            "/capture",
            "/library",
            "/trash",
            "/settings",
            "/settings/security",
        ]
        .contains(&path)
    {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };
    let file = Assets::get(asset).ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
    let mut r = file.data.into_owned().into_response();
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        mime_guess::from_path(asset)
            .first_or_octet_stream()
            .to_string()
            .parse()?,
    );
    Ok(r)
}
fn auth_route(
    app: &App,
    method: &Method,
    path: &str,
    h: &HeaderMap,
    body: &[u8],
    peer: &str,
) -> Result<Response> {
    ensure!(*method == Method::POST, "NOT_FOUND");
    let v: Value = serde_json::from_slice(body)?;
    let cfg = &app.config;
    let origin = &cfg.server.public_url;
    let appid = &cfg.auth.app_id;
    let binding_name = format!("{}_preauth", cfg.cookie_name());
    let old_binding = cookie(h, &binding_name);
    let binding = if old_binding.is_empty() {
        random()
    } else {
        old_binding
    };
    {
        let mut rate = app.rate.lock().unwrap();
        rate.retain(|_, (t, _)| *t + 60 > now());
        for (key, max) in [
            ("global".to_owned(), 200),
            (
                format!("peer:{path}:{peer}"),
                if path == "/auth/setup/verify" { 5 } else { 20 },
            ),
            (
                format!("{path}:{binding}"),
                if path == "/auth/setup/verify" { 5 } else { 20 },
            ),
        ] {
            let e = rate.entry(key).or_insert((now(), 0));
            ensure!(e.1 < max, "RATE_LIMITED");
            e.1 += 1;
        }
    }
    let reauth = path.starts_with("/auth/reauth/");
    let s = if reauth {
        let s = session(app, h)?.ok_or_else(|| anyhow::anyhow!("UNAUTHENTICATED"))?;
        ensure!(
            bookmarkd_auth::secure_eq(text_header(h, "x-csrf-token"), &s.csrf),
            "FORBIDDEN"
        );
        Some(s)
    } else {
        None
    };
    if path == "/auth/setup/verify" {
        let result = app.auth.bind_token(
            v["token"].as_str().unwrap_or(""),
            &binding,
            appid,
            origin,
            &cfg.auth.setup_mode,
        );
        let grant = result.map_err(|_| anyhow::anyhow!("FORBIDDEN"))?;
        let mut grants = app.grants.lock().unwrap();
        grants.retain(|_, (_, t)| *t > now());
        ensure!(grants.len() < 1000, "RATE_LIMITED");
        grants.insert(binding.clone(), (grant, now() + 600));
        let mut r = json_response(json!({"ok":true}));
        set_cookie(app, &mut r, &binding_name, &binding, Some(600));
        return Ok(r);
    }
    if path.ends_with("/begin") {
        ensure!(
            [
                "/auth/register/begin",
                "/auth/login/begin",
                "/auth/reauth/begin"
            ]
            .contains(&path),
            "NOT_FOUND"
        );
        let credentials = app.auth.credentials()?;
        let mut grant = None;
        let (options, state) = if path == "/auth/register/begin" {
            let g = app
                .grants
                .lock()
                .unwrap()
                .get(&binding)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("FORBIDDEN"))?
                .0;
            app.auth
                .grant(&g, &binding, appid, origin, &cfg.auth.setup_mode)
                .map_err(|_| anyhow::anyhow!("FORBIDDEN"))?;
            grant = Some(g);
            app.engine.register_begin(&credentials)?
        } else {
            app.engine
                .login_begin(&credentials)
                .map_err(|_| anyhow::anyhow!("无法开始验证，请通过管理终端检查初始化状态"))?
        };
        let id = random();
        let mut ceremonies = app.ceremonies.lock().unwrap();
        ceremonies.retain(|_, c| c.expires > now());
        ensure!(
            ceremonies.len() < 1000
                && ceremonies.values().filter(|c| c.binding == binding).count() < 5,
            "RATE_LIMITED"
        );
        ceremonies.insert(
            id.clone(),
            Ceremony {
                state,
                binding: binding.clone(),
                expires: now() + 300,
                grant,
                remember: v["remember_30d"].as_bool().unwrap_or(false),
                session: s.map(|s| s.id),
                credentials,
            },
        );
        let mut o = options;
        o["ceremony_id"] = json!(id);
        o["expires_at"] = json!(now() + 300);
        let mut r = json_response(o);
        set_cookie(app, &mut r, &binding_name, &binding, Some(600));
        return Ok(r);
    }
    ensure!(
        [
            "/auth/register/finish",
            "/auth/login/finish",
            "/auth/reauth/finish"
        ]
        .contains(&path),
        "NOT_FOUND"
    );
    let ceremony = app
        .ceremonies
        .lock()
        .unwrap()
        .remove(v["ceremony_id"].as_str().unwrap_or(""))
        .ok_or_else(|| anyhow::anyhow!("FORBIDDEN"))?;
    ensure!(
        ceremony.expires > now()
            && bookmarkd_auth::secure_eq(&ceremony.binding, &binding)
            && ceremony.session == s.as_ref().map(|s| s.id.clone()),
        "FORBIDDEN"
    );
    let mut r = json_response(json!({"ok":true}));
    if path == "/auth/register/finish" {
        let g = ceremony.grant.ok_or_else(|| anyhow::anyhow!("FORBIDDEN"))?;
        let credential = app
            .engine
            .register_finish(v["credential"].clone(), ceremony.state)
            .map_err(|_| anyhow::anyhow!("FORBIDDEN"))?;
        app.auth
            .finish_registration(
                &g,
                &binding,
                appid,
                origin,
                &cfg.auth.setup_mode,
                &credential,
            )
            .map_err(|_| anyhow::anyhow!("FORBIDDEN"))?;
        app.grants.lock().unwrap().remove(&binding);
    } else {
        ensure!(ceremony.grant.is_none(), "FORBIDDEN");
        let c = app
            .engine
            .login_finish(
                v["credential"].clone(),
                ceremony.state,
                &ceremony.credentials,
            )
            .map_err(|_| anyhow::anyhow!("FORBIDDEN"))?;
        if reauth {
            app.auth
                .reauthenticate(&cookie(h, &cfg.cookie_name()), appid, origin, &c)?;
        } else {
            let (token, _) = app
                .auth
                .finish_login(&c, appid, origin, ceremony.remember)
                .map_err(|_| anyhow::anyhow!("FORBIDDEN"))?;
            set_cookie(
                app,
                &mut r,
                &cfg.cookie_name(),
                &token,
                if ceremony.remember {
                    Some(2592000)
                } else {
                    None
                },
            );
        }
    }
    Ok(r)
}
fn check_version(actual: i64, v: &Value, current: &impl serde::Serialize) -> Result<()> {
    if v["version"].as_i64() != Some(actual) {
        bail!("VERSION_CONFLICT:{}", serde_json::to_string(current)?);
    }
    Ok(())
}
fn api(
    app: &App,
    method: &Method,
    path: &str,
    uri: &Uri,
    h: &HeaderMap,
    v: Value,
    s: &Session,
) -> Result<Response> {
    let route = path.trim_start_matches("/api/v1/");
    let parts: Vec<&str> = route.split('/').collect();
    let cfg = &app.config;
    let origin = &cfg.server.public_url;
    let appid = &cfg.auth.app_id;
    if route.starts_with("auth/") {
        if *method == Method::GET {
            return Ok(json_response(match route{"auth/credentials"=>json!(app.auth.credentials()?.iter().map(|c|json!({"id":c.id,"name":c.name})).collect::<Vec<_>>()),"auth/sessions"=>json!(app.auth.sessions(appid,origin)?.iter().map(|x|json!({"id":x.id,"created_at":x.created_at,"expires_at":x.expires_at,"current":x.id==s.id})).collect::<Vec<_>>()),_=>bail!("NOT_FOUND")}));
        }
        recent(s)?;
        match (method.as_str(), parts.as_slice()) {
            ("PATCH", ["auth", "credentials", id]) => {
                app.auth.rename(id, v["name"].as_str().unwrap_or(""))?
            }
            ("DELETE", ["auth", "sessions", id]) => app.auth.revoke_session(id, appid, origin)?,
            ("POST", ["auth", "sessions", "revoke-others"]) => {
                for x in app.auth.sessions(appid, origin)? {
                    if x.id != s.id {
                        app.auth.revoke_session(&x.id, appid, origin)?;
                    }
                }
            }
            _ => bail!("NOT_FOUND"),
        };
        return Ok(json_response(json!({"ok":true})));
    }
    if route == "exports" && *method == Method::POST {
        recent(s)?;
        let mut l = app.store.read()?;
        l.exported_at = now();
        l.includes_trash = v["includes_trash"].as_bool().unwrap_or(false);
        if !l.includes_trash {
            l.bookmarks.retain(|b| b.deleted_at.is_none());
            l.folders.retain(|f| f.deleted_at.is_none());
        }
        let html = v["format"] == "html";
        let data = if html {
            transfer::html(&l)
        } else {
            serde_json::to_string_pretty(&l)?
        };
        let mut r = data.into_response();
        r.headers_mut().insert(
            header::CONTENT_TYPE,
            if html {
                "text/html; charset=utf-8"
            } else {
                "application/json"
            }
            .parse()?,
        );
        r.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"bookmarkd.{}\"",
                if html { "html" } else { "json" }
            )
            .parse()?,
        );
        return Ok(r);
    }
    if parts.first() == Some(&"imports") {
        let mut jobs = app.imports.lock().unwrap();
        jobs.retain(|_, j| j.expires > now());
        match (method.as_str(), parts.as_slice()) {
            ("POST", ["imports"]) => {
                ensure!(jobs.len() < 5, "RATE_LIMITED");
                let content = v["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("缺少文件内容"))?;
                let (library, warnings) =
                    transfer::parse(content, v["format"].as_str().unwrap_or("html"))?;
                let i = Import {
                    library,
                    warnings,
                    expires: now() + 3600,
                    digest: digest("import", content),
                };
                let mut p = transfer::preview(&i);
                let id = model::id();
                p["id"] = json!(id);
                jobs.insert(id, i);
                return Ok(json_response(p));
            }
            ("GET", ["imports", id]) => {
                return Ok(json_response(transfer::preview(
                    jobs.get(*id).ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?,
                )));
            }
            ("DELETE", ["imports", id]) => {
                jobs.remove(*id);
                return Ok(json_response(json!({"ok":true})));
            }
            ("POST", ["imports", id, "commit"]) => {
                let job = jobs
                    .get(*id)
                    .ok_or_else(|| anyhow::anyhow!("导入预览已过期"))?;
                let key = format!("import:{id}");
                let hash = digest("request", &v.to_string());
                let target = v["folder_id"].as_str().map(String::from);
                let out = app.store.mutate(Some(&key), &hash, |l| {
                    if v["mode"] == "restore" {
                        ensure!(
                            l.bookmarks.is_empty() && l.folders.is_empty(),
                            "完整恢复需要空收藏库"
                        );
                        *l = job.library.clone();
                        Ok(json!({"added":l.bookmarks.len(),"skipped":0}))
                    } else {
                        l.import(
                            &job.library,
                            target,
                            v["keep_duplicates"].as_bool().unwrap_or(false),
                        )
                    }
                })?;
                return Ok(json_response(out));
            }
            _ => bail!("NOT_FOUND"),
        }
    }
    if *method == Method::GET {
        let mut l = app.store.read()?;
        l.folders.sort_by_key(|f| (f.position, f.id.clone()));
        let q: HashMap<String, String> =
            url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
                .into_owned()
                .collect();
        let value = match parts.as_slice() {
            ["library", "revision"] => json!({"revision":l.revision}),
            ["preferences"] => l.preferences,
            ["folders"] => json!(
                l.folders
                    .iter()
                    .filter(|f| f.deleted_at.is_none())
                    .collect::<Vec<_>>()
            ),
            ["tags"] => {
                let mut tags: std::collections::BTreeSet<String> = l.tags.iter().cloned().collect();
                for b in &l.bookmarks {
                    if b.deleted_at.is_none() {
                        tags.extend(b.tags.clone());
                    }
                }
                json!(tags)
            }
            ["trash"] => {
                json!({"bookmarks":l.bookmarks.iter().filter(|b|b.deleted_at.is_some()).collect::<Vec<_>>(),"folders":l.folders.iter().filter(|f|f.deleted_at.is_some()).collect::<Vec<_>>()})
            }
            ["bookmarks", id] => json!(
                l.bookmarks
                    .iter()
                    .find(|b| b.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?
            ),
            ["bookmarks"] => {
                let search = q.get("q").map(String::as_str).unwrap_or("");
                let mut entries: Vec<_> = l
                    .bookmarks
                    .iter()
                    .filter(|b| {
                        b.deleted_at.is_none()
                            && b.matches(search)
                            && q.get("folder_id")
                                .is_none_or(|f| b.folder_id.as_deref().unwrap_or("") == f)
                            && q.get("tag").is_none_or(|t| b.tags.contains(t))
                            && (q.get("pinned").is_none_or(|v| v != "true" || b.pinned))
                    })
                    .collect();
                entries.sort_by_key(|b| {
                    (
                        b.score(search),
                        !b.pinned,
                        if b.pinned {
                            b.pinned_position
                        } else {
                            b.position
                        },
                        std::cmp::Reverse(b.created_at),
                        &b.id,
                    )
                });
                let total = entries.len();
                let offset = q
                    .get("offset")
                    .and_then(|x| x.parse::<usize>().ok())
                    .unwrap_or(0);
                let limit = q
                    .get("limit")
                    .and_then(|x| x.parse::<usize>().ok())
                    .unwrap_or(100)
                    .clamp(1, 500);
                json!({"items":entries.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),"total":total,"revision":l.revision})
            }
            _ => bail!("NOT_FOUND"),
        };
        return Ok(json_response(value));
    }
    if route == "trash/purge" {
        recent(s)?;
        ensure!(v["confirm"] == true, "需要确认永久删除");
    }
    let key = h.get("idempotency-key").and_then(|h| h.to_str().ok());
    let hash = digest("request", &format!("{method}:{path}:{}", v));
    let result = app
        .store
        .mutate(key, &hash, |l| mutate(l, method, &parts, &v))?;
    Ok(json_response(result))
}
fn mutate(l: &mut Library, method: &Method, p: &[&str], v: &Value) -> Result<Value> {
    match (method.as_str(), p) {
        ("POST", ["tags"]) => {
            let name = v["name"].as_str().unwrap_or("").trim();
            model::name(name)?;
            ensure!(
                !l.tags
                    .iter()
                    .any(|t| model::normalize(t) == model::normalize(name)),
                "标签已存在"
            );
            l.tags.push(name.into());
            Ok(json!({"name": name, "revision": l.revision+1}))
        }

        ("POST", ["bookmarks"]) => {
            let mut b = Bookmark::from_input(v)?;
            let mut seen = std::collections::HashSet::new();
            b.tags.retain(|t| seen.insert(model::normalize(t)));
            l.check_folder(&b.folder_id, false)?;
            let result = json!(b);
            l.bookmarks.push(b);
            Ok(result)
        }
        ("POST", ["bookmarks", "batch"]) => {
            let items = v["items"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("缺少选择"))?;
            ensure!(
                !items.is_empty() && items.len() <= 500,
                "批量数量应为 1–500"
            );
            let mut seen = std::collections::HashSet::new();
            for item in items {
                let id = item["id"].as_str().unwrap_or("");
                ensure!(seen.insert(id), "重复选择");
                let b = l
                    .bookmarks
                    .iter_mut()
                    .find(|b| b.id == id)
                    .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
                check_version(b.version, item, b)?;
                match v["action"].as_str().unwrap_or("") {
                    "delete" => {
                        b.deleted_at = Some(now());
                        b.deletion_batch = Some(model::id());
                    }
                    "move" => b.folder_id = v["folder_id"].as_str().map(String::from),
                    "add-tags" | "remove-tags" => {
                        let tags: Vec<String> = serde_json::from_value(v["tags"].clone())?;
                        if v["action"] == "add-tags" {
                            for t in tags {
                                if !b
                                    .tags
                                    .iter()
                                    .any(|x| model::normalize(x) == model::normalize(&t))
                                {
                                    b.tags.push(t);
                                }
                            }
                        } else {
                            b.tags.retain(|x| {
                                !tags
                                    .iter()
                                    .any(|t| model::normalize(x) == model::normalize(t))
                            });
                        }
                    }
                    _ => bail!("未知批量操作"),
                };
                b.version += 1;
                b.updated_at = now();
            }
            Ok(json!({"updated":items.len()}))
        }
        ("POST", ["bookmarks", "reorder"]) | ("POST", ["folders", "reorder"]) => {
            let items = v["items"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("缺少顺序"))?;
            ensure!(items.len() <= 500, "最多 500 项");
            let mut ids = std::collections::HashSet::new();
            for (position, item) in items.iter().enumerate() {
                let id = item["id"].as_str().unwrap_or("");
                ensure!(ids.insert(id), "重复 ID");
                if p[0] == "bookmarks" {
                    let b = l
                        .bookmarks
                        .iter_mut()
                        .find(|b| b.id == id)
                        .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
                    check_version(b.version, item, b)?;
                    if v["scope"] == "pinned" {
                        b.pinned_position = position as i64;
                    } else {
                        b.position = position as i64;
                    }
                    b.version += 1;
                } else {
                    let f = l
                        .folders
                        .iter_mut()
                        .find(|f| f.id == id)
                        .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
                    check_version(f.version, item, f)?;
                    f.position = position as i64;
                    f.version += 1;
                }
            }
            Ok(json!({"ok":true}))
        }
        ("PATCH", ["bookmarks", id])
        | ("DELETE", ["bookmarks", id])
        | ("POST", ["bookmarks", id, "restore"]) => {
            let b = l
                .bookmarks
                .iter_mut()
                .find(|b| b.id == *id)
                .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
            check_version(b.version, v, b)?;
            if *method == Method::DELETE {
                b.deleted_at = Some(now());
                b.deletion_batch = Some(model::id());
            } else if p.len() == 3 {
                b.deleted_at = None;
                b.deletion_batch = None;
                if v.get("folder_id").is_some() {
                    b.folder_id = v["folder_id"].as_str().map(String::from);
                }
            } else {
                b.patch(v)?;
            }
            b.version += 1;
            b.updated_at = now();
            Ok(json!(b))
        }
        ("POST", ["folders"]) => {
            let f = Folder {
                id: model::id(),
                name: v["name"].as_str().unwrap_or("").trim().into(),
                parent_id: v["parent_id"].as_str().map(String::from),
                position: now(),
                version: 1,
                deleted_at: None,
                deletion_batch: None,
            };
            model::name(&f.name)?;
            let result = json!(f);
            l.folders.push(f);
            Ok(result)
        }
        ("PATCH", ["folders", id]) => {
            let f = l
                .folders
                .iter_mut()
                .find(|f| f.id == *id)
                .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
            check_version(f.version, v, f)?;
            if let Some(name) = v["name"].as_str() {
                f.name = name.trim().into();
            }
            if v.get("parent_id").is_some() {
                f.parent_id = v["parent_id"].as_str().map(String::from);
            }
            f.version += 1;
            Ok(json!(f))
        }
        ("DELETE", ["folders", id]) => {
            let f = l
                .folders
                .iter()
                .find(|f| f.id == *id)
                .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
            check_version(f.version, v, f)?;
            let ids = l.descendants(id);
            let batch = model::id();
            ensure!(
                v["strategy"] == "trash" || v["strategy"] == "move",
                "选择 trash 或 move 策略"
            );
            let target = v["folder_id"].as_str().map(String::from);
            ensure!(
                !target.as_ref().is_some_and(|t| ids.contains(t)),
                "目标目录在删除范围内"
            );
            for b in &mut l.bookmarks {
                if b.deleted_at.is_none() && b.folder_id.as_ref().is_some_and(|f| ids.contains(f)) {
                    if v["strategy"] == "move" {
                        b.folder_id = target.clone();
                    } else {
                        b.deleted_at = Some(now());
                        b.deletion_batch = Some(batch.clone());
                    }
                    b.version += 1;
                }
            }
            for f in &mut l.folders {
                if ids.contains(&f.id) && f.deleted_at.is_none() {
                    f.deleted_at = Some(now());
                    f.deletion_batch = Some(batch.clone());
                    f.version += 1;
                }
            }
            Ok(json!({"ok":true}))
        }
        ("POST", ["folders", id, "restore"]) => {
            let f = l
                .folders
                .iter()
                .find(|f| f.id == *id)
                .ok_or_else(|| anyhow::anyhow!("NOT_FOUND"))?;
            check_version(f.version, v, f)?;
            let batch = f.deletion_batch.clone();
            ensure!(batch.is_some(), "目录未删除");
            for f in &mut l.folders {
                if f.deletion_batch == batch {
                    f.deleted_at = None;
                    f.deletion_batch = None;
                    f.version += 1;
                    if f.id == *id && v.get("parent_id").is_some() {
                        f.parent_id = v["parent_id"].as_str().map(String::from);
                    }
                }
            }
            for b in &mut l.bookmarks {
                if b.deletion_batch == batch {
                    b.deleted_at = None;
                    b.deletion_batch = None;
                    b.version += 1;
                }
            }
            Ok(json!({"ok":true}))
        }
        ("POST", ["trash", "purge"]) => {
            l.bookmarks.retain(|b| b.deleted_at.is_none());
            l.folders.retain(|f| f.deleted_at.is_none());
            Ok(json!({"ok":true}))
        }
        ("PATCH", ["preferences"]) => {
            check_version(
                l.preferences["version"].as_i64().unwrap_or(1),
                v,
                &l.preferences,
            )?;
            let version = l.preferences["version"].as_i64().unwrap_or(1) + 1;
            let mut pref = v.clone();
            pref["version"] = json!(version);
            l.preferences = pref;
            Ok(l.preferences.clone())
        }
        ("PATCH", ["tags", old]) | ("DELETE", ["tags", old]) => {
            ensure!(
                v["revision"].as_i64() == Some(l.revision),
                "VERSION_CONFLICT:null"
            );
            let decoded =
                url::form_urlencoded::parse(format!("x={}", old.replace('+', "%2B")).as_bytes())
                    .next()
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
            let old = decoded.as_str();
            let new = v["name"].as_str().unwrap_or("");
            if *method == Method::PATCH {
                model::name(new)?;
            }
            l.tags.retain(|t| t != old);
            if *method == Method::PATCH
                && !l
                    .tags
                    .iter()
                    .any(|t| model::normalize(t) == model::normalize(new))
            {
                l.tags.push(new.into());
            }
            for b in &mut l.bookmarks {
                if b.tags.iter().any(|t| t == old) {
                    b.tags.retain(|t| t != old);
                    if *method == Method::PATCH
                        && !b
                            .tags
                            .iter()
                            .any(|t| model::normalize(t) == model::normalize(new))
                    {
                        b.tags.push(new.into());
                    }
                    b.version += 1;
                }
            }
            Ok(json!({"ok":true}))
        }
        _ => bail!("NOT_FOUND"),
    }
}
