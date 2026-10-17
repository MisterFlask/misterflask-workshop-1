// OpenAI Images API types

export type ImageQuality = 'low' | 'medium' | 'high' | 'xhigh' | 'max' | 'auto';
export type InputFidelity = 'low' | 'medium' | 'high' | 'xhigh' | 'max';
export type ImageBackground = 'transparent' | 'opaque' | 'auto';
export type ImageFormat = 'png' | 'jpeg' | 'webp';

// Options shared by /images/generations and /images/edits.
// Undefined fields are not sent, so the API applies its own per-model defaults.
export interface ImageOptions {
  model: string;
  size?: string;
  quality?: ImageQuality;
  background?: ImageBackground;
  output_format?: ImageFormat;
  output_compression?: number;
  n?: number;
}

export interface GenerateImageRequest extends ImageOptions {
  prompt: string;
  moderation?: 'low' | 'auto';
}

export interface InputImage {
  data: Buffer;
  filename: string;
  mimeType: string;
}

export interface EditImageRequest extends ImageOptions {
  prompt: string;
  images: InputImage[];
  mask?: InputImage;
  input_fidelity?: InputFidelity;
}

export interface GeneratedImage {
  base64Data: string;
  revisedPrompt?: string;
}

export interface ImageResponse {
  images: GeneratedImage[];
  size?: string;
  quality?: string;
  background?: string;
  outputFormat?: string;
  usage?: {
    input_tokens?: number;
    output_tokens?: number;
    total_tokens?: number;
  };
}

export interface OpenAIApiErrorBody {
  message: string;
  type?: string;
  code?: string | null;
  param?: string | null;
}
