import { z } from 'zod';
export declare const generateSpriteSchema: z.ZodObject<{
    category: z.ZodEnum<["soldier", "building", "terrain", "terrain_feature", "icon", "effect"]>;
    name: z.ZodString;
    description: z.ZodOptional<z.ZodString>;
    style: z.ZodDefault<z.ZodEnum<["fantasy", "retro", "modern_pixel"]>>;
    palette: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
    model: z.ZodDefault<z.ZodEnum<["gemini-2.5-flash-image", "gemini-3-pro-image-preview"]>>;
    save: z.ZodDefault<z.ZodBoolean>;
}, "strip", z.ZodTypeAny, {
    category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
    name: string;
    style: "fantasy" | "retro" | "modern_pixel";
    model: "gemini-2.5-flash-image" | "gemini-3-pro-image-preview";
    save: boolean;
    description?: string | undefined;
    palette?: string[] | undefined;
}, {
    category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
    name: string;
    description?: string | undefined;
    style?: "fantasy" | "retro" | "modern_pixel" | undefined;
    palette?: string[] | undefined;
    model?: "gemini-2.5-flash-image" | "gemini-3-pro-image-preview" | undefined;
    save?: boolean | undefined;
}>;
export type GenerateSpriteInput = z.infer<typeof generateSpriteSchema>;
type TextContent = {
    type: 'text';
    text: string;
};
type ImageContent = {
    type: 'image';
    data: string;
    mimeType: string;
};
type ToolResult = {
    content: Array<TextContent | ImageContent>;
    isError?: boolean;
};
export declare function handleGenerateSprite(params: GenerateSpriteInput): Promise<ToolResult>;
export {};
