.PHONY: dev start test typecheck check migrate secrets operator

dev:
	bun run dev

start:
	bun run start

test:
	bun run test

typecheck:
	bun run typecheck

check:
	bun run check

migrate:
	bun run migrate

secrets:
	bun run generate-secrets

operator:
	bun run operator -- $(ARGS)
