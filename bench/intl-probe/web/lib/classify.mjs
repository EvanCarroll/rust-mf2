// Classifying the difference between two formatted strings (items 3 and 6).
//
// In order, the first that applies:
//   bidi-marks   equal once U+200E/U+200F/U+061C/U+2066–U+2069 are removed
//   space        equal once every space character (any Zs, NBSP, NNBSP, TAB) is U+0020
//   numbering    equal once every decimal digit of any script is mapped to 0–9
//   digits       the digit sequences differ (rounding, digit count, grouping of digits aside)
//   symbols      same digits, different other characters (minus/plus signs, separators,
//                currency or unit symbols and names, word order)

const BIDI = /[\u200e\u200f\u061c\u2066-\u2069]/gu;
const SPACE = /[\s\u00a0\u202f\u2007\u2009]/gu;

// The zero of each decimal-digit run the engines' numbering systems use
// (latn, arab, arabext, nkoo, deva, beng, guru, gujr, orya, tamldec, telu,
// knda, mlym, sinh, thai, laoo, tibt, mymr, mymrshan, khmr, mong, fullwide).
const ZEROS = [0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66,
  0xce6, 0xd66, 0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0xff10];

/** Maps the decimal digits of those systems to ASCII. */
export function asciiDigits(s) {
  return s.replace(/\p{Nd}/gu, (d) => {
    const c = d.codePointAt(0);
    const z = ZEROS.find((z) => c >= z && c <= z + 9);
    return z === undefined ? d : String(c - z);
  });
}

export function classify(a, b) {
  if (a === b) return 'same';
  const na = a.replace(BIDI, ''), nb = b.replace(BIDI, '');
  if (na === nb) return 'bidi-marks';
  const sa = na.replace(SPACE, ' '), sb = nb.replace(SPACE, ' ');
  if (sa === sb) return 'space';
  const da = asciiDigits(sa), db = asciiDigits(sb);
  if (da === db) return 'numbering';
  const ga = da.replace(/\D/g, ''), gb = db.replace(/\D/g, '');
  if (ga !== gb) return 'digits';
  return 'symbols';
}

/** A tally with a few examples per class. */
export class Tally {
  constructor(keep = 8) { this.keep = keep; this.counts = {}; this.examples = {}; }
  add(cls, example) {
    this.counts[cls] = (this.counts[cls] ?? 0) + 1;
    const list = (this.examples[cls] ??= []);
    if (example !== undefined && list.length < this.keep) list.push(example);
  }
  toJSON() { return { counts: this.counts, examples: this.examples }; }
}
