# openai-image plugin

Gives Claude Code two tools backed by OpenAI's GPT Image models, the image generation behind
ChatGPT:

- `generate_image`: text prompt → images
- `edit_image`: edit local images, or combine up to 16 of them, optionally limited by a PNG mask

Results are saved to `generated-images/` in the current project (or wherever a call says) and
returned as file paths. The default model is `gpt-image-2.5-flare`. Every call is billed to your
OpenAI API account, which is separate from a ChatGPT subscription.

## Install

```bash
claude plugin marketplace add MisterFlask/misterflask-workshop-1
claude plugin install openai-image@misterflask-workshop
```

Or inside a session: `/plugin marketplace add MisterFlask/misterflask-workshop-1`, then
`/plugin install openai-image@misterflask-workshop`. Restart Claude Code afterwards.

## API key

Either:

- set the plugin's **OpenAI API key** option: `/plugin` → openai-image → Configure options. The
  key goes to your system's secure store, not a settings file, or
- leave the option empty and export `OPENAI_API_KEY` in the shell you start `claude` from.

The option wins when both are set.

## Where it works

Installed plugins load in the terminal, the IDE extensions and the desktop app's Code tab, in any
repo. **Cloud sessions (claude.ai/code) don't install plugins.** There, a repo needs its own
`.mcp.json` entry pointing at a copy of `dist/index.js`, as this repo's root `.mcp.json` does. The
cloud environment also needs `OPENAI_API_KEY` set and `api.openai.com` allowed under Network
access.

Source and full option reference: [`projects/openai-image-mcp`](../../projects/openai-image-mcp/).
Don't edit `dist/index.js` by hand; it is built from there.
