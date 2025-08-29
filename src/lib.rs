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
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
};

/// Custom user-defined messages that can be dispatched
/// through Tao’s event loop as `UserEvent`.
///
/// Currently empty, but can be extended later to
/// support WebView ↔ Rust communication.
mod assets;
mod executpy;
mod ipc_req;

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
///
pub enum RuntimeMessage {
    Eval(String),
}

#[pyfunction]
fn create_webframe(handler: Py<PyAny>, html: String) -> PyResult<()> {
    let event_loop = EventLoopBuilder::<RuntimeMessage>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = tao::window::WindowBuilder::new()
        .with_title("PyFrame")
        .build(&event_loop)
        .map_err(|err| pyo3::exceptions::PyOSError::new_err(err.to_string()))?;

    let _webview = wry::WebViewBuilder::new()
        .with_initialization_script(assets::INITIALIZEPY_SCRIPT)
        .with_ipc_handler(ipc_req::handle_ipc_req(handler, proxy))
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
                RuntimeMessage::Eval(script) => {
                    let _ = _webview.evaluate_script(&script);
                }
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
