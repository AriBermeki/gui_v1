use crossbeam_channel::{unbounded, Receiver, Sender};
use pyo3::{prelude::*, types::PyDict};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A handle that allows sending messages **from Python to Rust**.
///
/// Exposed to Python as a class. Wraps a `Sender<Py<PyAny>>`.
#[pyclass]
struct SenderHandle {
    tx: Arc<Sender<Py<PyAny>>>,
}

#[pymethods]
impl SenderHandle {
    /// Send a Python object to the Rust event loop.
    ///
    /// # Errors
    /// Returns a `PyRuntimeError` if the send operation fails.
    fn send(&self, msg: Py<PyAny>) -> PyResult<()> {
        self.tx
            .send(msg)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("send failed: {e}")))
    }
}

/// A handle that allows receiving messages **from Rust to Python**.
///
/// Exposed to Python as a class. Wraps a `Receiver<Py<PyAny>>`.
#[pyclass]
struct ReceiverHandle {
    rx: Arc<Receiver<Py<PyAny>>>,
}

#[pymethods]
impl ReceiverHandle {
    /// Attempt to receive a message without blocking.
    ///
    /// Returns `Some(PyAny)` if a message is available, otherwise `None`.
    fn recv(&self) -> Option<Py<PyAny>> {
        self.rx.try_recv().ok()
    }

    /// Wait until a message is available and return it.
    ///
    /// This method blocks the current thread until a message is received.
    fn recv_blocking(&self) -> Option<Py<PyAny>> {
        self.rx.recv().ok()
    }
}



fn spawn_py_event_loop(
    py_event_loop: Py<PyAny>,
    pyevent_to_rust_queue: Py<PyAny>,
    rust_to_py_ipc: Py<PyAny>,
    tx_from_py_to_rust: Arc<Sender<Py<PyAny>>>,
    rx_from_rust_to_py: Arc<Receiver<Py<PyAny>>>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    let handle = std::thread::spawn(move || {
        Python::with_gil(|py| -> PyResult<()> {
            println!("[PY] Python-Thread started");

            let asyncio = py.import("asyncio")?;
            let loop_obj = py_event_loop.bind(py);

            // Set the event loop
            asyncio.call_method1("set_event_loop", (loop_obj.clone(),))?;

            // Create sender and receiver handles
            let sender = Py::new(
                py,
                SenderHandle {
                    tx: tx_from_py_to_rust.clone(),
                },
            )?;
            let receiver = Py::new(
                py,
                ReceiverHandle {
                    rx: rx_from_rust_to_py.clone(),
                },
            )?;

            // Register Python tasks
            let py_events = pyevent_to_rust_queue.call1(py, (sender,))?;
            let py_ipc = rust_to_py_ipc.call1(py, (receiver,))?;

            loop_obj.call_method1("create_task", (py_events,))?;
            loop_obj.call_method1("create_task", (py_ipc,))?;

            // Run the asyncio loop forever
            loop_obj.call_method0("run_forever")?;

            Ok(())
        })
        .unwrap_or_else(|e| eprintln!("Python thread error: {:?}", e));
    });

    Ok(handle)
}
