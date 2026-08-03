export function insertText(current, addition, start = current.length, end = start) {
  if (!current) return { value: addition, cursor: addition.length };
  const selected = current.slice(start, end);
  if (selected) {
    const value = current.slice(0, start) + addition + current.slice(end);
    return { value, cursor: start + addition.length };
  }
  const needsBreak = start === current.length && !current.endsWith("\n\n");
  const prefix = needsBreak ? "\n\n" : "";
  const value = current.slice(0, start) + prefix + addition + current.slice(end);
  return { value, cursor: start + prefix.length + addition.length };
}

export function resolvePromptFile(current, incoming, action) {
  if (action === "replace") return incoming;
  if (action === "append") return current ? `${current.replace(/\s+$/, "")}\n\n${incoming.replace(/^\s+/, "")}` : incoming;
  throw new Error("Prompt file action must be replace or append.");
}

export function transferStateLabel(state) {
  return ({
    preparing: "Preparing package…", encrypting: "Encrypting on this device…",
    uploading: "Uploading encrypted chunks…", ready: "Your intention is ready.",
    waiting: "Waiting for TOHSENO…", leased: "Claimed by TOHSENO…",
    completed: "Imported on the Mac", expired: "This encrypted handoff expired.",
    cancelled: "This encrypted handoff was cancelled.", failed: "Encrypted handoff failed.",
  })[state] || "Waiting for TOHSENO…";
}
