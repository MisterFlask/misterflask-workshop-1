const API_BASE_URL = 'https://generativelanguage.googleapis.com/v1beta/models';
export class ApiKeyError extends Error {
    constructor() {
        super('GEMINI_API_KEY environment variable is not set');
        this.name = 'ApiKeyError';
    }
}
export class RateLimitError extends Error {
    retryAfter;
    constructor(retryAfter) {
        super(`Rate limited. Retry after ${retryAfter} seconds`);
        this.name = 'RateLimitError';
        this.retryAfter = retryAfter;
    }
}
export class GeminiApiClientError extends Error {
    code;
    status;
    constructor(error) {
        super(error.message);
        this.name = 'GeminiApiClientError';
        this.code = error.code;
        this.status = error.status;
    }
}
export async function generateImage(request) {
    const apiKey = process.env.GEMINI_API_KEY;
    if (!apiKey) {
        throw new ApiKeyError();
    }
    const url = `${API_BASE_URL}/${request.model}:generateContent`;
    const body = {
        contents: [
            {
                parts: [{ text: request.prompt }],
            },
        ],
        generationConfig: {
            responseModalities: ['IMAGE'],
            imageConfig: {
                aspectRatio: request.aspectRatio || '1:1',
            },
        },
    };
    const response = await fetch(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'x-goog-api-key': apiKey,
        },
        body: JSON.stringify(body),
    });
    if (response.status === 429) {
        const retryAfter = parseInt(response.headers.get('Retry-After') || '60', 10);
        throw new RateLimitError(retryAfter);
    }
    if (!response.ok) {
        const errorData = await response.json().catch(() => ({
            error: { code: response.status, message: response.statusText, status: 'ERROR' },
        }));
        throw new GeminiApiClientError(errorData.error);
    }
    const data = await response.json();
    // Find the image part in the response
    const candidates = data.candidates || [];
    for (const candidate of candidates) {
        const parts = candidate.content?.parts || [];
        for (const part of parts) {
            if (part.inline_data) {
                return {
                    base64Data: part.inline_data.data,
                    mimeType: part.inline_data.mime_type || 'image/png',
                };
            }
        }
    }
    throw new Error('No image found in API response');
}
