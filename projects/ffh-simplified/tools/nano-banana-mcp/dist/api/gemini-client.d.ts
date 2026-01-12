import type { GeminiImageRequest, GeminiImageResponse, GeminiApiError } from './types.js';
export declare class ApiKeyError extends Error {
    constructor();
}
export declare class RateLimitError extends Error {
    retryAfter: number;
    constructor(retryAfter: number);
}
export declare class GeminiApiClientError extends Error {
    code: number;
    status: string;
    constructor(error: GeminiApiError);
}
export declare function generateImage(request: GeminiImageRequest): Promise<GeminiImageResponse>;
