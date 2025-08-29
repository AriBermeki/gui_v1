from typing import Callable


def create_webframe(
    handler: Callable, 
    html: str,
    event_sender:Callable,
    handle_event_result: Callable
    ):...