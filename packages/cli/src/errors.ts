export class CliError extends Error {
  readonly exitCode: number;

  constructor(message: string, exitCode = 1) {
    super(message);
    this.name = "CliError";
    this.exitCode = exitCode;
  }
}

export const MACHINE_EXIT = Object.freeze({
  success: 0,
  invalidConfiguration: 2,
  missingDependency: 3,
  unhealthyServices: 4,
  internalFailure: 5,
});

export type MachineExitCode = typeof MACHINE_EXIT[keyof typeof MACHINE_EXIT];

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
