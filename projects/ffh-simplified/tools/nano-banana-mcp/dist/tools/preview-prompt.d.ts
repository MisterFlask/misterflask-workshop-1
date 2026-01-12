import { z } from 'zod';
export declare const previewPromptSchema: z.ZodObject<{
    category: z.ZodEnum<["soldier", "building", "terrain", "terrain_feature", "icon", "effect"]>;
    name: z.ZodString;
    description: z.ZodOptional<z.ZodString>;
    style: z.ZodDefault<z.ZodEnum<["fantasy", "retro", "modern_pixel"]>>;
    palette: z.ZodOptional<z.ZodArray<z.ZodString, "many">>;
}, "strip", z.ZodTypeAny, {
    category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
    name: string;
    style: "fantasy" | "retro" | "modern_pixel";
    description?: string | undefined;
    palette?: string[] | undefined;
}, {
    category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect";
    name: string;
    description?: string | undefined;
    style?: "fantasy" | "retro" | "modern_pixel" | undefined;
    palette?: string[] | undefined;
}>;
export type PreviewPromptInput = z.infer<typeof previewPromptSchema>;
type TextContent = {
    type: 'text';
    text: string;
};
type ToolResult = {
    content: Array<TextContent>;
};
export declare function handlePreviewPrompt(params: PreviewPromptInput): Promise<ToolResult>;
export {};
