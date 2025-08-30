import asyncio
import inspect
import json
from typing import Any, Awaitable,  Dict, List, Protocol, TypeVar, Union

# Generic type
R = TypeVar("R")  # return type

# A command can be sync or async
class CommandFn(Protocol[R]):
    def __call__(self, *args: Any, **kwargs: Any) -> Union[R, Awaitable[R]]: ...


# Global command registry
COMMANDS: Dict[str, CommandFn[Any]] = {}


def ipc_command(func: CommandFn[R]) -> CommandFn[R]:
    """
    Decorator to register a command.
    Supports both sync and async functions.
    """
    name: str = func.__name__

    if name in COMMANDS:
        raise ValueError(f"Command {name!r} is already registered.")

    COMMANDS[name] = func
    return func


async def dispatch(name: str, args: List[Any]) -> Any:
    """
    Execute a command with a list of arguments, whether sync or async.
    Example: await dispatch("add", [2, 3])
    """
    if name not in COMMANDS:
        raise ValueError(f"Command {name!r} not found.")

    func = COMMANDS[name]

    if inspect.iscoroutinefunction(func):
        # async function → await directly
        return await func(*args)  # type: ignore
    else:
        # sync function → run in executor (non-blocking)
        loop = asyncio.get_running_loop()
        return await loop.run_in_executor(None, lambda: func(*args))  # type: ignore
    




async def handle_ipc_message(raw: str) -> str:
    """
    Handle an IPC message from window.ipc.postMessage (JS).
    
    raw: JSON string from JS
    Returns: JS code string (to be eval'd by Rust/Wry)
    """
    try:
        msg = json.loads(raw)
        print(f"From IPC frontend: {msg})")
        if not isinstance(msg, dict):
            raise ValueError("Invalid IPC message format (not an object)")
        body: str = json.loads(msg.get("body", ""))
        if not isinstance(body, dict):
            raise ValueError("Invalid IPC message body (not an object)")
        msg = body
        cmd: str = msg["cmd"]
        # print(f"IPC command received: {cmd}({args})")
        result_id: str = msg["result_id"]
        error_id: str = msg["error_id"]
        args: list[Any] = msg.get("payload", [])
        try:
            result = await dispatch(cmd, args)
            # Erfolgreich → Callback aufrufen
            print("ipc cmd result:", result)
            # raise NotImplementedError("currently not implemented")
            # js_code = f"window._{result_id}({json.dumps(result)});"
            js_code = f"""console.log('{cmd} executed');"""
        except Exception as e:
            # Fehler → Error-Callback
            js_code = f"""console.log('error occurred while executing {cmd}: {str(e)}');"""
            print(js_code)
            # js_code = f"window._{error_id}({json.dumps(str(e))});"

        return js_code

    except Exception as e:
        print(f"IPC handling error: {e}")
        # Top-Level Fehler → ebenfalls als String zurückgeben
        return f"""console.error("IPC error: {str(e)}");"""