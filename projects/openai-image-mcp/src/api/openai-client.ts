import type {
  EditImageRequest,
  GenerateImageRequest,
  ImageOptions,
  ImageResponse,
  InputImage,
  OpenAIApiErrorBody,
} from './types.js';
import { API_BASE_URL, REQUEST_TIMEOUT_MS, readEnv } from '../config/defaults.js';

export class ApiKeyError extends Error {
  constructor() {
    super('OPENAI_API_KEY environment variable is not set');
    this.name = 'ApiKeyError';
  }
}

export class OpenAIApiError extends Error {
  status: number;
  code?: string;

  constructor(status: number, body: OpenAIApiErrorBody) {
    super(body.message);
    this.name = 'OpenAIApiError';
    this.status = status;
    this.code = body.code ?? body.type ?? undefined;
  }
}

export class NetworkError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'NetworkError';
  }
}

export async function generateImage(request: GenerateImageRequest): Promise<ImageResponse> {
  // JSON.stringify drops undefined fields, so unset options use the API defaults
  return post('/images/generations', JSON.stringify(request), { 'Content-Type': 'application/json' });
}

export async function editImage(request: EditImageRequest): Promise<ImageResponse> {
  const form = new FormData();
  form.append('prompt', request.prompt);

  // The API takes a single `image` file or an `image[]` list for multiple references
  const imageField = request.images.length > 1 ? 'image[]' : 'image';
  for (const image of request.images) {
    form.append(imageField, toBlob(image), image.filename);
  }
  if (request.mask) {
    form.append('mask', toBlob(request.mask), request.mask.filename);
  }

  const fields: ImageOptions & { input_fidelity?: string } = {
    model: request.model,
    size: request.size,
    quality: request.quality,
    background: request.background,
    output_format: request.output_format,
    output_compression: request.output_compression,
    n: request.n,
    input_fidelity: request.input_fidelity,
  };
  for (const [key, value] of Object.entries(fields)) {
    if (value !== undefined) {
      form.append(key, String(value));
    }
  }

  // fetch sets the multipart Content-Type (with boundary) itself
  return post('/images/edits', form, {});
}

function toBlob(image: InputImage): Blob {
  return new Blob([new Uint8Array(image.data)], { type: image.mimeType });
}

async function post(endpoint: string, body: string | FormData, headers: Record<string, string>): Promise<ImageResponse> {
  const apiKey = readEnv('OPENAI_API_KEY');
  if (!apiKey) {
    throw new ApiKeyError();
  }

  const url = `${API_BASE_URL}${endpoint}`;
  let response: Response;
  try {
    response = await fetch(url, {
      method: 'POST',
      headers: { ...headers, Authorization: `Bearer ${apiKey}` },
      body,
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
    });
  } catch (error) {
    throw toNetworkError(error, url);
  }

  if (!response.ok) {
    const errorData = await response.json().catch(() => ({
      error: { message: response.statusText || `HTTP ${response.status}` },
    }));
    throw new OpenAIApiError(response.status, errorData.error ?? { message: `HTTP ${response.status}` });
  }

  const data = await response.json();
  const images = (data.data || [])
    .filter((item: { b64_json?: string }) => item.b64_json)
    .map((item: { b64_json: string; revised_prompt?: string }) => ({
      base64Data: item.b64_json,
      revisedPrompt: item.revised_prompt,
    }));

  if (images.length === 0) {
    throw new Error('No base64 image data found in API response (only GPT Image models are supported)');
  }

  return {
    images,
    size: data.size,
    quality: data.quality,
    background: data.background,
    outputFormat: data.output_format,
    usage: data.usage,
  };
}

function toNetworkError(error: unknown, url: string): NetworkError {
  const err = error as Error & { cause?: Error };
  if (err.name === 'TimeoutError') {
    return new NetworkError(`Request to ${url} timed out after ${REQUEST_TIMEOUT_MS / 1000} seconds`);
  }

  let message = `Could not reach ${url}: ${err.cause?.message ?? err.message}`;
  // Node's built-in fetch ignores HTTPS_PROXY unless NODE_USE_ENV_PROXY=1 (Node >= 22.21)
  if (readEnv('HTTPS_PROXY') && readEnv('NODE_USE_ENV_PROXY') !== '1') {
    message += '. HTTPS_PROXY is set but NODE_USE_ENV_PROXY=1 is not, so this request bypassed the proxy.';
  }
  return new NetworkError(message);
}
