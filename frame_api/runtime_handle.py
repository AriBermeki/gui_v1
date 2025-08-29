import asyncio
import dataclasses
import json
import os
import struct
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Type, TypeVar, Union

from pydantic import BaseModel

T = TypeVar("T")



def make_json_safe(obj: Any) -> Any:
    """
    Convert arbitrary Python objects into JSON-serializable structures.

    Supported conversions:
    - None, str, int, float, bool → returned as-is
    - Path → string path
    - BaseModel → .model_dump()
    - Dataclasses → asdict()
    - dict/list/tuple/set → recursively converted
    - Fallback → str(obj)
    """
    if obj is None:
        return None
    if isinstance(obj, (str, int, float, bool)):
        return obj
    if isinstance(obj, Path):
        return str(obj)
    if isinstance(obj, BaseModel):
        return obj.model_dump()
    if dataclasses.is_dataclass(obj):
        return dataclasses.asdict(obj)
    if isinstance(obj, dict):
        return {str(k): make_json_safe(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple, set)):
        return [make_json_safe(v) for v in obj]
    return str(obj)  # Fallback: convert to string


def normalize_args(args: Optional[Any]) -> list:
    """
    Normalize arguments so they are always returned as a JSON-safe list.

    Examples:
    - None → []
    - Already a list → [safe(...), ...]
    - Single value → [safe(value)]
    """
    if args is None:
        return []
    if isinstance(args, list):
        return [make_json_safe(a) for a in args]
    return [make_json_safe(args)]




class ApiRequestModel(BaseModel):
    """
    Represents a request sent to the Rust event loop.
    """
    id: int
    method: str
    args: List[Any]

    def to_json_array(self) -> list:
        """
        Convert request to array format: [id, method, args].
        Ensures arguments are JSON-safe.
        """
        return [self.id, self.method, [make_json_safe(a) for a in self.args]]


class ApiResponseModel(BaseModel):
    """
    Represents a response received from the Rust event loop.
    """
    id: int
    code: int
    msg: str
    result: Any

    @classmethod
    def from_array(cls, arr: List[Any]) -> "ApiResponseModel":
        """
        Create ApiResponseModel from array: [id, code, msg, result].
        """
        if not isinstance(arr, list) or len(arr) != 4:
            raise ValueError(f"Invalid ApiResponse array: {arr}")
        return cls(id=arr[0], code=arr[1], msg=arr[2], result=arr[3])


class ApiError(Exception):
    """
    Custom exception raised for errors reported by the Rust event loop.
    """
    def __init__(self, code: int, msg: str):
        super().__init__(f"[API-{code}] {msg}")
        self.code = code
        self.msg = msg




class PendingRegistry:
    """
    Registry for tracking pending asyncio Futures tied to request IDs.

    Features:
    - Request IDs wrap around in the range 0–254 (u8).
    - Ensures cleanup of futures to prevent memory leaks.
    """

    def __init__(self, max_id: int = 255):
        self._pending: Dict[int, asyncio.Future[Any]] = {}
        self._counter: int = 0
        self._max_id = max_id

    def next_id(self) -> int:
        """
        Return the next free request ID with wrap-around.
        """
        for _ in range(self._max_id):
            self._counter = (self._counter + 1) % self._max_id
            if self._counter not in self._pending:
                return self._counter
        raise RuntimeError("No free request IDs available")

    def register(self, req_id: int, future: asyncio.Future[Any]) -> None:
        """Register a new request with its future."""
        self._pending[req_id] = future

    def pop(self, req_id: int, default: Optional[Any] = None) -> Optional[asyncio.Future[Any]]:
        """
        Remove and return a future for the given request ID.
        Returns default if not present.
        """
        return self._pending.pop(req_id, default)

    def resolve(self, req_id: int, result: Any = None, error: Optional[Exception] = None) -> None:
        """
        Resolve a pending future with either a result or an error.
        """
        future = self._pending.pop(req_id, None)
        if future and not future.done():
            if error:
                future.set_exception(error)
            else:
                future.set_result(result)

    def cancel_all(self, exc: Optional[Exception] = None) -> None:
        """
        Cancel all pending futures (used during shutdown).
        """
        for fut in self._pending.values():
            if not fut.done():
                if exc:
                    fut.set_exception(exc)
                else:
                    fut.cancel()
        self._pending.clear()




_pending = PendingRegistry()
task_queue: asyncio.Queue[Dict[str, Any]] = asyncio.Queue()



async def send_loop_event(data: list) -> Optional[list]:
    """
    Send one event to the Rust loop and wait for its response.

    Steps:
    - Connect to 127.0.0.1:RUSTADDR (default 5555)
    - Send payload as length-prefixed JSON
    - Receive length-prefixed JSON response
    - Return decoded list
    """
    port = int(os.environ.get("RUSTADDR", "5555"))
    reader, writer = await asyncio.open_connection("127.0.0.1", port)

    payload = json.dumps(data).encode("utf-8")
    writer.write(struct.pack(">I", len(payload)) + payload)
    await writer.drain()

    # Read response header (length prefix)
    header = await reader.readexactly(4)
    (length,) = struct.unpack(">I", header)
    response_bytes = await reader.readexactly(length)

    writer.close()
    await writer.wait_closed()

    return json.loads(response_bytes.decode("utf-8"))


async def handle_event_loop_response(arr: list, future: Optional[asyncio.Future] = None):
    """
    Process a response from the Rust event loop.

    - If a future is provided, resolve it directly.
    - Otherwise, resolve through the global pending registry.
    """
    resp = ApiResponseModel.from_array(arr)
    if future:
        if resp.code != 0:
            future.set_exception(ApiError(resp.code, resp.msg))
        else:
            future.set_result(resp.result)
    else:
        _pending.resolve(
            resp.id,
            error=ApiError(resp.code, resp.msg) if resp.code != 0 else None,
            result=None if resp.code != 0 else resp.result,
        )




async def gui_endless_event_loop_tasks():
    """
    Endless loop that forwards requests from the task queue to Rust.

    Each task:
    - Extracts future + data
    - Sends data with send_loop_event()
    - Resolves the response with handle_event_loop_response()
    - Handles errors and ensures futures are cleaned up
    """
    try:
        while True:
            task = await task_queue.get()
            future: asyncio.Future[Any] = task.pop("future", None)
            data = task.get("data")

            try:
                # send data to rust eventloop without tcp !!!!!!!!
                arr = await send_loop_event(data)
                if arr:
                    await handle_event_loop_response(arr, future=future)
            except Exception as e:
                if future and not future.done():
                    future.set_exception(e)
            finally:
                await asyncio.sleep(0.01)
    except asyncio.CancelledError:
        print("[INFO] gui_endless_event_loop_tasks() cancelled.")
    finally:
        print("[INFO] gui_endless_event_loop_tasks() terminated.")
        _pending.cancel_all(RuntimeError("Event loop terminated"))


# -------------------------
# Public API
# -------------------------

async def eventloop_event_register_typed(
    method: str,
    args: Optional[Any] = None,
    result_type: Union[Type[BaseModel], Callable[[Any], T]] = dict,
) -> T:
    """
    Send a typed request to the Rust event loop and wait for the response.

    Workflow:
    - Normalize args (always list, JSON-safe)
    - Build ApiRequestModel
    - Register future in PendingRegistry
    - Put request into task_queue
    - Await future result (timeout 10s)
    - Cast/validate result using result_type:
        * BaseModel → .model_validate()
        * Callable → result_type(raw_result)
        * Else raw_result
    """
    req_id = _pending.next_id()
    request = ApiRequestModel(id=req_id, method=method, args=normalize_args(args))
    future: asyncio.Future[T] = asyncio.get_event_loop().create_future()
    _pending.register(req_id, future)

    await task_queue.put({
        "data": request.to_json_array(),
        "future": future,
    })

    try:
        raw_result = await asyncio.wait_for(future, timeout=10.0)

        if isinstance(result_type, type) and issubclass(result_type, BaseModel):
            return result_type.model_validate(raw_result)
        if callable(result_type):
            return result_type(raw_result)
        return raw_result

    except Exception:
        _pending.pop(req_id)
        raise
    finally:
        _pending.pop(req_id, None) 
