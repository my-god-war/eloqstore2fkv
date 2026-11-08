// Build-time revision. The web workflow sets VITE_GIT_REV from GITHUB_SHA;
// local builds fall back to the `main` branch. Keep this module browser-safe:
// importing node:process / node:child_process here would break the client
// bundle when GIT_CDN.js is loaded by the SPA.
export default import.meta.env.VITE_GIT_REV || "main";
