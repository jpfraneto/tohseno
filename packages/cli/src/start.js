import { delegate } from "./native.js";

export function startProduct(run = delegate, write = console.log) {
  write("Starting TOHSENO…");
  const service = run(["service", "install"]);
  if (service !== 0) return service;
  write("Opening TOHSENO…");
  return run(["studio"]);
}
