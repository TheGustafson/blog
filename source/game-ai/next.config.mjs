import createMDX from '@next/mdx';
import remarkMath from 'remark-math';
import remarkGfm from 'remark-gfm';
import rehypeKatex from 'rehype-katex';
import rehypeSlug from 'rehype-slug';
import rehypePrettyCode from 'rehype-pretty-code';

// The publish script sets PUBLISH_BLOG=1 and NEXT_PUBLIC_BASE_PATH="" — the
// site serves at the root of thegustafson.com (custom domain), so there is no
// basePath anymore. The plumbing stays in case the site ever moves back under
// a subpath. Local dev leaves both unset.
const basePath = process.env.PUBLISH_BLOG === '1' ? (process.env.NEXT_PUBLIC_BASE_PATH || '') : '';

/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'export',
  basePath,
  assetPrefix: basePath || undefined,
  pageExtensions: ['js', 'jsx', 'md', 'mdx', 'ts', 'tsx'],
  images: { unoptimized: true },
  experimental: {
    // Lowers peak memory during webpack compilation; slower builds but
    // viable on memory-constrained machines with a hundred-plus MDX pages.
    webpackMemoryOptimizations: true,
    // Make the memory-saving build worker explicit and keep every production
    // build inside the same bounded worker budget. This matters locally too:
    // a preview build compiles the same hundred-plus MDX pages as publish.
    webpackBuildWorker: true,
    cpus: 2,
  },
};

/** @type {import('rehype-pretty-code').Options} */
const options = {
  theme: 'github-light',
  keepBackground: false,
};

const withMDX = createMDX({
  options: {
    remarkPlugins: [remarkMath, remarkGfm],
    rehypePlugins: [rehypeSlug, rehypeKatex, [rehypePrettyCode, options]],
  },
});

export default withMDX(nextConfig);
