import fs from 'fs/promises';
import path from 'path';
import type { SpriteCategory } from '../api/types.js';
import { DEFAULT_OUTPUT_DIR } from '../config/defaults.js';
import { SPRITE_IDS } from '../config/categories.js';

/**
 * Ensure directory exists, creating it if necessary
 */
async function ensureDir(dirPath: string): Promise<void> {
  try {
    await fs.mkdir(dirPath, { recursive: true });
  } catch (error) {
    // Ignore if already exists
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') {
      throw error;
    }
  }
}

/**
 * Get the output path for a sprite
 */
export function getSpritePath(category: SpriteCategory, name: string): string {
  // Map category to directory name (use plural for most)
  const dirName = category === 'terrain_feature' ? 'terrain_features' : `${category}s`;
  return path.join(DEFAULT_OUTPUT_DIR, dirName, `${name}.png`);
}

/**
 * Save a sprite image to the file system
 */
export async function saveSprite(
  imageBuffer: Buffer,
  category: SpriteCategory,
  name: string
): Promise<string> {
  const filePath = getSpritePath(category, name);
  const dirPath = path.dirname(filePath);

  await ensureDir(dirPath);
  await fs.writeFile(filePath, imageBuffer);

  return filePath;
}

/**
 * Check if a sprite file exists
 */
export async function spriteExists(category: SpriteCategory, name: string): Promise<boolean> {
  const filePath = getSpritePath(category, name);
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

/**
 * List existing sprites in a category
 */
export async function listExistingSprites(category: SpriteCategory): Promise<string[]> {
  const dirName = category === 'terrain_feature' ? 'terrain_features' : `${category}s`;
  const dirPath = path.join(DEFAULT_OUTPUT_DIR, dirName);

  try {
    const files = await fs.readdir(dirPath);
    return files
      .filter(f => f.endsWith('.png'))
      .map(f => f.replace('.png', ''));
  } catch {
    return [];
  }
}

/**
 * Get sprite status for a category: which exist and which are missing
 */
export async function getSpriteStatus(category: SpriteCategory): Promise<{
  existing: string[];
  missing: string[];
  total: number;
}> {
  const allIds = SPRITE_IDS[category];
  const existing = await listExistingSprites(category);
  const existingSet = new Set(existing);
  const missing = allIds.filter(id => !existingSet.has(id));

  return {
    existing,
    missing,
    total: allIds.length,
  };
}

/**
 * Get sprite status for all categories
 */
export async function getAllSpriteStatus(): Promise<
  Record<SpriteCategory, { existing: string[]; missing: string[]; total: number }>
> {
  const categories: SpriteCategory[] = ['soldier', 'building', 'terrain', 'terrain_feature', 'icon', 'effect'];
  const result: Record<string, { existing: string[]; missing: string[]; total: number }> = {};

  for (const category of categories) {
    result[category] = await getSpriteStatus(category);
  }

  return result as Record<SpriteCategory, { existing: string[]; missing: string[]; total: number }>;
}
