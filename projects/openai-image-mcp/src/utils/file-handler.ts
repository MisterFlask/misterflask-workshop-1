import fs from 'fs/promises';
import path from 'path';
import type { GeneratedImage, InputImage } from '../api/types.js';
import { DEFAULT_OUTPUT_DIR, PROJECT_DIR } from '../config/defaults.js';

const MIME_TYPES: Record<string, string> = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
};

export interface SavedImage {
  path: string;
  bytes: number;
  format: string;
  base64Data: string;
  revisedPrompt?: string;
}

export interface SaveOptions {
  outputDir?: string;
  filename?: string;
  prompt: string;
  fallbackFormat?: string;
}

/**
 * Read a local image to send to the edits endpoint
 */
export async function readInputImage(filePath: string): Promise<InputImage> {
  const resolved = path.resolve(PROJECT_DIR, filePath);
  const mimeType = MIME_TYPES[path.extname(resolved).toLowerCase()];
  if (!mimeType) {
    throw new Error(`Unsupported input image type: ${filePath} (use .png, .jpg, .jpeg or .webp)`);
  }

  try {
    const data = await fs.readFile(resolved);
    return { data, filename: path.basename(resolved), mimeType };
  } catch (error) {
    throw new Error(`Could not read input image ${resolved}: ${(error as Error).message}`);
  }
}

/**
 * Save generated images to disk.
 * An explicit filename overwrites existing files; generated names never do.
 */
export async function saveImages(images: GeneratedImage[], options: SaveOptions): Promise<SavedImage[]> {
  const outputDir = path.resolve(PROJECT_DIR, options.outputDir ?? DEFAULT_OUTPUT_DIR);
  await fs.mkdir(outputDir, { recursive: true });

  const baseName = options.filename
    ? options.filename.replace(/\.(png|jpe?g|webp)$/i, '')
    : `${timestamp()}-${slugify(options.prompt)}`;

  const saved: SavedImage[] = [];
  for (const [index, image] of images.entries()) {
    const buffer = Buffer.from(image.base64Data, 'base64');
    const format = detectFormat(buffer) ?? options.fallbackFormat ?? 'png';
    const extension = format === 'jpeg' ? 'jpg' : format;
    const stem = images.length > 1 ? `${baseName}-${index + 1}` : baseName;

    let filePath = path.join(outputDir, `${stem}.${extension}`);
    if (!options.filename) {
      filePath = await uniquePath(filePath);
    }

    await fs.mkdir(path.dirname(filePath), { recursive: true });
    await fs.writeFile(filePath, buffer);
    saved.push({
      path: filePath,
      bytes: buffer.length,
      format,
      base64Data: image.base64Data,
      revisedPrompt: image.revisedPrompt,
    });
  }

  return saved;
}

/**
 * Identify the image format from its magic bytes
 */
function detectFormat(buffer: Buffer): string | undefined {
  if (buffer.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))) {
    return 'png';
  }
  if (buffer[0] === 0xff && buffer[1] === 0xd8 && buffer[2] === 0xff) {
    return 'jpeg';
  }
  if (buffer.toString('ascii', 0, 4) === 'RIFF' && buffer.toString('ascii', 8, 12) === 'WEBP') {
    return 'webp';
  }
  return undefined;
}

async function uniquePath(filePath: string): Promise<string> {
  const { dir, name, ext } = path.parse(filePath);
  let candidate = filePath;
  for (let i = 2; await exists(candidate); i++) {
    candidate = path.join(dir, `${name}-${i}${ext}`);
  }
  return candidate;
}

async function exists(filePath: string): Promise<boolean> {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

function slugify(text: string): string {
  const slug = text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 40)
    .replace(/-+$/, '');
  return slug || 'image';
}

function timestamp(): string {
  // 2026-10-17T03:15:22.123Z -> 20261017-031522
  return new Date().toISOString().replace(/[-:]/g, '').replace('T', '-').slice(0, 15);
}
