import type { Handle } from "@sveltejs/kit";
import { ensureSession } from "$lib/server/session";

// Every visitor has a session from the first response, as the Pleris server
// issues one. Creating it lazily on the first write raced: two presses before
// any cookie arrived made two sessions, and the second cookie won.
export const handle: Handle = ({ event, resolve }) => {
  ensureSession(event.cookies);
  return resolve(event);
};
