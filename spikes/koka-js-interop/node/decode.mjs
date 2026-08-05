// spike: koka-js-interop — the decoding boundary.
//
// Charter §7.10: "Every value entering from JSON, forms, headers, cookies,
// browser storage, databases, third-party APIs, or message queues starts as
// `Unknown` until decoded. Decoders must return `Result<T, DecodeError>`. Do not
// generate unchecked assertions."
//
// A value arriving from generated Koka JS is exactly such an external value from
// JavaScript's point of view: JS has no access to Koka's type checker. So the
// boundary gets a real decoder, not a cast.
//
// The rules this file obeys, and which the tests enforce:
//   1. Discriminate using Koka's EXPORTED predicates (`is_qok`, `is_draft`, ...),
//      never by reading `_tag` integers. See finding F-2: tag integers are
//      assigned by declaration order and silently renumber when a constructor is
//      inserted, so hardcoding them is precisely the "unchecked guessing" the
//      charter forbids.
//   2. Every field is checked for its runtime type before being accepted.
//   3. Failure is a returned value, never a thrown exception.
//   4. Anything the decoder cannot verify is reported, not assumed.

// --- the JS-side Result -----------------------------------------------------

export const Ok = (value) => ({ ok: true, value });
export const Err = (error) => ({ ok: false, error });

/** @param {string} path @param {string} expected @param {unknown} got */
export const decodeError = (path, expected, got) => ({
  kind: "DecodeError",
  path,
  expected,
  got: describe(got),
});

function describe(v) {
  if (v === null) return "null";
  if (Array.isArray(v)) return `array(${v.length})`;
  if (typeof v === "object") {
    const tag = Object.hasOwn(v, "_tag") ? `_tag=${v._tag}` : "no _tag";
    return `object{${Object.keys(v).join(",")}} (${tag})`;
  }
  return `${typeof v}(${String(v)})`;
}

// --- primitive decoders -----------------------------------------------------

export function decodeString(path, u) {
  return typeof u === "string" ? Ok(u) : Err(decodeError(path, "string", u));
}

export function decodeBool(path, u) {
  // Koka `bool` reaches JS as a real JS boolean. Verified, not assumed:
  // see test "koka bool arrives as a JS boolean".
  return typeof u === "boolean" ? Ok(u) : Err(decodeError(path, "boolean", u));
}

/**
 * Koka `int` is arbitrary precision. Small values arrive as JS numbers, large
 * values as BigInt. Both are accepted; anything else is rejected rather than
 * coerced. Charter §7.1 forbids implicit numeric coercion.
 */
export function decodeInt(path, u) {
  if (typeof u === "bigint") return Ok(u);
  if (typeof u === "number" && Number.isInteger(u)) return Ok(u);
  return Err(decodeError(path, "integer (number|bigint)", u));
}

export function decodeField(obj, path, key, inner) {
  if (typeof obj !== "object" || obj === null) {
    return Err(decodeError(path, "object", obj));
  }
  if (!Object.hasOwn(obj, key)) {
    return Err(decodeError(`${path}.${key}`, "present field", undefined));
  }
  return inner(`${path}.${key}`, obj[key]);
}

/**
 * Structural pre-check, applied BEFORE any Koka predicate.
 *
 * FINDING F-5 — Koka's generated `is_*` predicates are tag *discriminators*,
 * not *validators*. Two shapes of them exist and both are unsafe on unknown
 * input:
 *
 *   export function is_just(maybe)  { return (maybe !== null); }
 *   export function is_qok(qresult) { return (qresult._tag === 1); }
 *
 * So `is_just(42)` is `true`, `is_just("x")` is `true`, and `is_qok(null)`
 * *throws* a TypeError. They are only correct when the argument is already known
 * to be a value of that exact Koka type — which is precisely the knowledge JS
 * does not have at this boundary. Every decoder therefore confirms the shape
 * itself first and uses the predicate only to pick between confirmed variants.
 */
function isTaggedObject(u) {
  return typeof u === "object" && u !== null && !Array.isArray(u);
}

// --- Koka list ---------------------------------------------------------------

/**
 * Koka `list<a>` is a cons list: `Cons(head, tail)` / `Nil`, both `_tag`-shaped.
 * We discriminate with the std_core_types predicates rather than by tag.
 */
export function decodeList(path, u, decodeElem) {
  const out = [];
  let cur = u;
  let i = 0;
  // Guard against a malformed cyclic structure rather than hanging.
  const LIMIT = 1_000_000;
  while (i < LIMIT) {
    // `Nil === null`. Checked structurally; `is_nil` would say the same thing
    // but would also say it for any other null-valued Koka constructor.
    if (cur === null) return Ok(out);
    if (!isTaggedObject(cur) || !Object.hasOwn(cur, "head") || !Object.hasOwn(cur, "tail")) {
      return Err(decodeError(`${path}[${i}]`, "koka list cell (Cons{head,tail}|Nil)", cur));
    }
    const head = decodeElem(`${path}[${i}]`, cur.head);
    if (!head.ok) return head;
    out.push(head.value);
    cur = cur.tail;
    i += 1;
  }
  return Err(decodeError(path, `list shorter than ${LIMIT}`, u));
}

// --- domain decoders ---------------------------------------------------------

export function decodeStore(path, u) {
  const id = decodeField(u, path, "id", decodeString);
  if (!id.ok) return id;
  const name = decodeField(u, path, "name", decodeString);
  if (!name.ok) return name;
  return Ok({ id: id.value, name: name.value });
}

/**
 * NOTE (finding F-4): `money-usd` is a single-field Koka `value struct`, and
 * Koka ERASES it — `Money_usd(350)` returns the raw `350`. At this boundary a
 * `money-usd` is indistinguishable from any other integer. The decoder therefore
 * cannot verify the currency; it can only record that the caller asserted it.
 *
 * This is a real hole in the charter §7.1 "typed units / `Money<USD>`" goal when
 * Koka is the backend, and it is why the `pw` compiler must carry nominal type
 * information in its own manifest rather than relying on the Koka JS boundary.
 */
export function decodeMoneyUsd(path, u) {
  const n = decodeInt(path, u);
  if (!n.ok) return n;
  return Ok({ currency: "USD", minorUnits: n.value, _unverifiedNominalType: true });
}

export function decodeMenuItem(path, u) {
  const id = decodeField(u, path, "id", decodeString);
  if (!id.ok) return id;
  const name = decodeField(u, path, "name", decodeString);
  if (!name.ok) return name;
  const price = decodeField(u, path, "price", decodeMoneyUsd);
  if (!price.ok) return price;
  const available = decodeField(u, path, "available", decodeBool);
  if (!available.ok) return available;
  return Ok({
    id: id.value,
    name: name.value,
    price: price.value,
    available: available.value,
  });
}

/** `store-error` — shape-checked first, then discriminated by exported predicate. */
export function decodeStoreError(kk, path, u) {
  if (!isTaggedObject(u) || !Object.hasOwn(u, "_tag")) {
    return Err(decodeError(path, "store-error variant (tagged object)", u));
  }
  if (kk.is_storeNotFound(u)) {
    const id = decodeField(u, path, "id", decodeString);
    if (!id.ok) return id;
    return Ok({ kind: "StoreNotFound", id: id.value });
  }
  if (kk.is_storeUnavailable(u)) return Ok({ kind: "StoreUnavailable" });
  if (kk.is_storeDecodeFailed(u)) {
    const field = decodeField(u, path, "field", decodeString);
    if (!field.ok) return field;
    return Ok({ kind: "StoreDecodeFailed", field: field.value });
  }
  return Err(decodeError(path, "store-error variant", u));
}

/** `order-state` — exhaustive over the five declared variants. */
export function decodeOrderState(kk, path, u) {
  if (!isTaggedObject(u) || !Object.hasOwn(u, "_tag")) {
    return Err(decodeError(path, "order-state variant (tagged object)", u));
  }
  if (kk.is_draft(u)) return Ok({ kind: "Draft" });
  if (kk.is_pricing(u)) return Ok({ kind: "Pricing" });
  if (kk.is_confirmed(u)) {
    const c = decodeField(u, path, "confirmation", decodeString);
    return c.ok ? Ok({ kind: "Confirmed", confirmation: c.value }) : c;
  }
  if (kk.is_delivered(u)) {
    const r = decodeField(u, path, "receipt", decodeString);
    return r.ok ? Ok({ kind: "Delivered", receipt: r.value }) : r;
  }
  if (kk.is_cancelled(u)) {
    const r = decodeField(u, path, "reason", decodeString);
    return r.ok ? Ok({ kind: "Cancelled", reason: r.value }) : r;
  }
  // Charter §7.1: no unchecked cast, no silent default. An unrecognized variant
  // is an error naming what was actually seen.
  return Err(decodeError(path, "order-state variant", u));
}

/** `qresult<a,e>` — the charter's `Result<T,E>`. */
export function decodeQResult(kk, path, u, decodeOkValue, decodeErrValue) {
  if (!isTaggedObject(u) || !Object.hasOwn(u, "_tag")) {
    return Err(decodeError(path, "qresult variant (tagged object)", u));
  }
  if (kk.is_qok(u)) {
    const v = decodeField(u, path, "value", decodeOkValue);
    return v.ok ? Ok({ kind: "Ok", value: v.value }) : v;
  }
  if (kk.is_qerr(u)) {
    const e = decodeField(u, path, "error", (p, x) => decodeErrValue(kk, p, x));
    return e.ok ? Ok({ kind: "Err", error: e.value }) : e;
  }
  return Err(decodeError(path, "qresult variant (QOk|QErr)", u));
}

/**
 * `maybe<a>` — Koka's Option.
 *
 * FINDING F-7 — `Nothing === null` AND `Nil === null`. A bare `null` arriving at
 * this boundary is ambiguous: it is `Nothing`, or the empty list, or any other
 * null-represented Koka constructor. Runtime discrimination is IMPOSSIBLE; the
 * decoder must be told statically which type it is decoding. That is a direct
 * counterexample to charter §7.1's "no ambient `null`" goal at the JS boundary,
 * and it is why `pw` must emit its own type manifest rather than trusting the
 * shape of Koka's JS output.
 */
export function decodeMaybe(path, u, decodeSome) {
  if (u === null) return Ok({ kind: "None" });
  if (!isTaggedObject(u) || !Object.hasOwn(u, "value")) {
    return Err(decodeError(path, "maybe variant (Just{value}|Nothing===null)", u));
  }
  const v = decodeField(u, path, "value", decodeSome);
  return v.ok ? Ok({ kind: "Some", value: v.value }) : v;
}
