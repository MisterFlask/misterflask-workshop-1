import { z } from 'zod';
import type { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import type { ImageOptions, ImageResponse } from '../api/types.js';
import { ApiKeyError, NetworkError, OpenAIApiError } from '../api/openai-client.js';
import { saveImages } from '../utils/file-handler.js';
import { DEFAULT_MODEL, DEFAULT_OUTPUT_DIR, MAX_INLINE_BYTES } from '../config/defaults.js';

// Options accepted by both generate_image and edit_image
export const imageOptionsShape = {
  model: z.string().optional()
    .describe(`GPT Image model id (default: ${DEFAULT_MODEL}). Others include gpt-image-1.5 and gpt-image-1-mini.`),
  size: z.string().optional()
    .describe('"auto" or WIDTHxHEIGHT, e.g. "1024x1024", "1536x1024" (landscape), "1024x1536" (portrait). gpt-image-2 also accepts custom sizes: both edges multiples of 16, longest edge at most 3840, aspect ratio at most 3:1.'),
  quality: z.enum(['low', 'medium', 'high', 'auto']).optional()
    .describe('Rendering quality. Cost and latency rise steeply with quality; use low or medium for drafts.'),
  background: z.enum(['transparent', 'opaque', 'auto']).optional()
    .describe('Background handling. "transparent" needs png or webp output and a model that supports it.'),
  output_format: z.enum(['png', 'jpeg', 'webp']).optional()
    .describe('File format of the result (API default: png)'),
  output_compression: z.number().int().min(0).max(100).optional()
    .describe('Compression level 0-100 for jpeg or webp output'),
  n: z.number().int().min(1).max(10).optional()
    .describe('Number of images to generate (default 1). Each one is billed.'),
  output_dir: z.string().optional()
    .describe(`Directory to save images in, absolute or relative to the project root (default: ${DEFAULT_OUTPUT_DIR})`),
  filename: z.string().optional()
    .describe('Base filename without extension. Overwrites an existing file of that name; with n > 1, "-1", "-2", ... are appended. Default: timestamp plus a slug of the prompt.'),
  inline: z.boolean().optional()
    .describe('Also return each image inline in the tool result (skipped for files over 1 MB). Default false: view saved files with the Read tool instead.'),
};

export interface ImageOptionsInput {
  model?: string;
  size?: string;
  quality?: ImageOptions['quality'];
  background?: ImageOptions['background'];
  output_format?: ImageOptions['output_format'];
  output_compression?: number;
  n?: number;
  output_dir?: string;
  filename?: string;
  inline?: boolean;
}

/**
 * Pick the fields that go to the API, leaving out local-only options
 */
export function toApiOptions(params: ImageOptionsInput): ImageOptions {
  return {
    model: params.model ?? DEFAULT_MODEL,
    size: params.size,
    quality: params.quality,
    background: params.background,
    output_format: params.output_format,
    output_compression: params.output_compression,
    n: params.n,
  };
}

/**
 * Save the API response to disk and describe it for the caller
 */
export async function buildImageResult(
  verb: string,
  prompt: string,
  model: string,
  params: ImageOptionsInput,
  response: ImageResponse
): Promise<CallToolResult> {
  const saved = await saveImages(response.images, {
    outputDir: params.output_dir,
    filename: params.filename,
    prompt,
    fallbackFormat: response.outputFormat ?? params.output_format,
  });

  const details = [response.size, response.quality, response.background && `${response.background} background`]
    .filter(Boolean)
    .join(', ');
  const noun = saved.length === 1 ? 'image' : 'images';
  let text = `${verb} ${saved.length} ${noun} with ${model}${details ? ` (${details})` : ''}:\n`;

  for (const image of saved) {
    text += `- ${image.path} (${formatBytes(image.bytes)})\n`;
    if (image.revisedPrompt) {
      text += `  Revised prompt: ${image.revisedPrompt}\n`;
    }
  }

  if (response.usage?.total_tokens !== undefined) {
    text += `\nUsage: ${response.usage.input_tokens ?? '?'} input + ${response.usage.output_tokens ?? '?'} output tokens\n`;
  }

  const content: CallToolResult['content'] = [];
  if (params.inline) {
    const tooLarge = saved.filter(image => image.bytes > MAX_INLINE_BYTES);
    if (tooLarge.length > 0) {
      text += `\n${tooLarge.length} ${tooLarge.length === 1 ? 'image is' : 'images are'} over ${formatBytes(MAX_INLINE_BYTES)} and not shown inline; use the Read tool on the path to view.\n`;
    }
    for (const image of saved) {
      if (image.bytes <= MAX_INLINE_BYTES) {
        content.push({ type: 'image', data: image.base64Data, mimeType: `image/${image.format}` });
      }
    }
  } else {
    text += '\nUse the Read tool on a path to view the image.\n';
  }

  return { content: [{ type: 'text', text }, ...content] };
}

/**
 * Turn any failure into a readable tool error
 */
export function errorResult(action: string, error: unknown): CallToolResult {
  let text: string;
  if (error instanceof ApiKeyError) {
    text = 'Error: OPENAI_API_KEY environment variable is not set. Add it to the environment Claude Code runs in, then restart the session.';
  } else if (error instanceof OpenAIApiError) {
    text = `OpenAI API error (HTTP ${error.status}${error.code ? `, ${error.code}` : ''}): ${error.message}`;
  } else if (error instanceof NetworkError) {
    text = `Network error: ${error.message}`;
  } else {
    text = `Error ${action}: ${(error as Error).message}`;
  }

  return { content: [{ type: 'text', text }], isError: true };
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
