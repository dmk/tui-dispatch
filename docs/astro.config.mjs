import { defineConfig } from 'astro/config';
import react from '@astrojs/react';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://dmk.github.io',
  base: '/tui-dispatch',
  integrations: [
    starlight({
      title: 'tui-dispatch',
      description: 'Centralized state management for Rust TUI apps',
      social: {
        github: 'https://github.com/dmk/tui-dispatch',
      },
      editLink: {
        baseUrl: 'https://github.com/dmk/tui-dispatch/edit/main/docs/',
      },
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Quick Start', slug: 'getting-started/quick-start' },
            { label: 'Core Concepts', slug: 'getting-started/core-concepts' },
            { label: 'Philosophy', slug: 'getting-started/philosophy' },
          ],
        },
        {
          label: 'Patterns',
          items: [
            { label: 'Async & Effects', slug: 'patterns/async' },
            { label: 'Event Bus', slug: 'patterns/event-bus' },
            { label: 'Keybindings', slug: 'patterns/keybindings' },
            { label: 'Middleware', slug: 'patterns/middleware' },
            { label: 'Reducer Composition', slug: 'patterns/reducer-composition' },
          ],
        },
        {
          label: 'Components',
          items: [
            { label: 'Pre-built Components', slug: 'components/prebuilt' },
            { label: 'Building Custom', slug: 'components/custom' },
          ],
        },
        {
          label: 'Debugging',
          items: [
            { label: 'Debug Layer', slug: 'debugging/debug-layer' },
            { label: 'Debug Sessions', slug: 'debugging/debug-sessions' },
            { label: 'Feature Flags', slug: 'debugging/feature-flags' },
          ],
        },
        {
          label: 'Examples',
          items: [
            { label: 'Overview', slug: 'examples' },
            { label: 'Counter', slug: 'examples/counter' },
            { label: 'GitHub Lookup', slug: 'examples/github-lookup' },
            { label: 'Markdown Preview', slug: 'examples/markdown-preview' },
            { label: 'Live Preview', slug: 'examples/live-preview' },
          ],
        },
        {
          label: 'Tutorials',
          items: [
            { label: 'Fetching Data', slug: 'tutorials/async-fetch' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'FAQ', slug: 'reference/faq' },
          ],
        },
        {
          label: 'Future',
          items: [
            { label: 'Ideas', slug: 'future/ideas' },
          ],
        },
      ],
    }),
    react(),
  ],
  vite: {
    resolve: {
      dedupe: ['react', 'react-dom'],
    },
    optimizeDeps: {
      exclude: ['@dkkoval/tui-preview'],
    },
  },
});
