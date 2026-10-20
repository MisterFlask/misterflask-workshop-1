// The cross-origin-isolation service worker of the static site (docs/WEB.md): GitHub Pages
// cannot send headers, so this worker re-serves the site's own (same-origin) responses with
//   Cross-Origin-Opener-Policy: same-origin
//   Cross-Origin-Embedder-Policy: require-corp
//   Cross-Origin-Resource-Policy: same-origin
// which makes the page `crossOriginIsolated` (SharedArrayBuffer, Atomics.wait in the engine
// worker). It lives at the site's root (its scope is the site); web/boot.js registers it and
// reloads the page once when the page is not isolated yet. A classic script: no imports.
// Cross-origin requests are left to the browser (the site makes none).

self.addEventListener("install", () => {
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.cache === "only-if-cached" && req.mode !== "same-origin") return;
  let url;
  try {
    url = new URL(req.url);
  } catch {
    return;
  }
  if (url.origin !== self.location.origin) return;
  event.respondWith(
    fetch(req).then((r) => {
      // An opaque or error response cannot be rebuilt (nor needs the headers).
      if (r.status === 0 || r.type === "opaque" || r.type === "opaqueredirect") return r;
      const headers = new Headers(r.headers);
      headers.set("Cross-Origin-Opener-Policy", "same-origin");
      headers.set("Cross-Origin-Embedder-Policy", "require-corp");
      headers.set("Cross-Origin-Resource-Policy", "same-origin");
      const nullBody = r.status === 101 || r.status === 103 || r.status === 204 || r.status === 205 || r.status === 304;
      return new Response(nullBody ? null : r.body, { status: r.status, statusText: r.statusText, headers });
    }),
  );
});
