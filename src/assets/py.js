(function () {
  function uid() {
    return window.crypto.getRandomValues(new Uint32Array(1))[0].toString();
  }

  function transformCallback(callback, once = true) {
    const identifier = uid();
    const prop = `_${identifier}`;

    Object.defineProperty(window, prop, {
      value: (result) => {
        if (once) Reflect.deleteProperty(window, prop);
        if (callback) callback(result);
      },
      writable: false,
      configurable: true,
    });

    return identifier;
  }

  async function invoke(cmd, args = []) {
    return new Promise((resolve, reject) => {
      if (!window.ipc || typeof window.ipc.postMessage !== "function") {
        reject(new Error("IPC bridge is not available!"));
        return;
      }

      const result_id = transformCallback((result) => resolve(result), true);
      const error_id = transformCallback((error) => reject(error), true);

      const message = {
        cmd,
        result_id,
        error_id,
        payload: args,
      };
      console.log(message)

      window.ipc.postMessage(message);
    });
  }


  if (!window) {
    window= {};
  }

  window.invoke = invoke;
})();
