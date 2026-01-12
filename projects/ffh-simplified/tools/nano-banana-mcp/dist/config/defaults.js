import path from 'path';
import { fileURLToPath } from 'url';
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
// Default output directory relative to the MCP server location
// Goes up to tools/, then to assets/sprites/
export const DEFAULT_OUTPUT_DIR = process.env.SPRITE_OUTPUT_DIR ||
    path.resolve(__dirname, '..', '..', '..', '..', 'assets', 'sprites');
// Default model to use
export const DEFAULT_MODEL = 'gemini-2.5-flash-image';
// Default style
export const DEFAULT_STYLE = 'fantasy';
// Sprite dimensions
export const SPRITE_WIDTH = 32;
export const SPRITE_HEIGHT = 32;
