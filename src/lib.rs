//! # Frame Module
//!
//! This module implements a simple bridge between Rust, Wry/Tao
//! (GUI + WebView), and Python via [PyO3].  
//!
//! It enables:
//! - Rendering an HTML UI inside a native window,
//! - Handling IPC requests (`window.ipc.postMessage(...)`) from the WebView,
//! - Forwarding requests to a Python handler function,
//! - Returning or logging results from the Python side.
//!
//! ## Overview
//! - [`SerdeRequest`] serializes incoming HTTP-like requests (`wry::http::Request`).
//! - [`handle_ipc_req`] creates an IPC handler that converts requests
//!   to JSON and forwards them to Python.
//! - [`create_webframe`] creates a native window with a WebView, binds the IPC handler,
//!   and starts the Tao event loop.
//!
//! ## Example (Python)
//! ```python
//! import frame
//!
//! def on_ipc(msg: str):
//!     print("Got IPC:", msg)
//!
//! html = "<html><body><script>window.ipc.postMessage('Hello')</script></body></html>"
//! frame.create_webframe(on_ipc, html)
//! ```

use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use wry::http::Request;

/// Custom user-defined messages that can be dispatched
/// through Tao’s event loop as `UserEvent`.
///
/// Currently empty, but can be extended later to
/// support WebView ↔ Rust communication.
pub enum RuntimeMessage {}

/// A serializable representation of an incoming [`Request`].
///
/// Contains:
/// - HTTP method (`GET`, `POST`, …),
/// - URI,
/// - Version (`HTTP/1.1`, `HTTP/2`),
/// - Headers as `HashMap<String, String>`,
/// - Body of generic type `T` (e.g., `String`).
///
/// This type can be serialized to JSON using [`serde`].
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
pub fn handle_ipc_req(handler: Py<PyAny>) -> impl Fn(Request<String>) + 'static {
    move |_req: Request<String>| {
        Python::with_gil(|py| {
            let req = SerdeRequest::from(_req);
            let json = serde_json::to_string_pretty(&req).unwrap();
            let ipc_py_handler = handler.clone_ref(py);
            match ipc_py_handler.call1(py, (json,)) {
                Ok(res) => res,
                Err(error) => {
                    eprintln!("IPC Error: {:?}", error);
                    py.None()
                }
            }
        });
    }
}

/// Creates a native window with an embedded WebView.
///
/// This function:
/// - Initializes a Tao event loop,
/// - Builds a [`tao::window::Window`] titled `"PyFrame"`,
/// - Builds a [`wry::WebView`] with:
///   - provided HTML content (`html` parameter),
///   - an IPC handler that forwards messages to Python,
/// - Starts the event loop (`event_loop.run`).
///
/// # Parameters
/// - `handler`: A Python callable that receives IPC messages as JSON.
/// - `html`: The HTML string to render inside the WebView.
///
/// # Errors
/// - Returns `PyOSError` if the window cannot be created.
/// - Returns `PyRuntimeError` if WebView creation fails.
#[pyfunction]
fn create_webframe(handler: Py<PyAny>, html: String) -> PyResult<()> {
    let event_loop = EventLoopBuilder::<RuntimeMessage>::with_user_event().build();

    let window = tao::window::WindowBuilder::new()
        .with_title("PyFrame")
        .build(&event_loop)
        .map_err(|err| pyo3::exceptions::PyOSError::new_err(err.to_string()))?;

    let _webview = wry::WebViewBuilder::new()
        .with_ipc_handler(handle_ipc_req(handler))
        .with_html(&html)
        .with_devtools(true)
        .build(&window)
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;

    event_loop.run(move |event, _window_target, flow: &mut ControlFlow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                window_id, event, ..
            } => match event {
                WindowEvent::CloseRequested => {
                    println!("Close requested for window {:?}", window_id);
                    *flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::UserEvent(_user_event) => match _user_event {
                // Future RuntimeMessage events could be handled here
            },
            _ => {}
        }
    });
}

/// Python module entry point for `frame`.
///
/// Exports the [`create_webframe`] function to Python.
///
/// This module can be compiled with `maturin` or `setuptools-rust`
/// and imported into Python.
#[pymodule]
fn frame(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(create_webframe, m)?)?;
    Ok(())
}
