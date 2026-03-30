import type { NextConfig } from "next";

const isProd = process.env.NODE_ENV === "production";

// On GitHub Pages the repo name is the sub-path:
// https://policyengine.github.io/policyengine-uk-rust/
const basePath = isProd ? "/policyengine-uk-rust" : "";

const nextConfig: NextConfig = {
  output: "export",
  basePath,
  // Images can't be optimised in static export mode
  images: { unoptimized: true },
};

export default nextConfig;
