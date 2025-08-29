use pyo3::{types::PyAnyMethods, Py, PyAny, PyResult, Python};

/// Schedule `handler` on the asyncio loop.
///
/// If called from Python in an async context, this will return
/// the created Task object. Otherwise, it raises a RuntimeError.
pub fn executer(py: Python, handler: Py<PyAny>, args: String) -> PyResult<Py<PyAny>> {
    // Import asyncio and get the running loop
    let asyncio = py.import("asyncio")?;

    let loop_obj = asyncio.call_method0("new_event_loop")?;
    asyncio.call_method1("set_event_loop", (loop_obj.clone(),))?;

    // Create a Task for the handler coroutine
    let handel = handler.call1(py, (args,))?;
    let task = loop_obj.call_method1("create_task", (handel,))?;

    // Run until complete
    let result = loop_obj.call_method1("run_until_complete", (task,))?;
    Ok(result.into())
}
