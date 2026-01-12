import type { SpriteCategory, SpriteStyle } from '../api/types.js';
import { CATEGORY_PROMPTS, STYLE_DESCRIPTIONS } from '../config/categories.js';

export interface PromptBuilderParams {
  category: SpriteCategory;
  name: string;
  description?: string;
  style: SpriteStyle;
  palette?: string[];
}

function formatName(name: string): string {
  // Convert snake_case to readable format: fire_elemental -> Fire Elemental
  return name
    .split('_')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

export function buildPrompt(params: PromptBuilderParams): string {
  const { category, name, description, style, palette } = params;
  const categoryConfig = CATEGORY_PROMPTS[category];
  const styleDesc = STYLE_DESCRIPTIONS[style];
  const formattedName = formatName(name);

  // Build the prompt
  let prompt = categoryConfig.base.replace('{name}', formattedName);

  // Add style
  prompt += `\n\nStyle: ${styleDesc}`;

  // Add requirements
  prompt += `\n\nRequirements:${categoryConfig.requirements.replace(/\{name\}/g, formattedName)}`;

  // Add custom description if provided
  if (description) {
    prompt += `\n\nAdditional details: ${description}`;
  }

  // Add palette instructions if provided
  if (palette && palette.length > 0) {
    prompt += `\n\nColor palette: Use primarily these colors: ${palette.join(', ')}`;
  }

  // Final technical requirements
  prompt += `\n\nTechnical requirements:
- Output exactly 32x32 pixels
- PNG format with transparency where appropriate
- Pixel-perfect art (no anti-aliasing blur)
- Crisp, defined pixels`;

  return prompt;
}
