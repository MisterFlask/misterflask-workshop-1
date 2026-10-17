# Projects

This directory contains standalone code projects. Each subdirectory is a self-contained project with its own dependencies and build process.

## Current Projects

| Project | Description | Tech Stack |
|---------|-------------|------------|
| [ffh2-rs](./ffh2-rs/) | Fall from Heaven 2 (0.41) ported to Rust | Rust, web client |
| [openai-image-mcp](./openai-image-mcp/) | MCP server for OpenAI (ChatGPT) image generation in Claude Code | TypeScript, MCP SDK, esbuild |

## Adding a New Project

1. Create a new directory under `projects/`
2. Initialize with appropriate tooling (e.g., `npm init`, `cargo init`, etc.)
3. Add a README.md explaining the project
4. Update this file with the new project entry

Each project should be independently buildable and runnable without affecting others.
