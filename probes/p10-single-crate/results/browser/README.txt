A6 --split, in a browser (the Playwright MCP's Chromium), against
`cargo leptos serve --split` of hello/ (port 3960), 2026-09-28:

1. / ?lang=fr — server-rendered in French (<html lang="fr">, title
   "Bonjour, MessageFormat 2"). Requests: the page, hello.js, hello.wasm, the
   fr catalog (/i18n/fr.f852be80bc0d97c1.mf2b) once. Console: only a 404 for
   /favicon.ico (the probe has none).
2. Click the "Revenir" link (to /visits): no document request — the router
   navigated client-side (so the page had hydrated) and fetched the lazy
   route's chunk: /pkg/__wasm_split.______________________.js and
   /pkg/split___visits_view_1606048974470344421.wasm. The route shows
   "Vous êtes venu une fois." (the plural, from the catalog, in the chunk's
   view).
3. Click the "Revenir" button once, choose English, press "Appliquer": no
   document request; one request, /i18n/en.0a810a470afa186f.mf2b; the
   title becomes "Hello, MessageFormat 2", the status "You have been here 2
   times." — the count kept, the language switched live.

The snapshots and the console log are the files beside this one.
