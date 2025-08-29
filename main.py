from frame import create_webframe
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


def on_ipc(msg):
    print("IPC from WebView:", msg)


create_webframe(
    handler=on_ipc,
    html=html
)
