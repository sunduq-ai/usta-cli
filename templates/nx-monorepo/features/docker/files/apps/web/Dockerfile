FROM node:20-alpine AS builder

WORKDIR /app
RUN corepack enable && corepack prepare pnpm@9.14.4 --activate

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json ./apps/web/
COPY packages/ ./packages/
RUN pnpm install --frozen-lockfile || pnpm install

COPY apps/web/ ./apps/web/
COPY tsconfig.base.json ./
RUN pnpm --filter web build

FROM nginx:alpine AS production
COPY --from=builder /app/apps/web/dist /usr/share/nginx/html
EXPOSE 80
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
    CMD wget --quiet --tries=1 --spider http://localhost/ || exit 1
CMD ["nginx", "-g", "daemon off;"]
