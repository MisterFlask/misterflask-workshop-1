export interface GeminiImageRequest {
    prompt: string;
    model: 'gemini-2.5-flash-image' | 'gemini-3-pro-image-preview';
    aspectRatio?: '1:1' | '16:9' | '9:16' | '4:3' | '3:4';
}
export interface GeminiImageResponse {
    base64Data: string;
    mimeType: string;
}
export interface GeminiApiError {
    code: number;
    message: string;
    status: string;
}
export type SpriteCategory = 'soldier' | 'building' | 'terrain' | 'terrain_feature' | 'icon' | 'effect';
export type SpriteStyle = 'fantasy' | 'retro' | 'modern_pixel';
export interface GenerateSpriteParams {
    category: SpriteCategory;
    name: string;
    description?: string;
    style: SpriteStyle;
    palette?: string[];
    model: 'gemini-2.5-flash-image' | 'gemini-3-pro-image-preview';
    save: boolean;
}
export interface GenerateSpriteResult {
    name: string;
    category: SpriteCategory;
    base64Data: string;
    mimeType: string;
    savedPath?: string;
}
export interface BatchSpriteItem {
    category: SpriteCategory;
    name: string;
    description?: string;
}
export interface BatchResult {
    successes: GenerateSpriteResult[];
    failures: Array<{
        name: string;
        error: string;
    }>;
}
