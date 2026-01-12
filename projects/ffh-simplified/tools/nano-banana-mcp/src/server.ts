import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { generateSpriteSchema, handleGenerateSprite } from './tools/generate-sprite.js';
import { generateBatchSchema, handleGenerateBatch } from './tools/generate-batch.js';
import { listCategoriesSchema, handleListCategories } from './tools/list-categories.js';
import { previewPromptSchema, handlePreviewPrompt } from './tools/preview-prompt.js';

export function registerTools(server: McpServer): void {
  // Tool: generate_sprite
  server.tool(
    'generate_sprite',
    'Generate a single 32x32 pixel art sprite using Google Nano Banana (Gemini) image generation. Supports soldiers, buildings, terrain, icons, and effects for game development.',
    generateSpriteSchema.shape,
    async (params) => {
      const parsed = generateSpriteSchema.parse(params);
      return handleGenerateSprite(parsed);
    }
  );

  // Tool: generate_sprite_batch
  server.tool(
    'generate_sprite_batch',
    'Generate multiple 32x32 pixel art sprites in batch. Useful for generating all sprites in a category at once. Limited to 20 sprites per batch.',
    generateBatchSchema.shape,
    async (params) => {
      const parsed = generateBatchSchema.parse(params);
      return handleGenerateBatch(parsed);
    }
  );

  // Tool: list_sprite_categories
  server.tool(
    'list_sprite_categories',
    'List all sprite categories and show which sprites exist vs. which are missing. Useful for planning sprite generation.',
    listCategoriesSchema.shape,
    async (params) => {
      const parsed = listCategoriesSchema.parse(params);
      return handleListCategories(parsed);
    }
  );

  // Tool: preview_prompt
  server.tool(
    'preview_prompt',
    'Preview the AI prompt that would be used for sprite generation without calling the API. Useful for debugging and prompt engineering.',
    previewPromptSchema.shape,
    async (params) => {
      const parsed = previewPromptSchema.parse(params);
      return handlePreviewPrompt(parsed);
    }
  );
}
