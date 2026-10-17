import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { generateImageShape, handleGenerateImage } from './tools/generate-image.js';
import { editImageShape, handleEditImage } from './tools/edit-image.js';

export function registerTools(server: McpServer): void {
  // Tool: generate_image
  server.registerTool(
    'generate_image',
    {
      title: 'Generate image (OpenAI)',
      description: 'Create images from a text prompt with OpenAI GPT Image, the model behind ChatGPT image generation. Saves each image to disk and returns the file paths. Every call is billed to the configured OpenAI account.',
      inputSchema: generateImageShape,
      annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: true },
    },
    async (params) => handleGenerateImage(params)
  );

  // Tool: edit_image
  server.registerTool(
    'edit_image',
    {
      title: 'Edit image (OpenAI)',
      description: 'Edit local images, or combine up to 16 reference images into a new one, with OpenAI GPT Image. An optional PNG mask limits the edit to its transparent area. Saves results to disk and returns the file paths. Every call is billed to the configured OpenAI account.',
      inputSchema: editImageShape,
      annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: true },
    },
    async (params) => handleEditImage(params)
  );
}
