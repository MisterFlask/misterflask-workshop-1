import type { SpriteCategory } from '../api/types.js';
/**
 * Get the output path for a sprite
 */
export declare function getSpritePath(category: SpriteCategory, name: string): string;
/**
 * Save a sprite image to the file system
 */
export declare function saveSprite(imageBuffer: Buffer, category: SpriteCategory, name: string): Promise<string>;
/**
 * Check if a sprite file exists
 */
export declare function spriteExists(category: SpriteCategory, name: string): Promise<boolean>;
/**
 * List existing sprites in a category
 */
export declare function listExistingSprites(category: SpriteCategory): Promise<string[]>;
/**
 * Get sprite status for a category: which exist and which are missing
 */
export declare function getSpriteStatus(category: SpriteCategory): Promise<{
    existing: string[];
    missing: string[];
    total: number;
}>;
/**
 * Get sprite status for all categories
 */
export declare function getAllSpriteStatus(): Promise<Record<SpriteCategory, {
    existing: string[];
    missing: string[];
    total: number;
}>>;
