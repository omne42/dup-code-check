import { defineConfig } from 'vitepress';

function resolveBase(): string {
  if (process.env.GITHUB_ACTIONS !== 'true') return '/';
  const repo = process.env.GITHUB_REPOSITORY ?? '';
  const name = repo.split('/')[1];
  return name ? `/${name}/` : '/';
}

export default defineConfig({
  lang: 'en-US',
  title: 'dup-code-check',
  description: 'Duplicate files / code span detection (Rust CLI)',
  base: resolveBase(),
  cleanUrls: true,

  outDir: '_book',

  themeConfig: {
    nav: [
      { text: 'Docs', link: '/introduction' },
      { text: '中文', link: '/introduction.zh-CN' },
    ],

    sidebar: [
      {
        text: 'English',
        items: [
          { text: 'Introduction', link: '/introduction' },
          { text: 'Getting Started', link: '/getting-started' },
          { text: 'Installation & Build', link: '/installation' },
          { text: 'CLI Usage', link: '/cli' },
          { text: 'Scan Options', link: '/scan-options' },
          { text: 'Detectors & Algorithms', link: '/detectors' },
          { text: 'Output & Report', link: '/output' },
          { text: 'CI Integration', link: '/ci' },
          { text: 'Performance & Scaling', link: '/performance' },
          { text: 'Architecture', link: '/architecture' },
          { text: 'Development', link: '/development' },
          { text: 'Contributing', link: '/contributing' },
          { text: 'Troubleshooting', link: '/troubleshooting' },
          { text: 'FAQ', link: '/faq' },
          { text: 'Roadmap', link: '/roadmap' },
          { text: 'Competitors', link: '/competitors' },
        ],
      },
      {
        text: '中文',
        items: [
          { text: '介绍', link: '/introduction.zh-CN' },
          { text: '快速开始', link: '/getting-started.zh-CN' },
          { text: '安装与构建', link: '/installation.zh-CN' },
          { text: 'CLI 使用', link: '/cli.zh-CN' },
          { text: '扫描选项', link: '/scan-options.zh-CN' },
          { text: '检测器与算法', link: '/detectors.zh-CN' },
          { text: '输出与报告', link: '/output.zh-CN' },
          { text: 'CI 集成', link: '/ci.zh-CN' },
          { text: '性能与可扩展性', link: '/performance.zh-CN' },
          { text: '架构', link: '/architecture.zh-CN' },
          { text: '开发指南', link: '/development.zh-CN' },
          { text: '贡献指南', link: '/contributing.zh-CN' },
          { text: '排障', link: '/troubleshooting.zh-CN' },
          { text: 'FAQ', link: '/faq.zh-CN' },
          { text: '路线图', link: '/roadmap.zh-CN' },
          { text: '竞品对比', link: '/competitors.zh-CN' },
        ],
      },
    ],

    socialLinks: [{ icon: 'github', link: 'https://github.com/omne42/dup-code-check' }],

    editLink: {
      pattern: 'https://github.com/omne42/dup-code-check/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },

    search: { provider: 'local' },

    footer: {
      message: 'Released under the MIT License.',
      copyright: `Copyright © ${new Date().getFullYear()} omne42`,
    },
  },
});

