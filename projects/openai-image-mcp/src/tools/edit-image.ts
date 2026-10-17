import { z } from 'zod';
import type { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { editImage } from '../api/openai-client.js';
import { readInputImage } from '../utils/file-handler.js';
import { buildImageResult, errorResult, imageOptionsShape, toApiOptions } from './shared.js';

export const editImageShape = {
  prompt: z.string().min(1)
    .describe('What to change or create. When several images are given, refer to them by order, e.g. "put the hat from image 2 on the knight in image 1".'),
  images: z.array(z.string()).min(1).max(16)
    .describe('Paths to input images (.png, .jpg, .jpeg or .webp), relative to the project root or absolute. Up to 16.'),
  mask: z.string().optional()
    .describe('Path to a PNG mask the same size as the first image; fully transparent pixels mark the area to edit.'),
  input_fidelity: z.enum(['high', 'low']).optional()
    .describe('How closely to preserve input details such as faces (gpt-image-1 and gpt-image-1.5 only; omit for gpt-image-2, which always uses high).'),
  ...imageOptionsShape,
};

const editImageSchema = z.object(editImageShape);

export type EditImageInput = z.infer<typeof editImageSchema>;

export async function handleEditImage(params: EditImageInput): Promise<CallToolResult> {
  try {
    const images = await Promise.all(params.images.map(readInputImage));
    const mask = params.mask ? await readInputImage(params.mask) : undefined;
    if (mask && mask.mimeType !== 'image/png') {
      throw new Error('The mask must be a PNG with an alpha channel');
    }

    const options = toApiOptions(params);
    const response = await editImage({
      ...options,
      prompt: params.prompt,
      images,
      mask,
      input_fidelity: params.input_fidelity,
    });
    return await buildImageResult('Edited', params.prompt, options.model, params, response);
  } catch (error) {
    return errorResult('editing image', error);
  }
}
