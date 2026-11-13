import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = dirname(fileURLToPath(import.meta.url));
const html = await readFile(join(root, 'index.html'), 'utf8');
const source = `const html = ${JSON.stringify(html)};

export default {
  async fetch(request) {
    const { pathname } = new URL(request.url);
    if (pathname === '/install.sh') {
      return Response.redirect('https://raw.githubusercontent.com/plu-nz/passage/main/scripts/install.sh', 302);
    }
    if (pathname !== '/') return new Response('Page not found', { status: 404 });
    return new Response(request.method === 'HEAD' ? null : html, {
      headers: {
        'content-type': 'text/html; charset=utf-8',
        'cache-control': 'public, max-age=300',
        'content-security-policy': "default-src 'none'; style-src 'unsafe-inline'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        'referrer-policy': 'strict-origin-when-cross-origin',
        'x-content-type-options': 'nosniff',
      },
    });
  },
};
`;
await mkdir(join(root, 'dist'), { recursive: true });
await writeFile(join(root, 'dist', 'worker.js'), source);
