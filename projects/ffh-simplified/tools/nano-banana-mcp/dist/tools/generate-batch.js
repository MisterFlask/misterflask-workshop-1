import { z } from 'zod';
import { handleGenerateSprite } from './generate-sprite.js';
import { DEFAULT_MODEL, DEFAULT_STYLE } from '../config/defaults.js';
const batchSpriteItemSchema = z.object({
    category: z.enum(['soldier', 'building', 'terrain', 'terrain_feature', 'icon', 'effect']),
    name: z.string(),
    description: z.string().optional(),
});
export const generateBatchSchema = z.object({
    sprites: z.array(batchSpriteItemSchema)
        .min(1)
        .max(20)
        .describe('Array of sprites to generate (max 20)'),
    style: z.enum(['fantasy', 'retro', 'modern_pixel']).default(DEFAULT_STYLE)
        .describe('Visual style for all sprites'),
    model: z.enum(['gemini-2.5-flash-image', 'gemini-3-pro-image-preview']).default(DEFAULT_MODEL)
        .describe('Which Nano Banana model to use'),
});
export async function handleGenerateBatch(params) {
    const results = [];
    for (const sprite of params.sprites) {
        try {
            const result = await handleGenerateSprite({
                category: sprite.category,
                name: sprite.name,
                description: sprite.description,
                style: params.style,
                model: params.model,
                save: true,
            });
            if (result.isError) {
                const firstContent = result.content[0];
                const errorText = firstContent && 'text' in firstContent ? firstContent.text : 'Unknown error';
                results.push({
                    name: sprite.name,
                    category: sprite.category,
                    success: false,
                    error: errorText,
                });
            }
            else {
                // Extract saved path from the text content
                const textItem = result.content.find(c => c.type === 'text');
                const textContent = textItem && 'text' in textItem ? textItem.text : '';
                const pathMatch = textContent.match(/saved to: (.+)$/);
                results.push({
                    name: sprite.name,
                    category: sprite.category,
                    success: true,
                    savedPath: pathMatch?.[1],
                });
            }
        }
        catch (error) {
            results.push({
                name: sprite.name,
                category: sprite.category,
                success: false,
                error: error.message,
            });
        }
        // Small delay between requests to avoid rate limiting
        await new Promise(resolve => setTimeout(resolve, 500));
    }
    // Build summary
    const successes = results.filter(r => r.success);
    const failures = results.filter(r => !r.success);
    let summary = `## Batch Generation Complete\n\n`;
    summary += `**Total:** ${results.length} sprites\n`;
    summary += `**Successes:** ${successes.length}\n`;
    summary += `**Failures:** ${failures.length}\n\n`;
    if (successes.length > 0) {
        summary += `### Generated Sprites\n`;
        for (const s of successes) {
            summary += `- ${s.name} (${s.category})${s.savedPath ? `: ${s.savedPath}` : ''}\n`;
        }
        summary += '\n';
    }
    if (failures.length > 0) {
        summary += `### Failed Sprites\n`;
        for (const f of failures) {
            summary += `- ${f.name} (${f.category}): ${f.error}\n`;
        }
    }
    return {
        content: [{ type: 'text', text: summary }],
        isError: failures.length === results.length,
    };
}
