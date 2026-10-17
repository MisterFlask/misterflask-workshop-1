import path from 'path';

/**
 * Read an environment variable, treating empty values and unexpanded
 * `${VAR}` placeholders (left by .mcp.json when the variable is unset) as missing.
 */
export function readEnv(name: string): string | undefined {
  const value = process.env[name]?.trim();
  if (!value || value.startsWith('${')) {
    return undefined;
  }
  return value;
}

// Default model; any GPT Image model id the account can use is accepted
export const DEFAULT_MODEL = readEnv('OPENAI_IMAGE_MODEL') ?? 'gpt-image-2.5-flare';

// OpenAI-compatible API base URL
export const API_BASE_URL = (readEnv('OPENAI_BASE_URL') ?? 'https://api.openai.com/v1').replace(/\/+$/, '');

// Relative paths in tool calls resolve against the project root Claude Code passes in
export const PROJECT_DIR = readEnv('CLAUDE_PROJECT_DIR') ?? process.cwd();

// Where images are saved when a call does not name an output_dir
export const DEFAULT_OUTPUT_DIR = path.resolve(PROJECT_DIR, readEnv('OPENAI_IMAGE_OUTPUT_DIR') ?? 'generated-images');

// High-quality generations can take a couple of minutes
export const REQUEST_TIMEOUT_MS = 5 * 60 * 1000;

// Images larger than this are not returned inline even when asked for
export const MAX_INLINE_BYTES = 1024 * 1024;
