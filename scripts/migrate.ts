import { loadConfig } from "../apps/site/config.ts";
import {
  migrateDatabase,
  openDatabase,
  type TohsenoDatabase,
} from "../apps/site/src/database.ts";

let database: TohsenoDatabase | undefined;

try {
  const config = loadConfig();
  database = openDatabase(config.databasePath);
  const result = migrateDatabase(database);

  console.log(JSON.stringify({
    event: "database.migrated",
    databasePath: config.databasePath,
    appliedMigrations: result.applied,
    appliedCount: result.applied.length,
  }));
} catch (error) {
  const message = error instanceof Error ? error.message : "Unknown migration failure";
  console.error(JSON.stringify({ event: "database.migration_failed", message }));
  process.exitCode = 1;
} finally {
  database?.close();
}
