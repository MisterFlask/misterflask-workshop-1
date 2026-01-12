import { z } from 'zod';
import { buildPrompt } from '../utils/prompt-builder.js';
import { DEFAULT_STYLE } from '../config/defaults.js';

export const previewPromptSchema = z.object({
  category: z.enum(['soldier', 'building', 'terrain', 'terrain_feature', 'icon', 'effect'])
    .describe('The category of sprite'),
  name: z.string()
    .describe('Identifier for the sprite'),
  description: z.string().optional()
    .describe('Additional description to include'),
  style: z.enum(['fantasy', 'retro', 'modern_pixel']).default(DEFAULT_STYLE)
    .describe('Visual style for the sprite'),
  palette: z.array(z.string()).optional()
    .describe('Hex colors to use'),
});

export type PreviewPromptInput = z.infer<typeof previewPromptSchema>;

type TextContent = { type: 'text'; text: string };
type ToolResult = { content: Array<TextContent> };

export async function handlePreviewPrompt(params: PreviewPromptInput): Promise<ToolResult> {
  const prompt = buildPrompt({
    category: params.category,
    name: params.name,
    description: params.description,
    style: params.style,
    palette: params.palette,
  });

  const output = `# Prompt Preview for "${params.name}" (${params.category})

**Style:** ${params.style}
${params.palette ? `**Palette:** ${params.palette.join(', ')}` : ''}

---

\`\`\`
${prompt}
\`\`\`

---

*This prompt would be sent to the Nano Banana (Gemini) image generation API.*
*Use \`generate_sprite\` to actually generate the image.*
`;

  return {
    content: [{ type: 'text' as const, text: output }],
  };
}
