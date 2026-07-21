FROM oven/bun:1.2.18-alpine

WORKDIR /app

ENV NODE_ENV=production \
    PORT=3000 \
    HOME=/home/bun

COPY --chown=bun:bun package.json ./package.json
COPY --chown=bun:bun apps/site ./apps/site

USER bun

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["bun", "-e", "const port = process.env.PORT ?? '3000'; const response = await fetch('http://127.0.0.1:' + port + '/healthz'); if (!response.ok) process.exit(1);"]

CMD ["bun", "run", "start"]
