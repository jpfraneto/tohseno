export class RelayError extends Error {
  constructor(
    readonly status: number,
    message: string,
    readonly errorClass = "relay_invalid",
  ) {
    super(message);
  }
}
