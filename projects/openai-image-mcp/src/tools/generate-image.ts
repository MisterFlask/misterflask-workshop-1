import { z } from 'zod';
import type { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { generateImage } from '../api/openai-client.js';
import { buildImageResult, errorResult, imageOptionsShape, toApiOptions } from './shared.js';

export const generateImageShape = {
  prompt: z.string().min(1)
    .describe('Description of the image to create. Be specific about subject, style, composition, lighting and any text to render.'),
  moderation: z.enum(['low', 'auto']).optional()
    .describe('Content moderation strictness (API default: auto)'),
  ...imageOptionsShape,
};

const generateImageSchema = z.object(generateImageShape);

export type GenerateImageInput = z.infer<typeof generateImageSchema>;

export async function handleGenerateImage(params: GenerateImageInput): Promise<CallToolResult> {
  try {
    const options = toApiOptions(params);
    const response = await generateImage({
      ...options,
      prompt: params.prompt,
      moderation: params.moderation,
    });
    return await buildImageResult('Generated', params.prompt, options.model, params, response);
  } catch (error) {
    return errorResult('generating image', error);
  }
}
