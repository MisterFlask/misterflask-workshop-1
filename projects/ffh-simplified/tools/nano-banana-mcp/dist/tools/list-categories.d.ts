import { z } from 'zod';
export declare const listCategoriesSchema: z.ZodObject<{
    category: z.ZodDefault<z.ZodOptional<z.ZodEnum<["soldier", "building", "terrain", "terrain_feature", "icon", "effect", "all"]>>>;
}, "strip", z.ZodTypeAny, {
    category: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect" | "all";
}, {
    category?: "soldier" | "building" | "terrain" | "terrain_feature" | "icon" | "effect" | "all" | undefined;
}>;
export type ListCategoriesInput = z.infer<typeof listCategoriesSchema>;
type TextContent = {
    type: 'text';
    text: string;
};
type ToolResult = {
    content: Array<TextContent>;
};
export declare function handleListCategories(params: ListCategoriesInput): Promise<ToolResult>;
export {};
