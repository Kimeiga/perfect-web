import type { Cookies } from "@sveltejs/kit";
import { randomUUID } from "node:crypto";

export const SESSION_COOKIE = "session";

/** The request's session, if it has one. Reading never creates one. */
export function currentSession(cookies: Cookies): string | undefined {
  return cookies.get(SESSION_COOKIE);
}

/** The request's session, created on first write. */
export function ensureSession(cookies: Cookies): string {
  const existing = cookies.get(SESSION_COOKIE);
  if (existing) return existing;
  const session = randomUUID();
  cookies.set(SESSION_COOKIE, session, { path: "/", sameSite: "lax", httpOnly: true });
  return session;
}
