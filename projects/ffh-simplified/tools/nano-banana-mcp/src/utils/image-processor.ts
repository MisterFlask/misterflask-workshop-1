import sharp from 'sharp';
import { SPRITE_WIDTH, SPRITE_HEIGHT } from '../config/defaults.js';

/**
 * Resize an image to exactly 32x32 pixels
 * Uses nearest-neighbor scaling to preserve pixel art sharpness
 */
export async function resizeTo32x32(imageBuffer: Buffer): Promise<Buffer> {
  return sharp(imageBuffer)
    .resize(SPRITE_WIDTH, SPRITE_HEIGHT, {
      kernel: sharp.kernel.nearest, // Preserve pixel art sharpness
      fit: 'fill', // Exact size, may distort if not square
    })
    .png() // Always output PNG for transparency support
    .toBuffer();
}

/**
 * Convert base64 string to Buffer
 */
export function base64ToBuffer(base64Data: string): Buffer {
  return Buffer.from(base64Data, 'base64');
}

/**
 * Convert Buffer to base64 string
 */
export function bufferToBase64(buffer: Buffer): string {
  return buffer.toString('base64');
}

/**
 * Get image metadata (dimensions, format)
 */
export async function getImageMetadata(imageBuffer: Buffer): Promise<{
  width: number;
  height: number;
  format: string;
}> {
  const metadata = await sharp(imageBuffer).metadata();
  return {
    width: metadata.width || 0,
    height: metadata.height || 0,
    format: metadata.format || 'unknown',
  };
}
