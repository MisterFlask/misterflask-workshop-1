import type { SpriteCategory, SpriteStyle } from '../api/types.js';
export interface PromptBuilderParams {
    category: SpriteCategory;
    name: string;
    description?: string;
    style: SpriteStyle;
    palette?: string[];
}
export declare function buildPrompt(params: PromptBuilderParams): string;
