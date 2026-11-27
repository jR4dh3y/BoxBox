interface FooterLink {
  label: string;
  href: string;
  external?: boolean;
}

export const footerColumns: { title: string; links: FooterLink[] }[] = [
  {
    title: 'Get it',
    links: [
      { label: 'Download', href: '/download/' },
      { label: 'Desktop app', href: '/docs/desktop/' },
      { label: 'Quickstart', href: '/docs/quickstart/' },
      { label: 'Release notes', href: '/docs/release-notes/' },
    ],
  },
  {
    title: 'Docs',
    links: [
      { label: 'Overview', href: '/docs/' },
      { label: 'Configuration', href: '/docs/configuration/' },
      { label: 'API reference', href: '/docs/api/' },
      { label: 'Troubleshooting', href: '/docs/troubleshooting/' },
    ],
  },
  {
    title: 'Source',
    links: [
      { label: 'GitHub', href: 'https://github.com/jR4dh3y/BoxBox', external: true },
      { label: 'Releases', href: 'https://github.com/jR4dh3y/BoxBox/releases', external: true },
      { label: 'Report an issue', href: 'https://github.com/jR4dh3y/BoxBox/issues', external: true },
    ],
  },
];

export const footerProjects = [
  { name: 'fbs-core', href: 'https://fbs-landing.vercel.app/', about: 'S3-compatible blob storage and CDN' },
  { name: 'qckpages', href: 'https://qckpage.jr4.in/', about: 'Publish HTML files as pages in seconds' },
  { name: 'vigil-owl', href: 'https://vigil-owl.vercel.app/', about: 'Self-hosted NVR with timeline playback' },
];

export const footerTagline = 'Self-hosted, fast, and free. Your files stay where they are.';
export const footerLicense = 'MIT licensed';

export const footerLegalLinks: FooterLink[] = [
  { label: 'Privacy', href: '/privacy/' },
  { label: 'Terms', href: '/terms/' },
  { label: 'Security', href: '/security/' },
  { label: 'jR4dh3y', href: 'https://github.com/jR4dh3y', external: true },
];
