import { NextResponse, type NextRequest } from "next/server";
import { randomUUID } from "node:crypto";
import { SESSION_COOKIE } from "@/lib/session";

// Every visitor has a session from the first response, as the Pleris server
// issues one. Creating it lazily on the first write raced: two presses before
// any cookie arrived made two sessions, and the second cookie won.
export function proxy(request: NextRequest) {
  const response = NextResponse.next();
  if (!request.cookies.has(SESSION_COOKIE)) {
    response.cookies.set(SESSION_COOKIE, randomUUID(), {
      path: "/",
      sameSite: "lax",
      httpOnly: true,
    });
  }
  return response;
}

export const config = { matcher: "/stores/:path*" };
