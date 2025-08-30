import asyncio
from frame import emit_str, emit_async

async def spam_emit(n):
    for i in range(n):
        emit_str(f'{{"event": "tick", "i": {i}}}')
        print(emit_async(f'{{"event": "tick", "i": {i}}}'))
        await asyncio.sleep(0.1)  # Yield to the event loop

async def background_task(n):
    i = 0
    while n != i:
        print(f"{i}. Background still running...")
        await asyncio.sleep(0.5)
        i += 1

async def main():
    await asyncio.gather(
        spam_emit(30),
        background_task(10)
    )

asyncio.run(main())