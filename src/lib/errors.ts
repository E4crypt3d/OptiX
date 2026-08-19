/// Convert an unknown rejection value (thrown by `invoke`) into a readable
/// message. Backend `OptixError`s serialize as `{ kind, message }`, so a bare
/// `String(e)` renders "[object Object]".
export function errMsg(e: unknown): string {
  if (e == null) return "Unknown error";
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (typeof e === "object") {
    const message = (e as { message?: unknown }).message;
    if (typeof message === "string" && message.length > 0) return message;
  }
  try {
    const json = JSON.stringify(e);
    if (json && json !== "{}") return json;
  } catch {
    // Fall through to String(e).
  }
  return String(e);
}
