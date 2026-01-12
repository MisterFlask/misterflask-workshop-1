import { z } from 'zod';
import { generateImage, ApiKeyError, RateLimitError, GeminiApiClientError } from '../api/gemini-client.js';
import { buildPrompt } from '../utils/prompt-builder.js';
import { resizeTo32x32, base64ToBuffer, bufferToBase64 } from '../utils/image-processor.js';
import { saveSprite } from '../utils/file-handler.js';
import { DEFAULT_MODEL, DEFAULT_STYLE } from '../config/defaults.js';
export const generateSpriteSchema = z.object({
    category: z.enum(['soldier', 'building', 'terrain', 'terrain_feature', 'icon', 'effect'])
        .describe('The category of sprite to generate'),
    name: z.string()
        .describe('Identifier for the sprite (e.g., "fire_elemental", "mage_tower")'),
    description: z.string().optional()
        .describe('Additional description to guide generation'),
    style: z.enum(['fantasy', 'retro', 'modern_pixel']).default(DEFAULT_STYLE)
        .describe('Visual style for the sprite'),
    palette: z.array(z.string()).optional()
        .describe('Hex colors to use (e.g., ["#ff0000", "#00ff00"])'),
    model: z.enum(['gemini-2.5-flash-image', 'gemini-3-pro-image-preview']).default(DEFAULT_MODEL)
        .describe('Which Nano Banana model to use'),
    save: z.boolean().default(true)
        .describe('Whether to save the generated image to assets'),
});
export async function handleGenerateSprite(params) {
    try {
        // Build the prompt
        const prompt = buildPrompt({
            category: params.category,
            name: params.name,
            description: params.description,
            style: params.style,
            palette: params.palette,
        });
        // Generate the image
        const result = await generateImage({
            prompt,
            model: params.model,
        });
        // Convert to buffer and resize to 32x32
        const originalBuffer = base64ToBuffer(result.base64Data);
        const resizedBuffer = await resizeTo32x32(originalBuffer);
        const resizedBase64 = bufferToBase64(resizedBuffer);
        // Save if requested
        let savedPath;
        if (params.save) {
            savedPath = await saveSprite(resizedBuffer, params.category, params.name);
        }
        // Return the result
        const textContent = savedPath
            ? `Generated sprite "${params.name}" (${params.category}) and saved to: ${savedPath}`
            : `Generated sprite "${params.name}" (${params.category})`;
        return {
            content: [
                { type: 'text', text: textContent },
                { type: 'image', data: resizedBase64, mimeType: 'image/png' },
            ],
        };
    }
    catch (error) {
        if (error instanceof ApiKeyError) {
            return {
                content: [{ type: 'text', text: 'Error: GEMINI_API_KEY environment variable is not set. Please set it to use this tool.' }],
                isError: true,
            };
        }
        if (error instanceof RateLimitError) {
            return {
                content: [{ type: 'text', text: `Rate limited by Gemini API. Please retry after ${error.retryAfter} seconds.` }],
                isError: true,
            };
        }
        if (error instanceof GeminiApiClientError) {
            return {
                content: [{ type: 'text', text: `Gemini API error (${error.status}): ${error.message}` }],
                isError: true,
            };
        }
        return {
            content: [{ type: 'text', text: `Error generating sprite: ${error.message}` }],
            isError: true,
        };
    }
}
