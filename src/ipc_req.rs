use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wry::http::Request;

use crate::RuntimeMessage;

#[derive(Debug, Serialize, Deserialize)]
pub struct SerdeRequest<T> {
    /// HTTP method of the request (e.g. `"POST"`).
    pub method: String,
    /// Target URI (e.g. `"/api/call"` or `"https://example.com"`).
    pub uri: String,
    /// HTTP version, typically `"HTTP/1.1"`.
    pub version: String,
    /// Request headers, collected into a `HashMap`.
    pub headers: HashMap<String, String>,
    /// Request body (generic type).
    pub body: T,
}

impl<T: Serialize> From<Request<T>> for SerdeRequest<T> {
    /// Converts a [`Request<T>`] into a [`SerdeRequest<T>`],
    /// extracting all relevant parts into serializable values.
    fn from(req: Request<T>) -> Self {
        let (parts, body) = req.into_parts();

        // Convert `HeaderMap` into `HashMap<String, String>`
        let headers = parts
            .headers
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect::<HashMap<_, _>>();

        SerdeRequest {
            method: parts.method.to_string(),
            uri: parts.uri.to_string(),
            version: format!("{:?}", parts.version),
            headers,
            body,
        }
    }
}

/// Creates an IPC handler for [`wry::WebViewBuilder::with_ipc_handler`].
///
/// The handler:
/// - receives incoming [`Request<String>`] objects from the WebView,
/// - converts them into [`SerdeRequest`],
/// - serializes them into JSON,
/// - calls the provided Python function with the JSON as an argument.
///
/// # Parameters
/// - `handler`: A Python callable (e.g. `def handler(msg: str): ...`)
///   that processes the incoming JSON request.
///
/// # Returns
/// A closure that can be passed directly to Wry as an IPC handler.
 pub fn handle_ipc_req(
    handler: Py<PyAny>,
    proxy: tao::event_loop::EventLoopProxy<RuntimeMessage>,
) -> impl Fn(Request<String>) + 'static {
    move |_req: Request<String>| {
        let p = proxy.clone();
        Python::with_gil(|py| {
            let req = SerdeRequest::from(_req);
            let json = serde_json::to_string_pretty(&req).unwrap();
            // match expression *without* a trailing semicolon, so it is returned
            let handler = handler.clone_ref(py);
            match handler.call1(py, (json,)) {
                Ok(res) => {
                    let proxy = p.clone();
                    println!("IPC response: {}", res);
                    if let Ok(Some(script)) = res.extract::<Option<String>>(py) {
                        println!("ipc script: {}", res);
                        let _ = proxy.send_event(RuntimeMessage::Eval(script));
                    }
                }
                Err(error) => {
                    eprintln!("Some Error: {:?}", error);
                    // Nur eine Fehlermeldung ausgeben
                }
            };
        });
    }
}





/* 

pub fn handle_ipc_req(
    handler: Py<PyAny>,
    proxy: tao::event_loop::EventLoopProxy<RuntimeMessage>,
) -> impl Fn(Request<String>) + 'static {
    move |_req: Request<String>| {
        let p = proxy.clone();
        Python::with_gil(|py| {
            let req = SerdeRequest::from(_req);
            let json = serde_json::to_string_pretty(&req).unwrap();
            // match expression *without* a trailing semicolon, so it is returned
            match crate::executpy::executer(py, handler.clone_ref(py), json) {
                Ok(res) => {
                    let proxy = p.clone();
                    let script = res.extract::<String>(py).unwrap();
                    let _ = proxy.send_event(RuntimeMessage::Eval(script));
                }
                Err(error) => {
                    eprintln!("Some Error: {:?}", error);
                    // Nur eine Fehlermeldung ausgeben
                }
            };
        });
    }
}



*/