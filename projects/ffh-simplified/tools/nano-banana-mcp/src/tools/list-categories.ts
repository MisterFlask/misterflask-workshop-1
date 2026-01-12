import { z } from 'zod';
import { getSpriteStatus, getAllSpriteStatus } from '../utils/file-handler.js';
import type { SpriteCategory } from '../api/types.js';

export const listCategoriesSchema = z.object({
  category: z.enum(['soldier', 'building', 'terrain', 'terrain_feature', 'icon', 'effect', 'all'])
    .optional()
    .default('all')
    .describe('Which category to list, or "all" for all categories'),
});

export type ListCategoriesInput = z.infer<typeof listCategoriesSchema>;

type TextContent = { type: 'text'; text: string };
type ToolResult = { content: Array<TextContent> };

export async function handleListCategories(params: ListCategoriesInput): Promise<ToolResult> {
  let output = '# Sprite Categories Status\n\n';

  if (params.category === 'all') {
    const allStatus = await getAllSpriteStatus();

    for (const [category, status] of Object.entries(allStatus)) {
      output += formatCategoryStatus(category as SpriteCategory, status);
    }

    // Add totals
    const totalExisting = Object.values(allStatus).reduce((sum, s) => sum + s.existing.length, 0);
    const totalMissing = Object.values(allStatus).reduce((sum, s) => sum + s.missing.length, 0);
    const grandTotal = totalExisting + totalMissing;

    output += `---\n\n`;
    output += `## Summary\n`;
    output += `- **Total sprites needed:** ${grandTotal}\n`;
    output += `- **Existing:** ${totalExisting} (${Math.round(totalExisting / grandTotal * 100)}%)\n`;
    output += `- **Missing:** ${totalMissing} (${Math.round(totalMissing / grandTotal * 100)}%)\n`;
  } else {
    const status = await getSpriteStatus(params.category);
    output += formatCategoryStatus(params.category, status);
  }

  return {
    content: [{ type: 'text' as const, text: output }],
  };
}

function formatCategoryStatus(
  category: SpriteCategory,
  status: { existing: string[]; missing: string[]; total: number }
): string {
  let output = `## ${formatCategoryName(category)}\n\n`;
  output += `**Progress:** ${status.existing.length}/${status.total} sprites\n\n`;

  if (status.existing.length > 0) {
    output += `**Existing:**\n`;
    for (const name of status.existing.slice(0, 10)) {
      output += `- ${name}\n`;
    }
    if (status.existing.length > 10) {
      output += `- ... and ${status.existing.length - 10} more\n`;
    }
    output += '\n';
  }

  if (status.missing.length > 0) {
    output += `**Missing:**\n`;
    for (const name of status.missing.slice(0, 10)) {
      output += `- ${name}\n`;
    }
    if (status.missing.length > 10) {
      output += `- ... and ${status.missing.length - 10} more\n`;
    }
    output += '\n';
  }

  return output;
}

function formatCategoryName(category: SpriteCategory): string {
  return category
    .split('_')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ') + 's';
}
