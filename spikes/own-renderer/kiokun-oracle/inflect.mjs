// The rewrite's own forward conjugator, for the deinflection oracle's inputs
// (track `kiokun`, the deinflection ADR): a dictionary form and its JMdict
// part of speech give the inflected forms both sides are asked to read back.
// The facts are the ones the ADR cites (English Wikipedia's "Japanese
// conjugation", revision 1379736007; Wiktionary's Appendix:Japanese verbs,
// revision 92311215, and Appendix:Japanese adjectives, revision 91406689),
// in the rewrite's own structure; nothing here is kiokun.com's.

// A godan verb's bases by its final kana: the a-, i-, e- and o-row kana,
// and its te and ta forms.
const GODAN = {
  う: ["わ", "い", "え", "お", "って", "った"],
  く: ["か", "き", "け", "こ", "いて", "いた"],
  ぐ: ["が", "ぎ", "げ", "ご", "いで", "いだ"],
  す: ["さ", "し", "せ", "そ", "して", "した"],
  つ: ["た", "ち", "て", "と", "って", "った"],
  ぬ: ["な", "に", "ね", "の", "んで", "んだ"],
  ぶ: ["ば", "び", "べ", "ぼ", "んで", "んだ"],
  む: ["ま", "み", "め", "も", "んで", "んだ"],
  る: ["ら", "り", "れ", "ろ", "って", "った"],
};

// A verb's bases: { stem, irrealis, voice, continuative, hypothetical,
// imperative, volitional, te, ta, godan }, or null for a class not covered.
function verbBases(word, pos) {
  const last = word.slice(-1);
  const stem = word.slice(0, -1);
  if (pos.startsWith("v5")) {
    const row = GODAN[last];
    if (!row || pos === "v5aru" || pos === "v5r-i") return null;
    let [a, i, e, o, te, ta] = row;
    if (pos === "v5k-s") [te, ta] = ["って", "った"];
    if (pos === "v5u-s") [te, ta] = ["うて", "うた"];
    return { stem, irrealis: a, voice: a, continuative: i, hypothetical: e, imperative: e, volitional: `${o}う`, te, ta, godan: true };
  }
  if (pos === "v1" && last === "る") {
    return { stem, irrealis: "", voice: "", continuative: "", hypothetical: "れ", imperative: "ろ", volitional: "よう", te: "て", ta: "た", godan: false };
  }
  if (pos === "vk" && word.endsWith("来る")) {
    const s = word.slice(0, -2);
    return { stem: s, irrealis: "来", voice: "来", continuative: "来", hypothetical: "来れ", imperative: "来い", volitional: "来よう", te: "来て", ta: "来た", godan: false };
  }
  if (pos === "vk" && word.endsWith("くる")) {
    const s = word.slice(0, -2);
    return { stem: s, irrealis: "こ", voice: "こ", continuative: "き", hypothetical: "くれ", imperative: "こい", volitional: "こよう", te: "きて", ta: "きた", godan: false };
  }
  if (pos === "vs-i" && word.endsWith("する")) {
    return suru(word.slice(0, -2));
  }
  if (pos === "vs") {
    return suru(word);
  }
  return null;
}

function suru(stem) {
  return { stem, irrealis: "し", voice: "さ", continuative: "し", hypothetical: "すれ", imperative: "しろ", volitional: "しよう", te: "して", ta: "した", godan: true };
}

// The forms of a verb with bases `b`, each with what was done to it.
function verbForms(b) {
  const s = b.stem;
  const voice = b.godan ? ["れる", "せる"] : ["られる", "させる"];
  const forms = [
    [`${s}${b.ta}`, "past"],
    [`${s}${b.te}`, "te form"],
    [`${s}${b.irrealis}ない`, "negative"],
    [`${s}${b.irrealis}なかった`, "negative, past"],
    [`${s}${b.irrealis}なくて`, "negative, te form"],
    [`${s}${b.irrealis}なければ`, "negative, conditional (ば)"],
    [`${s}${b.continuative}ます`, "polite"],
    [`${s}${b.continuative}ました`, "polite, past"],
    [`${s}${b.continuative}ません`, "polite, negative"],
    [`${s}${b.continuative}ませんでした`, "polite, negative, past"],
    [`${s}${b.continuative}たい`, "desire"],
    [`${s}${b.continuative}たくない`, "desire, negative"],
    [`${s}${b.volitional}`, "volitional"],
    [`${s}${b.hypothetical}ば`, "conditional (ば)"],
    [`${s}${b.ta}ら`, "conditional (たら)"],
    [`${s}${b.imperative}`, "imperative"],
    [`${s}${b.voice}${voice[0]}`, "passive"],
    [`${s}${b.voice}${voice[0].slice(0, -1)}た`, "passive, past"],
    [`${s}${b.voice}${voice[1]}`, "causative"],
    [`${s}${b.voice}${voice[1].slice(0, -1)}られた`, "causative, passive, past"],
    [`${s}${b.te}いる`, "progressive"],
    [`${s}${b.te}いた`, "progressive, past"],
  ];
  if (b.godan && b.irrealis !== "し") forms.push([`${s}${b.hypothetical}る`, "potential"]);
  return forms;
}

// The forms of an i-adjective, each with what was done to it.
function adjectiveForms(word, pos) {
  if (!word.endsWith("い")) return [];
  const stem = pos === "adj-ix" && word.endsWith("いい") ? `${word.slice(0, -2)}よ` : word.slice(0, -1);
  return [
    [`${stem}かった`, "past"],
    [`${stem}くない`, "negative"],
    [`${stem}くなかった`, "negative, past"],
    [`${stem}くて`, "te form"],
    [`${stem}ければ`, "conditional (ば)"],
    [`${stem}く`, "adverb"],
  ];
}

// **The inflected forms of `word`**, a dictionary form of JMdict part of
// speech `pos`: [form, what was done].
export function inflections(word, pos) {
  if (pos === "adj-i" || pos === "adj-ix") return adjectiveForms(word, pos);
  const b = verbBases(word, pos);
  return b ? verbForms(b) : [];
}

// The parts of speech the conjugator covers.
export function covered(pos) {
  return pos === "adj-i" || pos === "adj-ix" || pos === "vk" || pos === "vs-i" || pos === "vs" || pos === "v1" || (pos.startsWith("v5") && pos !== "v5aru" && pos !== "v5r-i");
}
