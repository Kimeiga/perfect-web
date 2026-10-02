import { cookies } from "next/headers";
import { randomUUID } from "node:crypto";

export const SESSION_COOKIE = "session";

/** The request's session, if it has one. Reading never creates one. */
export async function currentSession(): Promise<string | undefined> {
  return (await cookies()).get(SESSION_COOKIE)?.value;
}

/** The request's session, created on first write. Server Actions only. */
export async function ensureSession(): Promise<string> {
  const store = await cookies();
  const existing = store.get(SESSION_COOKIE)?.value;
  if (existing) return existing;
  const session = randomUUID();
  store.set(SESSION_COOKIE, session, { path: "/", sameSite: "lax", httpOnly: true });
  return session;
}
