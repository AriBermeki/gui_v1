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
//! some test
//! ```
use once_cell::sync::Lazy;
use std::sync::{Mutex, Once};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};
use pyo3::prelude::*;
use serde_json::Value;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use pyo3_async_runtimes::tokio;

/// Custom user-defined messages that can be dispatched
/// through Tao’s event loop as `UserEvent`.
///
/// Currently empty, but can be extended later to
/// support WebView ↔ Rust communication.
mod assets;
mod executpy;
mod ipc_req;


static EVENT_QUEUE: Lazy<Mutex<(UnboundedSender<Value>, Option<UnboundedReceiver<Value>>)>> = Lazy::new(|| {
        let (tx, rx) = unbounded_channel();
        Mutex::new((tx, Some(rx)))
    });



fn start_event_consumer() -> PyResult<()> {
    START_CONSUMER_ONCE.call_once(|| {
        let rx = {
            let mut guard = EVENT_QUEUE.lock().unwrap();
            guard.1.take()
        };

        if let Some(mut rx) = rx {
            if let Ok(rt) = tokio::get_runtime() {
                rt.spawn(async move {
                    while let Some(event) = rx.recv().await {
                        println!("[Rust] Received event: {:?}", event);
                        // Handle event here
                    }
                });
            } else {
                eprintln!("Failed to get Python asyncio runtime");
            }
        }
    });

    Ok(())
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
                    println!("Evaluating script: {}", script);
                    let result = _webview.evaluate_script(&script);
                    // Handle result or error if needed
                    match result {Ok(r) => {
                        // print the value of result, if its ok.
                        println!("Script evaluated successfully: {:?}", r);
                    }
                        Err(e) => eprintln!("Error evaluating script: {:?}", e),
                    }
                }
            },
            _ => {}
        }
    });

    println!("Event loop exited");
    Ok(())
}


static START_CONSUMER_ONCE: Once = Once::new();
// fn start_event_consumer() {
//     START_CONSUMER_ONCE.call_once(|| {
//         let rx = {
//             let mut guard = EVENT_QUEUE.lock().unwrap();
//             std::mem::replace(&mut guard.1, Some(unbounded_channel().1)) // take the original receiver
//         };

//         tokio::spawn(async move {
//             if let Some(mut rx) = rx {
//                 while let Some(event) = rx.recv().await {
//                     println!("[Rust] Received event: {:?}", event);
//                     // Handle your event here
//                 }
//             }
//         });
//     });
// }





#[pyfunction]
fn emit_str(json: &str) -> PyResult<()> {
    
    // Python::with_gil(|py| {
    let parsed: Value = serde_json::from_str(json).map_err(|err| {
        pyo3::exceptions::PyValueError::new_err(format!("Invalid JSON: {}", err))
    })?;
    
    println!("Event received from python: {}", parsed);
    // Try sending to the event queue
    let queue = EVENT_QUEUE.lock().unwrap();
        queue.0.send(parsed).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("Queue send failed: {}", e))
    })?;
    
    start_event_consumer();
    Ok(())
}


#[pyfunction]
fn emit_async<'a>(py: Python<'a>, json: &'a str) -> PyResult<pyo3::Bound<'a, pyo3::PyAny>> {
    let json = json.to_string(); // clone for async move

    pyo3_async_runtimes::tokio::future_into_py(py, async move {
        let parsed: Value = serde_json::from_str(&json)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        // println!("Async event: {}", parsed);
        
        // Try sending to the event queue
        let queue = EVENT_QUEUE.lock().unwrap();
        queue.0.send(parsed).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("Queue send failed: {}", e))
        })?;

        // Return None or awaitable resolved immediately
        Python::with_gil(|py| Ok(py.None()))
    })
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
    m.add_function(wrap_pyfunction!(emit_str, m)?)?;
    m.add_function(wrap_pyfunction!(emit_async, m)?)?;
    // m.add_function(wrap_pyfunction!(start_event_loop, m)?)?;
    Ok(())
}
