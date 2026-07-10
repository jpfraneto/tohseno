FROM oven/bun:1.2.18-alpine AS dependencies

WORKDIR /app

COPY package.json bun.lock ./
RUN bun install --frozen-lockfile --production

FROM oven/bun:1.2.18-alpine AS runtime

WORKDIR /app

ENV NODE_ENV=production \
    PORT=3000 \
    DATABASE_PATH=/data/tohseno.sqlite \
    HOME=/home/bun

COPY --from=dependencies --chown=bun:bun /app/node_modules ./node_modules
COPY --chown=bun:bun package.json ./package.json
COPY --chown=bun:bun apps/site ./apps/site
COPY --chown=bun:bun scripts ./scripts

RUN apk add --no-cache su-exec \
    && mkdir -p /data \
    && chown bun:bun /data \
    && chmod 0755 /app/scripts/container-entrypoint.sh

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["bun", "-e", "const port = process.env.PORT ?? '3000'; const response = await fetch('http://127.0.0.1:' + port + '/healthz'); if (!response.ok) process.exit(1);"]

ENTRYPOINT ["/app/scripts/container-entrypoint.sh"]
CMD ["bun", "run", "start"]
