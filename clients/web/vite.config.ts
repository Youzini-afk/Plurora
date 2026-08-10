import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { VitePWA } from 'vite-plugin-pwa';
import { existsSync, readFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const surfaceDevPaths = loadSurfaceDevPaths();
const ydltavernDevPath = surfaceDevPaths.ydltavern ?? resolve(__dirname, '../../../YdlTavern/packages/ydltavern-surface/dist');

function loadSurfaceDevPaths(): Record<string, string> {
  const envValue = process.env.PLURORA_SURFACE_DEV_PATHS;
  if (envValue) {
    try {
      return JSON.parse(envValue) as Record<string, string>;
    } catch {
      return {};
    }
  }

  const profilePath = process.env.PLURORA_HOST_PROFILE ?? resolve(__dirname, '../../profiles/forge-alpha.yaml');
  if (!existsSync(profilePath)) return {};
  const raw = readFileSync(profilePath, 'utf8');
  const map: Record<string, string> = {};
  const lines = raw.split(/\r?\n/);
  const start = lines.findIndex((line) => /^surface_dev_paths:\s*$/.test(line));
  if (start < 0) return map;
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i];
    if (/^\S/.test(line)) break;
    const match = line.match(/^\s{2}([^:#]+):\s*(.+?)\s*$/);
    if (!match) continue;
    const key = match[1].trim();
    const value = match[2].trim().replace(/^['"]|['"]$/g, '');
    map[key] = resolve(profilePath, '..', value);
  }
  return map;
}

// Plurora web client config.
// Plain TS app with iframe-hosted surface bundles. No framework.
// Public assets in clients/web/public/ are served at /.
export default defineConfig({
  root: '.',
  publicDir: 'public',
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
    },
  },
  build: {
    outDir: 'dist',
    target: 'es2022',
    sourcemap: true,
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: 'assets/main-[hash].js',
        assetFileNames: (assetInfo) => {
          if (assetInfo.name?.endsWith('.css')) return 'assets/styles-[hash][extname]';
          return 'assets/[name]-[hash][extname]';
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: '127.0.0.1',
    // Proxy control-plane routes to host serve during dev so the web client
    // can hit them at same-origin without CORS hassle.
    proxy: {
      '/rpc': {
        target: 'http://127.0.0.1:8787',
        changeOrigin: false,
      },
      '/journal': {
        target: 'http://127.0.0.1:8787',
        changeOrigin: false,
      },
      '/host': {
        target: 'http://127.0.0.1:8787',
        changeOrigin: false,
      },
    },
    fs: {
      // Allow serving files from the sibling YdlTavern repo for surface bundles.
      allow: [
        '..',
        resolve(__dirname, '../../../YdlTavern'),
        ...Object.values(surfaceDevPaths),
      ],
    },
  },
  // Dev-only middleware for demo surface bundles. Production should expose
  // package assets through a host-owned same-origin static route.
  plugins: [
    react(),
    tailwindcss(),
    VitePWA({
      registerType: 'prompt',
      injectRegister: null,
      includeAssets: ['icons/plurora.svg', 'icons/plurora-maskable.svg'],
      manifest: {
        id: '/',
        name: 'Plurora Library',
        short_name: 'Plurora',
        description: 'A permissioned Library for installing Works and controlling explicit Runs.',
        start_url: '/',
        scope: '/',
        display: 'standalone',
        background_color: '#fafaf7',
        theme_color: '#fafaf7',
        icons: [
          {
            src: '/icons/plurora.svg',
            sizes: 'any',
            type: 'image/svg+xml',
            purpose: 'any',
          },
          {
            src: '/icons/plurora-maskable.svg',
            sizes: 'any',
            type: 'image/svg+xml',
            purpose: 'maskable',
          },
        ],
      },
      workbox: {
        cleanupOutdatedCaches: true,
        globPatterns: ['**/*.{js,css,html,svg,webmanifest,woff,woff2}'],
        globIgnores: ['**/*.map'],
        navigateFallback: '/index.html',
        navigateFallbackAllowlist: [/^\/$/, /^\/installation\/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/],
        navigateFallbackDenylist: [
          /^\/rpc(?:\/|$)/,
          /^\/journal(?:\/|$)/,
          /^\/host(?:\/|$)/,
          /^\/p(?:\/|$)/,
          /^\/surface-bundles(?:\/|$)/,
        ],
        clientsClaim: false,
        skipWaiting: false,
      },
    }),
    {
      name: 'surface-dev-bundle-server',
      configureServer(server) {
        server.middlewares.use((req, res, next) => {
          const requestPath = req.url?.split('?')[0] ?? '';
          const match = requestPath.match(/^\/surface-bundles\/([^/]+)\/(.+)$/);
          if (!match) {
            next();
            return;
          }

          const prefix = decodeURIComponent(match[1]);
          const relativePath = decodeURIComponent(match[2]);
          const base = surfaceDevPaths[prefix] ?? (prefix === 'ydltavern' ? ydltavernDevPath : undefined);
          if (!base || relativePath.includes('..')) {
            next();
            return;
          }
          const filePath = resolve(base, relativePath);

          readFile(filePath)
            .then((data) => {
              if (relativePath.endsWith('.mjs') || relativePath.endsWith('.js')) {
                res.setHeader('Content-Type', 'application/javascript');
              } else if (relativePath.endsWith('.css')) {
                res.setHeader('Content-Type', 'text/css');
              } else if (relativePath.endsWith('.map')) {
                res.setHeader('Content-Type', 'application/json');
              }
              res.end(data);
            })
            .catch(() => {
              res.statusCode = 404;
              res.end('Not found');
            });
        });
      },
    },
    {
      name: 'ydltavern-st-compat-server',
      configureServer(server) {
        const compatBase = resolve(ydltavernDevPath, 'st-compat');
        const routes = {
          '/script.js': 'script.js',
          '/scripts/extensions.js': 'scripts/extensions.js',
          '/scripts/events.js': 'scripts/events.js',
          '/scripts/st-context.js': 'scripts/st-context.js',
          '/scripts/group-chats.js': 'scripts/group-chats.js',
          '/scripts/secrets.js': 'scripts/secrets.js',
          '/scripts/power-user.js': 'scripts/power-user.js',
        } as const;

        server.middlewares.use((req, res, next) => {
          const url = req.url?.split('?')[0];
          if (!url) {
            next();
            return;
          }

          const routePath = routes[url as keyof typeof routes];
          if (routePath) {
            const filePath = resolve(compatBase, routePath);
            readFile(filePath)
              .then((data) => {
                res.setHeader('Content-Type', 'application/javascript');
                res.end(data);
              })
              .catch(() => {
                res.statusCode = 404;
                res.end(`Not found: ${url} (st-compat shim missing — did you run npm run build in packages/ydltavern-surface?)`);
              });
            return;
          }

          // TODO(Round 9): Serve installed third-party ST extensions from the
          // host-managed extension install directory once that route is wired.
          if (url.startsWith('/scripts/extensions/')) {
            res.statusCode = 404;
            res.setHeader('Content-Type', 'text/plain');
            res.end(`/scripts/extensions/* serving not yet wired (Round 9 work). Path requested: ${url}`);
            return;
          }

          next();
        });
      },
    },
  ],
});
