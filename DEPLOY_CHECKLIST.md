# Before the next deploy

Every push to `main` deploys to Cloud Run. Finish these first, then tick them off or delete them.

- [x] **Update c2pa-react-component.** Done: 0.1.23 is on npm and in `package.json`. `src/Details.jsx` uses the new `c2pa-card--fluid` class,
      which so far exists only in the local `../c2pa-react-component` (linked here with yalc).
      Commit it there, publish a new version to npm (first change its
      `c2pa-react-component-types` dependency from `file:.yalc/...` back to an npm version), and bump `c2pa-react-component` in
      `package.json` (then `npm install`, and `yalc link c2pa-react-component` again for local dev).
      Without it the deployed L2 card stays at 420px wide.
- [x] **Publish c2pa-react-component-nouns-verbs to npm and add it to `package.json`.**
      Done: 0.1.0 is on npm and in `package.json`; a clean `npm ci` + build passes.
