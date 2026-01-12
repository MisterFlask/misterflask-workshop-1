/**
 * Resize an image to exactly 32x32 pixels
 * Uses nearest-neighbor scaling to preserve pixel art sharpness
 */
export declare function resizeTo32x32(imageBuffer: Buffer): Promise<Buffer>;
/**
 * Convert base64 string to Buffer
 */
export declare function base64ToBuffer(base64Data: string): Buffer;
/**
 * Convert Buffer to base64 string
 */
export declare function bufferToBase64(buffer: Buffer): string;
/**
 * Get image metadata (dimensions, format)
 */
export declare function getImageMetadata(imageBuffer: Buffer): Promise<{
    width: number;
    height: number;
    format: string;
}>;
