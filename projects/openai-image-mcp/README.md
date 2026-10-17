# OpenAI Image MCP

An MCP server that gives Claude Code sessions access to OpenAI's GPT Image models, the image
generation behind ChatGPT. It is registered in the repo's `.mcp.json` as `openai-image`, so any
Claude Code session started in this repo picks it up.

## Tools

| Tool | What it does |
|------|--------------|
| `generate_image` | Text prompt → one or more images |
| `edit_image` | Edit local images, or combine up to 16 reference images, optionally limited by a PNG mask |

Both tools save every result to disk and return the file paths. Claude views a result by calling
`Read` on the path. Pass `inline: true` to get the image back in the tool result as well (files
over 1 MB are skipped).

The default model is `gpt-image-2.5-flare`, OpenAI's fast everyday model. Pass
`model: "gpt-image-2.5-sunburst"` when edit precision matters more than speed.

Options shared by both tools: `model`, `size`, `quality` (`low` / `medium` / `high` / `xhigh` /
`max` / `auto`; `xhigh` and `max` are 2.5-only),
`background` (`transparent` / `opaque` / `auto`), `output_format` (`png` / `jpeg` / `webp`),
`output_compression`, `n`, `output_dir`, `filename`, `inline`. The server only sends options the
call sets, so the API applies its own per-model defaults and rejects anything the chosen model
doesn't support. The error message comes straight back to the tool result.

`generate_image` also takes `moderation` (`low` / `auto`). `edit_image` takes `images` (paths),
`mask` (path) and `input_fidelity` (values vary by model; omit for gpt-image-2).

Every call is billed to the OpenAI account behind the key. Higher quality and larger sizes cost
noticeably more, so use `low` or `medium` for drafts.

## Setup

1. **API key.** Set `OPENAI_API_KEY` in the environment Claude Code runs in. For a cloud
   session, add it in the environment's settings (cloud environment menu → Edit) and start a new
   session. Locally, export it in your shell before launching `claude`.
2. **Network access (cloud sessions).** The environment's network policy must allow
   `api.openai.com`. Under Network access, either pick a broader level or add `api.openai.com`
   to Allowed domains.
3. Start a new Claude Code session in the repo and approve the `openai-image` server when asked.
   `/mcp` shows whether it connected.

No install step is needed. `dist/index.js` is a single self-contained bundle (MCP SDK and zod
included, no native modules), so it runs on any OS with Node 20+ and no `node_modules`.

### Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `OPENAI_API_KEY` | — | Required |
| `OPENAI_IMAGE_MODEL` | `gpt-image-2.5-flare` | Model used when a call doesn't name one |
| `OPENAI_IMAGE_OUTPUT_DIR` | `<project root>/generated-images` | Where images go when a call doesn't set `output_dir` (gitignored) |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | OpenAI-compatible endpoint, e.g. a gateway |
| `NODE_USE_ENV_PROXY` | `1` (set in `.mcp.json`) | Makes Node's `fetch` honor `HTTPS_PROXY` (Node ≥ 22.21). Cloud sessions need it. Elsewhere it does nothing. |

## Development

```bash
npm install
npm run build   # typecheck, then bundle src/ into dist/index.js
```

Commit the rebuilt `dist/index.js` together with any source change, because `.mcp.json` runs the
bundle directly.

```
src/
  index.ts              # stdio entry point
  server.ts             # tool registration
  api/openai-client.ts  # /images/generations (JSON) and /images/edits (multipart)
  api/types.ts
  tools/                # generate_image, edit_image, shared options and result formatting
  utils/file-handler.ts # reading inputs, naming and saving outputs
  config/defaults.ts    # env-driven defaults
```
