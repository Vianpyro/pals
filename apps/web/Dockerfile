# The web interface as static files (spec D8, D22). npm runs only in the build stage, which holds no
# secret or credential: a compromised package that ran there could steal nothing. Only dist/ leaves it.
# Built and attested by .github/workflows/images.yml from the repository root.
# Node: same version as .devcontainer/Dockerfile; Dependabot updates both.
FROM node:26.13.0-alpine AS build
WORKDIR /src

# Manifests first: the install layer is reused until a dependency changes
COPY package.json package-lock.json .npmrc ./
COPY apps/web/package.json apps/web/
COPY packages/contracts/package.json packages/contracts/
COPY packages/ui/package.json packages/ui/
RUN npm ci && npm audit signatures

COPY packages/contracts/bindings packages/contracts/bindings
COPY packages/ui packages/ui
COPY apps/web apps/web
RUN npm run check --workspaces --if-present \
    && npm test --workspaces --if-present \
    && npm run build -w apps/web \
    && npm run budget -w apps/web \
    && rm -r apps/web/dist/.vite

# Nothing runs from this image: the nginx role copies /srv/web onto the web VMs, where nginx serves it
FROM scratch
COPY --from=build /src/apps/web/dist /srv/web
