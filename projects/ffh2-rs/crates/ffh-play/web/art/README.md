# ffh-play art toolkit

Original pixel art for the browser client, authored as text, and the ES modules that draw it.
The style guide and the full toolkit reference are in `docs/ART.md` (section 15, "Toolkit").

```sh
sh tools/check.sh                          # the test: sources, sizes, rules, types.json, coverage
node tools/serve.mjs                       # then open http://127.0.0.1:8737/preview/
node tools/screenshot.mjs /tmp/art         # every preview page as PNG (headless Chromium)
node tools/gen-types.mjs                   # regenerate types.json after data/xml changes
```

Sets: `terrain/`, `units/`, `cities/` (cities, improvements, resources), `ui/`. Each has its own
`manifest.txt`; add sprites to the set you own. The client imports `art.js` (and `compose.js` for a
whole map view).
