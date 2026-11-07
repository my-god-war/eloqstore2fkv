// Browser-safe: Vite bundles the package metadata from the JSON file. Do not
// import node:path / node:fs here, this module is pulled into the client
// bundle by URL.js -> GIT_CDN.js when docs/blog load from raw GitHub.
import PKG from "../../package.json";

export default PKG;
