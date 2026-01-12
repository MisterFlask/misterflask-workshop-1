import { z } from 'zod';
export declare const generateBatchSchema: z.ZodObject<{
    sprites: z.ZodArray<z.ZodObject<{
        category: z.ZodEnum<["soldier", "building", "terrain", "terrain_feature", "icon", "effect"]>;
        name: z.ZodString;
        description: z.ZodOptional<z.ZodString>;
    }, "strip", z.ZodTypeAny, {
        category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
        name: string;
        description?: string | undefined;
    }, {
        category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
        name: string;
        description?: string | undefined;
    }>, "many">;
    style: z.ZodDefault<z.ZodEnum<["fantasy", "retro", "modern_pixel"]>>;
    model: z.ZodDefault<z.ZodEnum<["gemini-2.5-flash-image", "gemini-3-pro-image-preview"]>>;
}, "strip", z.ZodTypeAny, {
    style: "fantasy" | "retro" | "modern_pixel";
    sprites: {
        category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
        name: string;
        description?: string | undefined;
    }[];
    model: "gemini-2.5-flash-image" | "gemini-3-pro-image-preview";
}, {
    sprites: {
        category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
        name: string;
        description?: string | undefined;
    }[];
    style?: "fantasy" | "retro" | "modern_pixel" | undefined;
    model?: "gemini-2.5-flash-image" | "gemini-3-pro-image-preview" | undefined;
}>;
export type GenerateBatchInput = z.infer<typeof generateBatchSchema>;
type TextContent = {
    type: 'text';
    text: string;
};
type ToolResult = {
    content: Array<TextContent>;
    isError?: boolean;
};
export declare function handleGenerateBatch(params: GenerateBatchInput): Promise<ToolResult>;
export {};
