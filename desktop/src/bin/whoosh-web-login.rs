//! Ephemeral official-site webview. Passwords, cookies and tokens stay inside
//! WebKit. Only activity data and signed download URLs cross the private pipe.
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{WebViewBuilder, WebViewBuilderExtDarwin, WebViewExtDarwin};

#[derive(Debug)]
enum Message {
    Command(Value),
    Response(String),
    Stop,
}

#[path = "../diagnostics.rs"]
mod diagnostics;

const INIT: &str = include_str!("../web_login_init.js");

fn allowed_navigation(url: &str) -> bool {
    // WebKit also sends CAPTCHA iframe navigations through this callback.
    if url == "about:blank" {
        return true;
    }
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && (u.host_str() == Some("event.mywhoosh.com")
                || (matches!(
                    u.host_str(),
                    Some("app.usercentrics.eu" | "app.eu.usercentrics.eu")
                ) && u.path().starts_with("/browser-sdk/")
                    && u.path().ends_with("/cross-domain-bridge.html"))
                || (matches!(
                    u.host_str(),
                    Some("www.google.com" | "www.recaptcha.net" | "www.gstatic.com")
                ) && u.path().starts_with("/recaptcha/")))
    })
}

fn script(command: &Value) -> Option<String> {
    let action = match command["op"].as_str()? {
        "activities" => {
            let page = command["page"].as_u64()?;
            if page == 0 {
                return None;
            }
            format!("const result = await store.dispatch('user/new_activities', {{type:'',page:{page},sortDate:'DESC'}}); if (!result || result.error) throw new Error(); const data = result; if (!data || !Array.isArray(data.results)) throw new Error(); reply({{type:'activities',data}});")
        }
        "download" => {
            let file = command.get("fileId")?;
            if !file.is_string() {
                return None;
            }
            format!("const result = await store.dispatch('user/downloadFile', {{fileId:{file}}}); if (!result || typeof result.data !== 'string' || !result.data.startsWith('https://')) throw new Error(); reply({{type:'download',url:result.data}});")
        }
        _ => return None,
    };
    Some(format!(
        r#"(async () => {{
      if (location.origin !== 'https://event.mywhoosh.com') return;
      const reply = value => window.ipc.postMessage(JSON.stringify(value));
      const root = document.getElementById('app');
        const store = root && root.__vue__ && root.__vue__.$store;
      try {{
        window.__WHOOSH_AUTH_REFUSED__ = false;
        if (!store) throw new Error();
        if (!store.getters['auth/loggedIn']) {{ reply({{type:'auth_required'}}); return; }}
        {action}
      }} catch (e) {{ reply({{type: window.__WHOOSH_AUTH_REFUSED__ || (store && !store.getters['auth/loggedIn']) ? 'auth_required' : 'error'}}); }}
    }})()"#
    ))
}

fn emit(value: &Value) {
    // stdout is a private pipe to the parent application, never a log file.
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{value}");
    let _ = stdout.flush();
}

fn main() -> wry::Result<()> {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let credentials: Option<Value> = if mode == "--login" {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            return Ok(());
        }
        serde_json::from_str(&line).ok()
    } else {
        None
    };
    let prefill = credentials
        .as_ref()
        .and_then(|v| Some((v["email"].as_str()?, v["password"].as_str()?)))
        .map(|(email, password)| {
            format!(
                "({})({}, {});",
                include_str!("../web_login_prefill.js"),
                serde_json::to_string(email).unwrap(),
                serde_json::to_string(password).unwrap()
            )
        })
        .unwrap_or_default();
    let restore = mode == "--restore";
    let clear = mode == "--clear";
    let event_loop = EventLoopBuilder::<Message>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_visible(!restore && !clear)
        .with_title(if std::env::var("WHOOSH_LANGUAGE").as_deref() == Ok("en") {
            "MyWhoosh sign-in — https://event.mywhoosh.com"
        } else {
            "Connexion MyWhoosh — https://event.mywhoosh.com"
        })
        .with_inner_size(tao::dpi::LogicalSize::new(1050., 800.))
        .build(&event_loop)
        .expect("Fenêtre MyWhoosh indisponible");
    if clear {
        wry::WebView::remove_data_store(b"WhooshBridge0001", move |result| {
            emit(&json!({"type": if result.is_ok() { "cleared" } else { "error" }}));
            let _ = proxy.send_event(Message::Stop);
        });
        event_loop.run(move |event, _, flow| {
            *flow = if matches!(event, Event::UserEvent(Message::Stop)) {
                ControlFlow::Exit
            } else {
                ControlFlow::Wait
            };
        });
    }
    let english = std::env::var("WHOOSH_LANGUAGE").as_deref() == Ok("en");
    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_incognito(false)
        .with_data_store_identifier(*b"WhooshBridge0001")
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled)
        .with_url("https://event.mywhoosh.com/auth/login/")
        .with_initialization_script(format!(
            "window.__WHOOSH_RESTORE__ = {restore};window.__WHOOSH_ENGLISH__ = {english};\n{INIT}\n{prefill}"
        ))
        .with_navigation_handler(|url| allowed_navigation(&url))
        .with_ipc_handler(move |request| {
            if request.uri().scheme_str() == Some("https")
                && request.uri().host() == Some("event.mywhoosh.com")
            {
                let _ = ipc_proxy.send_event(Message::Response(request.body().clone()));
            }
        })
        .build(&window)?;
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else {
                break;
            };
            let Ok(command) = serde_json::from_str(&line) else {
                break;
            };
            if proxy.send_event(Message::Command(command)).is_err() {
                return;
            }
        }
        let _ = proxy.send_event(Message::Stop);
    });
    let mut ready = false;
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::UserEvent(Message::Stop) => {
                *flow = ControlFlow::Exit;
            }
            Event::UserEvent(Message::Response(body)) => {
                if let Ok(value) = serde_json::from_str::<Value>(&body) {
                    if value["type"] == "diagnostic" {
                        if let Some(
                            code @ ("cmp_ready"
                            | "cmp_not_ready"
                            | "consent_panel_opened"
                            | "consent_panel_error"),
                        ) = value["code"].as_str()
                        {
                            diagnostics::record("mywhoosh.web.consent", code);
                        }
                        return;
                    }
                    if value["type"] == "ready" {
                        if ready {
                            return;
                        }
                        ready = true;
                        window.set_visible(false);
                    }
                    if matches!(
                        value["type"].as_str(),
                        Some("ready" | "activities" | "download" | "error" | "auth_required")
                    ) {
                        emit(&value);
                    }
                }
            }
            Event::UserEvent(Message::Command(command)) => {
                if let Some(js) = script(&command).filter(|_| ready) {
                    if webview.evaluate_script(&js).is_err() {
                        emit(&json!({"type":"error"}));
                    }
                } else {
                    emit(&json!({"type":"error"}));
                }
            }
            _ => (),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allows_official_site_captcha_and_consent_bridge_only() {
        assert!(allowed_navigation("https://event.mywhoosh.com/auth/login/"));
        assert!(allowed_navigation(
            "https://www.google.com/recaptcha/api2/anchor"
        ));
        assert!(!allowed_navigation(
            "https://event.mywhoosh.com.evil.test/auth/login/"
        ));
        assert!(!allowed_navigation("http://event.mywhoosh.com/auth/login/"));
        assert!(!allowed_navigation("https://www.google.com/search"));
    }
    #[test]
    fn permits_consent_bridge_without_opening_arbitrary_navigation() {
        assert!(allowed_navigation(
            "https://app.usercentrics.eu/browser-sdk/3.0.0/cross-domain-bridge.html"
        ));
        assert!(allowed_navigation(
            "https://app.eu.usercentrics.eu/browser-sdk/latest/cross-domain-bridge.html"
        ));
        assert!(!allowed_navigation(
            "https://app.usercentrics.eu.evil.test/browser-sdk/latest/cross-domain-bridge.html"
        ));
        assert!(!allowed_navigation(
            "https://app.usercentrics.eu/browser-sdk/latest/other.html"
        ));
        assert!(!allowed_navigation(
            "http://app.usercentrics.eu/browser-sdk/latest/cross-domain-bridge.html"
        ));
    }
    #[test]
    fn permits_only_read_actions_and_escapes_file_ids() {
        assert!(script(&json!({"op":"delete"})).is_none());
        assert!(script(&json!({"op":"activities","page":0})).is_none());
        assert!(script(&json!({"op":"download","fileId":{}})).is_none());
        let file = "\"}); bad(); //";
        let js = script(&json!({"op":"download","fileId":file})).unwrap();
        assert!(js.contains(&serde_json::to_string(file).unwrap()));
        assert!(!js.contains("password"));
    }
}
