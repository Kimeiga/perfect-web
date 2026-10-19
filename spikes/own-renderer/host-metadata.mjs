// Data and projections only: importing a spec must never load Playwright's config.
import { createHash } from "node:crypto";
import { existsSync, lstatSync, readFileSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";

const own = (value, key) => Object.hasOwn(value, key);
const object = (value) => value !== null && typeof value === "object" && !Array.isArray(value);
const fail = (message) => { throw new Error(`browser host registry: ${message}`); };
const integer = (value, label, minimum = 0) => {
  if (!Number.isSafeInteger(value) || value < minimum || value > 65535) fail(`${label} must be an integer in ${minimum}..65535`);
};
const names = (value, label) => {
  if (!Array.isArray(value) || value.some((name) => typeof name !== "string" || !name)) fail(`${label} must contain names`);
  if (new Set(value).size !== value.length) fail(`${label} contains duplicate names`);
};

export function validateRegistry(value) {
  if (!object(value) || value.schema !== 1) fail("unsupported schema");
  integer(value.stride, "stride", 1);
  names(value.engines, "engines");
  if (!value.engines.length) fail("engines is empty");
  for (const field of ["builds", "roles", "mutable", "portExports", "specs"]) {
    if (!object(value[field])) fail(`${field} must be an object`);
  }
  for (const [name, build] of Object.entries(value.builds)) {
    if (build?.externalGeneratedSources !== undefined && typeof build.externalGeneratedSources !== "boolean") fail(`${name}.externalGeneratedSources must be boolean`);
    if (!object(build) || typeof build.prerequisite !== "string" || !build.prerequisite ||
        (build.dist !== null && (typeof build.dist !== "string" || !/^dist(?:-[a-z0-9-]+)?$/.test(build.dist)))) fail(`invalid build ${name}`);
  }
  const seen = new Map();
  for (const [name, role] of Object.entries(value.roles)) {
    if (!object(role) || !object(role.offsets)) fail(`invalid role ${name}`);
    const keys = Object.keys(role.offsets);
    if (!(keys.length === 1 && keys[0] === "shared") &&
        (keys.length !== value.engines.length || value.engines.some((engine) => !own(role.offsets, engine)))) fail(`${name} must declare shared or every engine offset`);
    for (const [engine, offset] of Object.entries(role.offsets)) {
      integer(offset, `${name}/${engine} offset`);
      if (offset >= value.stride) fail(`${name}/${engine} offset ${offset} escapes stride ${value.stride}`);
      if (seen.has(offset)) fail(`duplicate offset ${offset}: ${seen.get(offset)} and ${name}/${engine}`);
      seen.set(offset, `${name}/${engine}`);
    }
    if (role.auxiliary === true) {
      if (role.build !== null || role.provider !== undefined || role.depends !== undefined) fail(`invalid auxiliary role ${name}`);
    } else if (typeof role.build !== "string" || !own(value.builds, role.build)) fail(`${name} names an unknown build`);
    for (const flag of ["auxiliary", "reserved", "identity", "blobs"]) {
      if (role[flag] !== undefined && typeof role[flag] !== "boolean") fail(`${name}.${flag} must be boolean`);
    }
    if (role.depends !== undefined) {
      names(role.depends, `${name}.depends`);
      for (const dependency of role.depends) {
        if (!own(value.roles, dependency) || value.roles[dependency].auxiliary !== true) fail(`${name} names unknown auxiliary dependency ${dependency}`);
        const offsets = value.roles[dependency].offsets;
        if (!object(offsets) || keys.some((engine) => !own(offsets, engine))) fail(`${name} dependency ${dependency} has incompatible engines`);
      }
    }
    if (role.provider !== undefined && (typeof role.provider !== "string" || !role.depends?.includes(role.provider))) fail(`${name} provider is not a declared dependency`);
  }
  if (!seen.size) fail("roles is empty");
  for (const field of ["mutable", "portExports"]) {
    for (const [name, role] of Object.entries(value[field])) {
      if (typeof role !== "string" || !own(value.roles, role) || value.roles[role].auxiliary) fail(`${field}.${name} names unknown host role ${role}`);
    }
  }
  for (const [name, spec] of Object.entries(value.specs)) {
    if (!/^[a-z0-9-]+\.spec\.mjs$/.test(name) || !object(spec)) fail(`invalid spec ${name}`);
    if (spec.annotations === true && spec.roles === undefined) continue;
    names(spec.roles, `${name}.roles`);
    if (!spec.roles.length || spec.annotations !== undefined) fail(`conflicting roles for ${name}`);
    for (const role of spec.roles) if (!own(value.roles, role) || value.roles[role].auxiliary) fail(`${name} names unknown host role ${role}`);
  }
  return value;
}

function freeze(value) {
  if (value && typeof value === "object") {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
}

export const REGISTRY = freeze(validateRegistry(JSON.parse(readFileSync(new URL("./host-registry.json", import.meta.url), "utf8"))));
export const ENGINES = REGISTRY.engines;
export const STRIDE = REGISTRY.stride;
export const BUILDS = REGISTRY.builds;
export const HOST_ANNOTATION = "pw-host";
export const PORT = Number(process.env.PORT ?? 3141);

export function listenersForSlots(registry, base, count = 1) {
  validateRegistry(registry);
  integer(base, "PORT", 1);
  integer(count, "slot count", 1);
  const max = Math.max(...Object.values(registry.roles).flatMap((role) => Object.values(role.offsets)));
  if (base + (count - 1) * registry.stride + max > 65535) fail("PORT and slot count exceed TCP port 65535");
  return Array.from({ length: count }, (_, slot) => Object.entries(registry.roles).flatMap(([role, definition]) =>
    Object.entries(definition.offsets).map(([engine, offset]) => ({ slot, role, engine, port: base + slot * registry.stride + offset })))).flat();
}

export function portsForRole(role, base = PORT, registry = REGISTRY) {
  validateRegistry(registry);
  listenersForSlots(registry, base);
  if (!own(registry.roles, role)) fail(`unknown role ${role}`);
  return Object.fromEntries(Object.entries(registry.roles[role].offsets).map(([engine, offset]) => [engine, base + offset]));
}

export function rolesForTest(file, engine, annotations = [], registry = REGISTRY) {
  validateRegistry(registry);
  if (!registry.engines.includes(engine)) fail(`unknown project/engine ${engine}`);
  if (!own(registry.specs, file)) fail(`unregistered selected spec ${file}`);
  const spec = registry.specs[file];
  const written = annotations.filter((annotation) => annotation.type === HOST_ANNOTATION).map((annotation) => annotation.description);
  if (spec.annotations !== true && written.length) fail(`conflicting host annotation in ${file}`);
  const roles = spec.annotations === true ? written : spec.roles;
  names(roles, `${file} selected roles`);
  if (spec.annotations === true && roles.length !== 1) fail(`${file} requires exactly one static host annotation`);
  const closure = new Set();
  for (const role of roles) {
    if (!own(registry.roles, role) || registry.roles[role].auxiliary) fail(`unknown selected host role ${role}`);
    closure.add(role);
    for (const dependency of registry.roles[role].depends ?? []) closure.add(dependency);
  }
  return [...closure].sort();
}

const portExport = (name) => freeze(portsForRole(REGISTRY.portExports[name]));
export const MUTABLE_PORTS = freeze(Object.fromEntries(Object.entries(REGISTRY.mutable).map(([name, role]) => [name, portsForRole(role)])));
export const KIOKUN_PORT = portsForRole("legacy-kiokun").shared;
export const KEYED_PORTS = portExport("KEYED_PORTS");
export const FEED_PORTS = portExport("FEED_PORTS");
export const IDENTITY_PORTS = portExport("IDENTITY_PORTS");
export const UPLOADS_PORTS = portExport("UPLOADS_PORTS");
export const NOTIFICATIONS_PORTS = portExport("NOTIFICATIONS_PORTS");
export const MESSAGES_PORTS = portExport("MESSAGES_PORTS");
export const KIOKUN_PORTS = portExport("KIOKUN_PORTS");
export const TODOMVC_PORTS = portExport("TODOMVC_PORTS");
export const STORE_ACCOUNTS_PORTS = portExport("STORE_ACCOUNTS_PORTS");
export const STORE_SEARCH_PORTS = portExport("STORE_SEARCH_PORTS");
export const STORE_PAYMENTS_PORTS = portExport("STORE_PAYMENTS_PORTS");
export const SCHEDULED_ORDERS_PORTS = portExport("SCHEDULED_ORDERS_PORTS");
export const REVIEWS_PORTS = portExport("REVIEWS_PORTS");
export const GUESTBOOK_PORTS = portExport("GUESTBOOK_PORTS");

// Availability is information, never a reason for the collection phase to skip.
// The planner checks only the builds required by its selected role closure.
function canonicalMissingSource(path) {
  let ancestor = path;
  const suffix = [];
  while (!existsSync(ancestor)) {
    // A dangling link cannot prove that a generated input lies outside the
    // repository. Refuse rather than applying an exemption by written path.
    try { if (lstatSync(ancestor).isSymbolicLink()) throw new Error("dangling generated-source link"); }
    catch (error) { if (error.code !== "ENOENT") throw error; }
    suffix.unshift(basename(ancestor));
    const parent = dirname(ancestor);
    if (parent === ancestor) throw new Error("generated-source location cannot be resolved");
    ancestor = parent;
  }
  return resolve(realpathSync(ancestor), ...suffix);
}

export function buildProblem(name, root = fileURLToPath(new URL(".", import.meta.url))) {
  const build = BUILDS[name];
  if (!build) fail(`unknown build ${name}`);
  if (build.dist === null) return null;
  const directory = resolve(root, build.dist);
  const runtime = resolve(root, "public/pw-runtime.mjs");
  const problem = (why) => `${why}: run ${build.prerequisite}`;
  if (!existsSync(resolve(directory, "sources.json"))) return problem(`${build.dist} is no whole build (sources.json is missing)`);
  if (!existsSync(runtime) || !existsSync(resolve(directory, "pw-runtime.mjs")) || !readFileSync(runtime).equals(readFileSync(resolve(directory, "pw-runtime.mjs")))) return problem(`${build.dist} was built with another pw-runtime.mjs`);
  try {
    const { files } = JSON.parse(readFileSync(resolve(directory, "sources.json"), "utf8"));
    if (!Array.isArray(files) || files.some((file) => !object(file) || typeof file.path !== "string" || typeof file.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(file.sha256))) return problem(`${build.dist} has invalid source records`);
    for (const { path, sha256 } of files) {
      const source = resolve(root, path);
      if (!existsSync(source)) {
        if (build.externalGeneratedSources === true) {
          const repository = realpathSync(resolve(root, "../.."));
          const outside = relative(repository, canonicalMissingSource(source));
          if (outside === ".." || outside.startsWith("../") || outside.startsWith("..\\") || isAbsolute(outside)) continue;
        }
        return problem(`missing required source ${path}`);
      }
      if (createHash("sha256").update(readFileSync(source)).digest("hex") !== sha256) return problem(`${path} changed since ${build.dist} was built`);
    }
  } catch (error) {
    return problem(`${build.dist} source records cannot be read (${error.message})`);
  }
  if (!existsSync(resolve(directory, "build"))) return problem(`${build.dist} is not built`);
  return null;
}
