/// <reference types="vite/client" />

// Loads every sprite PNG under assets/sprites/ at build time so the game works
// identically in `vite dev` and in a production build (assets/ is not the Vite
// publicDir, so plain runtime URL strings would not survive `vite build`).
//
// `import.meta.glob` is a compile-time Vite macro: it is statically replaced
// with an object mapping each matched file's path to its resolved URL. This
// runs fine under vitest too (vitest transforms source through Vite), but the
// resulting HTMLImageElements are only usable in a real browser - see the
// `typeof Image === 'undefined'` guard below for the node/vitest case.
const spriteModules = import.meta.glob('../../assets/sprites/**/*.png', {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>;

// Re-key from the glob's module path (e.g. '../../assets/sprites/soldiers/knight.png')
// down to the relative path callers use (e.g. 'soldiers/knight.png').
const urlsByRelPath: Record<string, string> = {};
for (const [modulePath, url] of Object.entries(spriteModules)) {
  const match = modulePath.match(/assets\/sprites\/(.+)$/);
  if (match) {
    urlsByRelPath[match[1]] = url;
  }
}

const imageCache = new Map<string, HTMLImageElement>();

/**
 * Returns the HTMLImageElement for a sprite at `relPath` (e.g. 'soldiers/knight.png',
 * 'features/city.png'), or null if the sprite doesn't exist or hasn't finished
 * loading yet. Callers should always have a non-sprite fallback rendering path.
 */
/**
 * Returns the bundled URL for a sprite at `relPath`, or null if no such
 * sprite exists. Useful for DOM `<img>` elements (city panel icons etc.);
 * canvas code should prefer getSprite().
 */
export function getSpriteUrl(relPath: string): string | null {
  return urlsByRelPath[relPath] ?? null;
}

export function getSprite(relPath: string): HTMLImageElement | null {
  // Guard for non-browser contexts (vitest runs under node; Image is undefined there).
  if (typeof Image === 'undefined') return null;

  const url = urlsByRelPath[relPath];
  if (!url) return null;

  let img = imageCache.get(relPath);
  if (!img) {
    img = new Image();
    img.src = url;
    imageCache.set(relPath, img);
  }

  return img.complete && img.naturalWidth > 0 ? img : null;
}
