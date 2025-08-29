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
    






async def handle_ipc_message(raw: str) -> None:
    """
    Handle an IPC message from window.ipc.postMessage (JS).
    
    raw: JSON string from JS
    send: function to send JS code back into the WebView (e.g. window.eval)
    """
    try:
        msg = json.loads(raw)
        cmd: str = msg["cmd"]
        result_id: str = msg["result_id"]
        error_id: str = msg["error_id"]
        args: list[Any] = msg.get("payload", [])

        try:
            result = await dispatch(cmd, args)
            # Call the temporary JS callback function
            js_code = f"window._{result_id}({json.dumps(result)});"
            print(js_code)
        except Exception as e:
            js_code = f"window._{error_id}({json.dumps(str(e))});"
            print(js_code)

        # send back JS code to execute inside the WebView
        # send(js_code)

    except Exception as e:
        print("IPC error:", e)


""" 

Example Usage




@command
def add(x: int, y: int) -> int:
    return x + y

@command
async def mul(x: int, y: int) -> int:
    await asyncio.sleep(0.1)
    return x * y


async def main() -> None:
    result_add: int = await dispatch("add", [2, 3])
    result_mul: int = await dispatch("mul", [2, 3])

    print(result_add)  # 5
    print(result_mul)  # 6


if __name__ == "__main__":
    create_webframe(
        html= html,
        ipc=handle_ipc
    )




"""