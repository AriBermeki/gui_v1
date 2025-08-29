import asyncio
from frame import create_webframe
from frame_api import handle_ipc_message, ipc_command
from pathlib import Path

# Path object for current script
ROOT_SRC = Path(__file__).resolve().parent
HTML_SRC = ROOT_SRC.joinpath('index.html')

def _html():
  if not HTML_SRC.exists():
      html = """
      <h2 style="position: absolute; top: 40%; left: 50%; transform: translate(-40%, -40%);">Do check the issue, HTML file not found.</h2>
      """
  else:
      with open(HTML_SRC) as f:
          html = f.read()
  return html

html = _html()

@ipc_command
def add(x: int, y: int) -> int:
    return x + y

@ipc_command
async def mul(x: int, y: int) -> int:
    await asyncio.sleep(0.1)
    return x * y


def on_ipc(data):
    return asyncio.run(handle_ipc_message(data))

if __name__ == "__main__":
    """
    Launch the webframe with:
    - handler: the async function above to process messages from JS
    - html: the HTML template string defined earlier
    """
    create_webframe(
        handler=on_ipc,
        html=html
    )