from frame import create_webframe

def on_ipc(msg):
    print("IPC from WebView:", msg)

html = r"""
<html>
  <head>
    <style>
      body {
        font-family: 'Segoe UI', Tahoma, sans-serif;
        background: #f0f2f5;
        margin: 0;
        padding: 2rem;
        display: flex;
        justify-content: center;
        align-items: center;
        height: 100vh;
      }
      .card {
        background: white;
        border-radius: 1rem;
        box-shadow: 0 8px 20px rgba(0,0,0,0.15);
        padding: 2rem;
        width: 350px;
        text-align: center;
      }
      h2 {
        margin-bottom: 1rem;
        color: #333;
      }
      .input-group {
        margin: 1rem 0;
        text-align: left;
      }
      label {
        display: block;
        margin-bottom: 0.3rem;
        font-size: 0.9rem;
        color: #555;
      }
      input {
        width: 100%;
        padding: 0.8rem;
        border: 1px solid #ccc;
        border-radius: 0.5rem;
        font-size: 1rem;
      }
      button {
        background: #007BFF;
        color: white;
        border: none;
        padding: 0.8rem 1.2rem;
        margin-top: 1rem;
        border-radius: 0.5rem;
        font-size: 1rem;
        cursor: pointer;
        transition: background 0.3s;
        width: 100%;
      }
      button:hover {
        background: #0056b3;
      }
      .message {
        margin-top: 1rem;
        font-size: 0.95rem;
        color: #007BFF;
        font-weight: 500;
      }
    </style>
  </head>
  <body>
    <div class="card">
      <h2>User Login</h2>
      <div class="input-group">
        <label for="username">Username</label>
        <input id="username" type="text" placeholder="Enter your username">
      </div>
      <div class="input-group">
        <label for="password">Password</label>
        <input id="password" type="password" placeholder="Enter your password">
      </div>
      <button onclick="submitForm()">Login</button>
      <div id="msg" class="message"></div>
    </div>

    <script>
      function submitForm() {
        const username = document.getElementById('username').value;
        const password = document.getElementById('password').value;
        window.ipc.postMessage({action: 'login', username, password});
        document.getElementById('msg').innerText = "Sending login request...";
      }
    </script>
  </body>
</html>
"""

create_webframe(
    handler=on_ipc,
    html=html
)
