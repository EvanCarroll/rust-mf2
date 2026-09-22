// Item 5: the browser floor, by feature detection — what the `intl` option
// needs from Intl.NumberFormat (v3) and Intl.PluralRules, each probed by
// behaviour (the resolved option and a formatted result), never by version.

function tryMake(ctor, locale, options) {
  try { return new ctor(locale, options); } catch (e) { return null; }
}

function check(fn) {
  try { return Boolean(fn()); } catch (e) { return false; }
}

const NF = (o, l = 'en') => tryMake(Intl.NumberFormat, l, o);
const PR = (o, l = 'en') => tryMake(Intl.PluralRules, l, o);

export function floor() {
  const f = {
    // Intl.NumberFormat v3 (ECMA-402 2023): the options MF2 took from it.
    'NumberFormat formatToParts': check(() => typeof Intl.NumberFormat.prototype.formatToParts === 'function'),
    'NumberFormat formatRange (a v3 marker)': check(() => typeof Intl.NumberFormat.prototype.formatRange === 'function'),
    roundingIncrement: check(() => {
      const n = NF({ minimumFractionDigits: 2, maximumFractionDigits: 2, roundingIncrement: 5 });
      return n.resolvedOptions().roundingIncrement === 5 && n.format(1.23) === '1.25';
    }),
    roundingMode: check(() => {
      const n = NF({ maximumFractionDigits: 0, roundingMode: 'halfEven' });
      return n.resolvedOptions().roundingMode === 'halfEven' && n.format(2.5) === '2' && n.format(3.5) === '4';
    }),
    roundingPriority: check(() => {
      const n = NF({ maximumFractionDigits: 2, maximumSignificantDigits: 2, roundingPriority: 'lessPrecision' });
      return n.resolvedOptions().roundingPriority === 'lessPrecision' && n.format(1.234) === '1.2';
    }),
    trailingZeroDisplay: check(() => {
      const n = NF({ minimumFractionDigits: 2, trailingZeroDisplay: 'stripIfInteger' });
      return n.format(1) === '1' && n.format(1.5) === '1.50';
    }),
    "useGrouping: 'min2'": check(() => {
      const n = NF({ useGrouping: 'min2' });
      return n.resolvedOptions().useGrouping === 'min2' && n.format(1234) === '1234' && n.format(12345) === '12,345';
    }),
    "signDisplay: 'negative'": check(() => {
      const n = NF({ signDisplay: 'negative' });
      return n.format(-0) === '0' && n.format(-1) === '-1' && n.format(1) === '1';
    }),
    'maximumFractionDigits up to 100': check(() => NF({ maximumFractionDigits: 100 }) !== null),
    'exact decimal string, 17+ digits': check(() => NF({ useGrouping: false, maximumFractionDigits: 20 })
      .format('12345678901234567890.123456789') === '12345678901234567890.123456789'),
    'exact decimal string past 2^53': check(() => NF({ useGrouping: false }).format('9007199254740993') === '9007199254740993'),
    "negative zero from the string '-0'": check(() => NF({}).format('-0') === '-0'),
    'exponent strings (1.5e3)': check(() => NF({ useGrouping: false }).format('1.5e3') === '1500'),
    // Intl.PluralRules with the same digit options.
    'PluralRules digit options (1.0 ≠ 1)': check(() => PR({ minimumFractionDigits: 1, maximumFractionDigits: 1 }).select(1) === 'other'
      && PR({}).select(1) === 'one'),
    'PluralRules roundingIncrement': check(() => PR({ minimumFractionDigits: 1, maximumFractionDigits: 1, roundingIncrement: 5 }).resolvedOptions().roundingIncrement === 5),
    'PluralRules roundingMode': check(() => PR({ maximumFractionDigits: 0, roundingMode: 'floor' }).select(1.9) === 'one'),
    'PluralRules roundingPriority': check(() => PR({ maximumFractionDigits: 2, maximumSignificantDigits: 2, roundingPriority: 'lessPrecision' }).resolvedOptions().roundingPriority === 'lessPrecision'),
    'PluralRules trailingZeroDisplay': check(() => PR({ minimumFractionDigits: 2, trailingZeroDisplay: 'stripIfInteger' }).select(1) === 'one'),
    "PluralRules notation: 'compact' (CLDR's c/e operand)": check(() => PR({ notation: 'compact' }, 'fr').resolvedOptions().notation === 'compact'),
    'PluralRules.select of a decimal string (ToNumber)': check(() => PR({ minimumFractionDigits: 1, maximumFractionDigits: 1 }).select('1.5') === 'other'),
    // What :currency / :unit need.
    "currencyDisplay: 'narrowSymbol'": check(() => NF({ style: 'currency', currency: 'USD', currencyDisplay: 'narrowSymbol' }, 'en-CA').format(1) === '$1.00'),
    "currencySign: 'accounting'": check(() => NF({ style: 'currency', currency: 'USD', currencySign: 'accounting' }).format(-1) === '($1.00)'),
    "style: 'unit'": check(() => NF({ style: 'unit', unit: 'kilometer-per-hour' }).format(5) === '5 km/h'),
    "numberingSystem: 'latn' on ar": check(() => NF({ numberingSystem: 'latn' }, 'ar').format(123) === '123'),
  };
  const units = check(() => typeof Intl.supportedValuesOf === 'function') ? Intl.supportedValuesOf('unit') : null;
  return {
    features: f,
    missing: Object.entries(f).filter(([, ok]) => !ok).map(([k]) => k),
    sanctionedUnits: units ? units.length : null,
    units,
  };
}
