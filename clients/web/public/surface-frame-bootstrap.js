// Surface frame bootstrap: dynamic bundle loader.
//
// Flow:
//   1. Tell parent we are ready
//   2. Wait for {type: 'mount', bundleUrl, exportName, wrapperClass, initialProps, stylesheets}
//   3. Inject stylesheets
//   4. Dynamic import bundle, find exportName, render
//   5. Surface code can call kernel RPC via window.parent.postMessage({type:'rpc.call'})

const root = document.getElementById('root');

// Compatibility shim for browser-targeted bundles that still contain
// React/Vite-style NODE_ENV checks. Surface frames run in a sandboxed browser
// context, not Node, so provide the tiny shape those bundles expect without
// exposing any broader process API.
globalThis.process ??= {};
globalThis.process.env ??= {};
globalThis.process.env.NODE_ENV ??= 'production';

let nextRpcId = 0;
const pendingRpc = new Map();
let bridgeToken = '';
let targetOrigin = '*';
let mounted = false;

function postToHost(message) {
  window.parent.postMessage(
    bridgeToken ? { ...message, bridge_token: bridgeToken } : message,
    targetOrigin,
  );
}

window.pluroraHost = {
  async callRpc(method, params) {
    const id = String(++nextRpcId);
    return new Promise((resolve, reject) => {
      pendingRpc.set(id, { resolve, reject });
      postToHost({ type: 'rpc.call', id, method, params });
    });
  },
};

let unmountFn = null;

window.addEventListener('message', async (e) => {
  if (e.source !== window.parent) return;
  const msg = e.data;
  if (!msg || typeof msg !== 'object') return;

  if (msg.type === 'rpc.result') {
    const pending = pendingRpc.get(msg.id);
    if (!pending) return;
    if (bridgeToken && msg.bridge_token !== bridgeToken) return;
    pendingRpc.delete(msg.id);
    if (msg.error) pending.reject(new Error(`${msg.error.code}: ${msg.error.message}`));
    else pending.resolve(msg.result);
    return;
  }

  if (msg.type === 'mount') {
    if (mounted) return;
    if (typeof msg.bridge_token !== 'string' || !msg.bridge_token) return;
    bridgeToken = msg.bridge_token;
    if (typeof msg.bundleUrl !== 'string' || !isAllowedAssetUrl(msg.bundleUrl)) {
      postToHost({ type: 'mount.error', code: 'invalid_bundle_url', message: 'Surface bundle URL is not allowed' });
      return;
    }
    if (typeof msg.exportName !== 'string' || !/^[A-Za-z_$][A-Za-z0-9_$]{0,95}$/.test(msg.exportName)) {
      postToHost({ type: 'mount.error', code: 'invalid_export', message: 'Surface export name is invalid' });
      return;
    }
    try {
      targetOrigin =
        msg.initialProps && typeof msg.initialProps.targetOrigin === 'string'
          ? msg.initialProps.targetOrigin
          : '*';
      // Inject stylesheets if provided
      if (msg.stylesheets && Array.isArray(msg.stylesheets)) {
        for (const href of msg.stylesheets) {
          if (typeof href !== 'string' || !isAllowedAssetUrl(href)) {
            postToHost({ type: 'mount.error', code: 'invalid_stylesheet_url', message: 'Surface stylesheet URL is not allowed' });
            return;
          }
          const link = document.createElement('link');
          link.rel = 'stylesheet';
          link.href = href;
          document.head.appendChild(link);
        }
      }

      mounted = true;
      const mod = await import(msg.bundleUrl);
      const mounter = mod[msg.exportName];
      if (!mounter) {
        root.textContent = `Surface bundle missing export ${msg.exportName}`;
        postToHost({ type: 'mount.error', code: 'missing_export', message: 'Surface bundle missing requested export' });
        return;
      }
      if (msg.wrapperClass) {
        root.className = msg.wrapperClass;
      }
      // mounter is a (root, props) => unmountFn function
      unmountFn = mounter(root, msg.initialProps ?? {});
    } catch (err) {
      root.textContent = `Surface bundle failed to load: ${err && err.message || err}`;
      postToHost({ type: 'mount.error', code: 'bundle_load_failed', message: 'Surface bundle failed to load' });
    }
  }

  if (msg.type === 'unmount') {
    if (bridgeToken && msg.bridge_token !== bridgeToken) return;
    if (typeof unmountFn === 'function') {
      try { unmountFn(); } catch { /* noop */ }
      unmountFn = null;
    }
    root.innerHTML = '';
    bridgeToken = '';
    targetOrigin = '*';
    mounted = false;
  }
});

// Signal ready
window.parent.postMessage({ type: 'ready' }, '*');

function isAllowedAssetUrl(value) {
  try {
    const url = new URL(value, window.location.href);
    return url.origin === window.location.origin
      && (url.pathname.startsWith('/surface-assets/')
        || url.pathname.startsWith('/assets/')
        || url.pathname === '/surface-frame-bootstrap.js');
  } catch {
    return false;
  }
}
