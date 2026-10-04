// ADR-0182, charter §15.6 test 14: the rules of keyboard and screen-reader
// semantics a test can decide, read in the page as the engine parsed, laid
// out and computed it.
//
// Each rule is axe-core's, or WCAG 2.2's at level AA, under the same name:
// what the document, its landmarks, headings and lists are; ids and what
// names them; ARIA's attributes, values and roles; what names each control
// and whether its name holds the words it shows; target size; contrast.
// Two are this page's own, and say so. axe-core itself is not run here: it
// is not installed, and installing it is the owner's call (ADR-0182).

/** Every way the document breaks a rule, as `rule: element detail`; none,
 * when it breaks none. Self-contained, as `page.evaluate` sends its source. */
export function violations() {
  const out = [];
  const describe = (el) => {
    if (!(el instanceof Element)) return "the document";
    const label = el.getAttribute("aria-label");
    return `<${el.localName}${el.id ? `#${el.id}` : ""}${label ? ` "${label}"` : ""}>`;
  };
  const say = (rule, el, detail) =>
    out.push(`${rule}: ${describe(el)}${detail ? ` ${detail}` : ""}`);
  const all = [...document.body.querySelectorAll("*")];
  const squash = (s) => s.replace(/\s+/g, " ").trim();

  // Shown to someone: rendered, and not hidden from assistive technology.
  // What `content-visibility: auto` skips is shown: it is in the
  // accessibility tree.
  const shown = (el) =>
    el.checkVisibility({ visibilityProperty: true }) && !el.closest('[aria-hidden="true"]');

  // The text an element's content gives a name (accname 1.2, step 2F): its
  // text, and the names of the elements in it.
  const contentText = (el) =>
    [...el.childNodes]
      .map((n) => {
        if (n.nodeType === Node.TEXT_NODE) return n.data;
        if (n.nodeType !== Node.ELEMENT_NODE) return "";
        if (!n.checkVisibility({ visibilityProperty: true })) return "";
        if (n.getAttribute("aria-hidden") === "true") return "";
        if (n.hasAttribute("aria-label")) return ` ${n.getAttribute("aria-label")} `;
        if (n.localName === "img") return ` ${n.getAttribute("alt") ?? ""} `;
        return ` ${contentText(n)} `;
      })
      .join("");
  const byIds = (el, attr) =>
    (el.getAttribute(attr) ?? "")
      .split(/\s+/)
      .filter(Boolean)
      .map((id) => [id, document.getElementById(id)]);
  const FROM_CONTENT = /^(button|a|summary|h[1-6]|td|th|option|legend|caption)$/;
  const ROLE_FROM_CONTENT =
    /^(button|link|tab|menuitem|menuitemcheckbox|menuitemradio|option|heading|cell|checkbox|radio|switch|treeitem)$/;
  const accessibleName = (el) => {
    const named = byIds(el, "aria-labelledby").filter(([, t]) => t);
    if (named.length) {
      return squash(named.map(([, t]) => t.getAttribute("aria-label") ?? contentText(t)).join(" "));
    }
    const label = squash(el.getAttribute("aria-label") ?? "");
    if (label) return label;
    if (el.labels?.length) return squash([...el.labels].map(contentText).join(" "));
    if (el.localName === "input" && ["button", "submit", "reset"].includes(el.type)) {
      return squash(el.value);
    }
    if (el.localName === "img" || (el.localName === "input" && el.type === "image")) {
      return squash(el.getAttribute("alt") ?? "");
    }
    if (FROM_CONTENT.test(el.localName) || ROLE_FROM_CONTENT.test(el.getAttribute("role") ?? "")) {
      const text = squash(contentText(el));
      if (text) return text;
    }
    return squash(el.getAttribute("title") ?? "");
  };

  // --- the document -----------------------------------------------------
  const lang = document.documentElement.lang;
  if (!/^[a-zA-Z]{2,3}(-[a-zA-Z0-9]{1,8})*$/.test(lang)) say("html-lang-valid", document, `"${lang}"`);
  if (!squash(document.title)) say("document-title", document);
  // Laid out at a phone's width, and zoomed as far as a person needs.
  const viewport = document.querySelector('meta[name="viewport"]');
  if (!viewport) {
    say("meta-viewport", document, "has none");
  } else {
    const content = Object.fromEntries(
      viewport.content.split(/[,;]/).map((p) => p.split("=").map((s) => s.trim().toLowerCase())),
    );
    if (content.width !== "device-width") say("meta-viewport", viewport, "is not the device's width");
    if (["no", "0"].includes(content["user-scalable"])) say("meta-viewport", viewport, "stops zoom");
    if (content["maximum-scale"] !== undefined && !(Number(content["maximum-scale"]) >= 5)) {
      say("meta-viewport-large", viewport, "limits zoom");
    }
  }

  // --- landmarks and regions ------------------------------------------------
  const mains = all.filter(
    (el) => (el.localName === "main" || el.getAttribute("role") === "main") && shown(el),
  );
  if (mains.length !== 1) say("landmark-one-main", document, `has ${mains.length}`);
  const regions = new Map();
  for (const r of all) {
    if (!(r.localName === "section" || r.getAttribute("role") === "region") || !shown(r)) continue;
    // This page's own: every section is a region a reader can go to by name.
    const name = accessibleName(r);
    if (!name) say("region-name", r);
    else if (regions.has(name)) say("landmark-unique", r, `shares "${name}"`);
    else regions.set(name, r);
  }

  // --- headings -------------------------------------------------------------
  const level = (h) => Number(h.getAttribute("aria-level") ?? h.localName.slice(1));
  const headings = all.filter(
    (el) => (/^h[1-6]$/.test(el.localName) || el.getAttribute("role") === "heading") && shown(el),
  );
  const ones = headings.filter((h) => level(h) === 1).length;
  if (ones !== 1) say("page-has-heading-one", document, `has ${ones}`);
  let previous = 0;
  for (const h of headings) {
    if (!squash(contentText(h))) say("empty-heading", h);
    if (level(h) > previous + 1) say("heading-order", h, `h${level(h)} after h${previous}`);
    previous = level(h);
  }

  // --- lists ----------------------------------------------------------------
  for (const el of all) {
    if (el.localName === "li" && !["ul", "ol", "menu"].includes(el.parentElement.localName)) {
      say("listitem", el);
    }
    if (el.localName === "ul" || el.localName === "ol") {
      for (const child of el.children) {
        if (!["li", "script", "template"].includes(child.localName)) {
          say("list", el, `holds <${child.localName}>`);
        }
      }
    }
  }

  // --- ids, and what names them ---------------------------------------------
  const ids = new Set();
  for (const el of document.querySelectorAll("[id]")) {
    if (ids.has(el.id)) say("duplicate-id", el);
    ids.add(el.id);
  }
  const IDREFS = [
    "aria-labelledby",
    "aria-describedby",
    "aria-controls",
    "aria-owns",
    "aria-flowto",
    "aria-details",
    "aria-errormessage",
    "aria-activedescendant",
  ];
  for (const el of all) {
    for (const attr of IDREFS) {
      for (const [id, target] of byIds(el, attr)) {
        if (!target) say("aria-valid-attr-value", el, `${attr} names "${id}", which nothing is`);
      }
    }
    if ((el.localName === "label" || el.localName === "output") && el.hasAttribute("for")) {
      for (const [id, target] of byIds(el, "for")) {
        if (!target) say("label", el, `for names "${id}", which nothing is`);
      }
    }
  }

  // --- ARIA's attributes, their values, and roles (WAI-ARIA 1.2) ------------
  const BOOL = ["true", "false"];
  const OPTIONAL = ["true", "false", "undefined"];
  const TRISTATE = ["true", "false", "mixed", "undefined"];
  const ARIA = {
    "aria-activedescendant": null,
    "aria-atomic": BOOL,
    "aria-autocomplete": ["inline", "list", "both", "none"],
    "aria-braillelabel": null,
    "aria-brailleroledescription": null,
    "aria-busy": BOOL,
    "aria-checked": TRISTATE,
    "aria-colcount": "integer",
    "aria-colindex": "integer",
    "aria-colindextext": null,
    "aria-colspan": "integer",
    "aria-controls": null,
    "aria-current": ["page", "step", "location", "date", "time", "true", "false"],
    "aria-describedby": null,
    "aria-description": null,
    "aria-details": null,
    "aria-disabled": BOOL,
    "aria-dropeffect": null,
    "aria-errormessage": null,
    "aria-expanded": OPTIONAL,
    "aria-flowto": null,
    "aria-grabbed": OPTIONAL,
    "aria-haspopup": ["false", "true", "menu", "listbox", "tree", "grid", "dialog"],
    "aria-hidden": OPTIONAL,
    "aria-invalid": ["grammar", "false", "spelling", "true"],
    "aria-keyshortcuts": null,
    "aria-label": null,
    "aria-labelledby": null,
    "aria-level": "integer",
    "aria-live": ["off", "polite", "assertive"],
    "aria-modal": BOOL,
    "aria-multiline": BOOL,
    "aria-multiselectable": BOOL,
    "aria-orientation": ["horizontal", "vertical", "undefined"],
    "aria-owns": null,
    "aria-placeholder": null,
    "aria-posinset": "integer",
    "aria-pressed": TRISTATE,
    "aria-readonly": BOOL,
    "aria-relevant": "relevant",
    "aria-required": BOOL,
    "aria-roledescription": null,
    "aria-rowcount": "integer",
    "aria-rowindex": "integer",
    "aria-rowindextext": null,
    "aria-rowspan": "integer",
    "aria-selected": OPTIONAL,
    "aria-setsize": "integer",
    "aria-sort": ["ascending", "descending", "none", "other"],
    "aria-valuemax": "number",
    "aria-valuemin": "number",
    "aria-valuenow": "number",
    "aria-valuetext": null,
  };
  const ROLES = new Set(
    (
      "alert alertdialog application article banner blockquote button caption cell checkbox " +
      "code columnheader combobox complementary contentinfo definition deletion dialog document " +
      "emphasis feed figure form generic grid gridcell group heading img insertion link list " +
      "listbox listitem log main mark marquee math menu menubar menuitem menuitemcheckbox " +
      "menuitemradio meter navigation none note option paragraph presentation progressbar radio " +
      "radiogroup region row rowgroup rowheader scrollbar search searchbox separator slider " +
      "spinbutton status strong subscript superscript switch tab table tablist tabpanel term " +
      "textbox time timer toolbar tooltip tree treegrid treeitem"
    ).split(" "),
  );
  // A name is prohibited on these roles: what names one is read by nothing.
  const NAMELESS = new Set([
    "caption",
    "code",
    "deletion",
    "emphasis",
    "generic",
    "insertion",
    "paragraph",
    "presentation",
    "none",
    "strong",
    "subscript",
    "superscript",
  ]);
  const IMPLICIT = {
    div: "generic",
    span: "generic",
    b: "generic",
    i: "generic",
    u: "generic",
    s: "generic",
    small: "generic",
    p: "paragraph",
    code: "code",
    del: "deletion",
    em: "emphasis",
    ins: "insertion",
    strong: "strong",
    sub: "subscript",
    sup: "superscript",
  };
  for (const el of all) {
    for (const { name, value } of el.attributes) {
      if (!name.startsWith("aria-")) continue;
      if (!(name in ARIA)) {
        say("aria-valid-attr", el, name);
        continue;
      }
      const allowed = ARIA[name];
      const v = value.trim();
      const ok =
        allowed === null ||
        (Array.isArray(allowed) && allowed.includes(v)) ||
        (allowed === "integer" && /^-?\d+$/.test(v)) ||
        (allowed === "number" && v !== "" && Number.isFinite(Number(v))) ||
        (allowed === "relevant" &&
          v.split(/\s+/).every((t) => ["additions", "removals", "text", "all"].includes(t)));
      if (!ok) say("aria-valid-attr-value", el, `${name}="${value}"`);
    }
    const role = el.getAttribute("role")?.trim().split(/\s+/)[0];
    if (role !== undefined && !ROLES.has(role)) say("aria-roles", el, `role="${role}"`);
    if (el.hasAttribute("aria-label") || el.hasAttribute("aria-labelledby")) {
      const its = role ?? IMPLICIT[el.localName];
      if (NAMELESS.has(its)) say("aria-prohibited-attr", el, `names a ${its}`);
    }
  }

  // --- what is focused and pressed ------------------------------------------
  const INTERACTIVE =
    'button, a[href], input:not([type="hidden"]), select, textarea, summary, [tabindex], ' +
    '[contenteditable=""], [contenteditable="true"], [role="button"], [role="link"], ' +
    '[role="checkbox"], [role="radio"], [role="switch"], [role="tab"], [role="menuitem"], ' +
    '[role="option"], [role="textbox"], [role="combobox"], [role="slider"], [role="spinbutton"]';
  // A heading the page moves focus to, `tabindex="-1"`, is a place for focus
  // to go, not a control (ADR-0172).
  const control = (el) =>
    el.matches(INTERACTIVE) &&
    !(el.getAttribute("tabindex") === "-1" && !el.matches("button, a[href], input, select, textarea, summary, [role]"));
  const controls = all.filter(control);
  const names = new Map();
  for (const el of all.filter((e) => e.matches(INTERACTIVE))) {
    const tabindex = el.getAttribute("tabindex");
    if (tabindex !== null && Number(tabindex) > 0) say("tabindex", el, `"${tabindex}"`);
    const focusable = !el.matches(":disabled") && (tabindex === null || Number(tabindex) >= 0);
    if (focusable && el.closest('[aria-hidden="true"]')) say("aria-hidden-focus", el);
    if (el.parentElement?.closest('button, a[href], [role="button"], [role="link"]')) {
      say("nested-interactive", el);
    }
  }
  for (const el of controls.filter(shown)) {
    const name = accessibleName(el);
    if (!name) {
      say("button-name", el);
      continue;
    }
    // Label in name (WCAG 2.5.3): the words a control shows are in its
    // name, so a person who says what they see is understood. A symbol, `−`
    // or `+`, is no words.
    const words = squash(contentText(el)).toLowerCase();
    if (/[\p{L}\p{N}]/u.test(words) && !name.toLowerCase().includes(words)) {
      say("label-content-name-mismatch", el, `shows "${words}"`);
    }
    // This page's own (ADR-0168): two buttons of one name are told apart by
    // nothing in a list of the page's buttons.
    if (el.matches('button, [role="button"]')) {
      if (names.has(name)) say("button-name-unique", el, `shares "${name}"`);
      names.set(name, el);
    }
  }

  // --- the size of what is pressed (WCAG 2.5.8) -------------------------------
  // At least 24 CSS pixels each way, or apart: a circle 24 across at its
  // centre meets no other target, nor another small one's circle.
  const targets = controls.filter(shown).map((el) => ({ el, r: el.getBoundingClientRect() }));
  const small = (t) => t.r.width < 24 || t.r.height < 24;
  const centre = (t) => [t.r.left + t.r.width / 2, t.r.top + t.r.height / 2];
  const toRect = ([x, y], r) =>
    Math.hypot(Math.max(r.left - x, 0, x - r.right), Math.max(r.top - y, 0, y - r.bottom));
  for (const t of targets.filter(small)) {
    const [x, y] = centre(t);
    const near = targets.find((o) => {
      if (o === t) return false;
      if (toRect([x, y], o.r) < 12) return true;
      const [ox, oy] = centre(o);
      return small(o) && Math.hypot(x - ox, y - oy) < 24;
    });
    if (near) {
      say("target-size", t.el, `${Math.round(t.r.width)}x${Math.round(t.r.height)}, by ${describe(near.el)}`);
    }
  }

  // --- contrast (WCAG 1.4.3) ------------------------------------------------
  const rgba = (s) => {
    const m = s.match(
      /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/,
    );
    if (!m) return null;
    const a = m[4] === undefined ? 1 : m[4].endsWith("%") ? parseFloat(m[4]) / 100 : parseFloat(m[4]);
    return [Number(m[1]), Number(m[2]), Number(m[3]), a];
  };
  const over = ([r, g, b, a], [R, G, B]) => [
    r * a + R * (1 - a),
    g * a + G * (1 - a),
    b * a + B * (1 - a),
  ];
  const luminance = (rgb) =>
    rgb
      .map((v) => v / 255)
      .map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
      .reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i], 0);
  // The colour behind an element: its own and its ancestors', over the white
  // canvas. Null where it cannot be read here: an image behind it, or
  // something translucent.
  const behind = (el) => {
    const layers = [];
    for (let e = el; e; e = e.parentElement) {
      const s = getComputedStyle(e);
      if (s.backgroundImage !== "none" || Number(s.opacity) < 1) return null;
      const c = rgba(s.backgroundColor);
      if (c === null) return null;
      if (c[3] > 0) layers.push(c);
      if (c[3] === 1) break;
    }
    return layers.reduceRight((under, c) => over(c, under), [255, 255, 255]);
  };
  const read = new Set();
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const el = n.parentElement;
    if (!n.data.trim() || read.has(el)) continue;
    if (["script", "style", "noscript", "template"].includes(el.localName)) continue;
    read.add(el);
    if (!el.checkVisibility({ visibilityProperty: true }) || el.closest(":disabled")) continue;
    const bg = behind(el);
    const fg = rgba(getComputedStyle(el).color);
    if (bg === null || fg === null) {
      say("color-contrast", el, "cannot be read here");
      continue;
    }
    const [hi, lo] = [luminance(over(fg, bg)), luminance(bg)].sort((a, b) => b - a);
    const ratio = (hi + 0.05) / (lo + 0.05);
    const s = getComputedStyle(el);
    const size = parseFloat(s.fontSize);
    const large = size >= 24 || (size >= 18.66 && Number(s.fontWeight) >= 700);
    if (ratio < (large ? 3 : 4.5)) say("color-contrast", el, `${ratio.toFixed(2)}:1`);
  }
  return out;
}

/** The controls a keyboard reaches, in the order the document has them:
 * what Tab should visit. */
export function tabStops() {
  return [...document.body.querySelectorAll("button, a[href], input, select, textarea, summary, [tabindex]")]
    .filter(
      (el) =>
        !el.matches(":disabled") &&
        el.getAttribute("tabindex") !== "-1" &&
        el.checkVisibility({ visibilityProperty: true }),
    )
    .map((el) => el.getAttribute("aria-label") ?? el.textContent.replace(/\s+/g, " ").trim());
}
