#!/usr/bin/env bun
import { main } from "./cli.ts";

process.exitCode = await main(Bun.argv.slice(2));
