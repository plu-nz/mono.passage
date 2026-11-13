// Password generator: Bitwarden-style options, crypto-backed and unbiased.

export type GeneratorOptions = {
  length: number;
  upper: boolean;
  lower: boolean;
  numbers: boolean;
  special: boolean;
  avoidAmbiguous: boolean;
  minNumbers: number;
  minSpecial: number;
};

export const minLength = 5;
export const maxLength = 128;
export const maxMinimum = 9;
export const specialChars = '!@#$%^&*';

export const defaultOptions: GeneratorOptions = {
  length: 20, upper: true, lower: true, numbers: true, special: true, avoidAmbiguous: false, minNumbers: 1, minSpecial: 1,
};

function pools(options: GeneratorOptions) {
  const ambiguous = options.avoidAmbiguous;
  return {
    upper: ambiguous ? 'ABCDEFGHJKLMNPQRSTUVWXYZ' : 'ABCDEFGHIJKLMNOPQRSTUVWXYZ',
    lower: ambiguous ? 'abcdefghijkmnopqrstuvwxyz' : 'abcdefghijklmnopqrstuvwxyz',
    numbers: ambiguous ? '23456789' : '0123456789',
    special: specialChars,
  };
}

// randomIndex returns a uniform integer in [0, max) by rejecting the values
// that would make the modulo uneven.
function randomIndex(max: number): number {
  const limit = Math.floor(0x1_0000_0000 / max) * max;
  const buffer = new Uint32Array(1);
  do crypto.getRandomValues(buffer); while (buffer[0] >= limit);
  return buffer[0] % max;
}

// normalize keeps at least one character set on, clamps the counts, and makes
// the length cover every required character.
export function normalize(options: GeneratorOptions): GeneratorOptions {
  const next = { ...defaultOptions, ...options };
  if (!next.upper && !next.lower && !next.numbers && !next.special) next.lower = true;
  const clampCount = (value: number) => Math.min(maxMinimum, Math.max(0, Math.round(Number(value) || 0)));
  next.minNumbers = next.numbers ? Math.max(1, clampCount(next.minNumbers)) : 0;
  next.minSpecial = next.special ? Math.max(1, clampCount(next.minSpecial)) : 0;
  const required = Number(next.upper) + Number(next.lower) + next.minNumbers + next.minSpecial;
  next.length = Math.min(maxLength, Math.max(minLength, required, Math.round(Number(next.length) || 0)));
  return next;
}

export function generatePassword(input: GeneratorOptions): string {
  const options = normalize(input);
  const sets = pools(options);
  const chars: string[] = [];
  const pick = (pool: string) => pool[randomIndex(pool.length)];
  if (options.upper) chars.push(pick(sets.upper));
  if (options.lower) chars.push(pick(sets.lower));
  for (let i = 0; i < options.minNumbers; i++) chars.push(pick(sets.numbers));
  for (let i = 0; i < options.minSpecial; i++) chars.push(pick(sets.special));
  const all = (['upper', 'lower', 'numbers', 'special'] as const).filter(set => options[set]).map(set => sets[set]).join('');
  while (chars.length < options.length) chars.push(pick(all));
  for (let i = chars.length - 1; i > 0; i--) {
    const j = randomIndex(i + 1);
    [chars[i], chars[j]] = [chars[j], chars[i]];
  }
  return chars.join('');
}

const storageKey = 'passage.generator';

// Only the options are remembered, never a generated password.
export function loadOptions(): GeneratorOptions {
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey) || 'null');
    return normalize(saved && typeof saved === 'object' ? saved : defaultOptions);
  } catch {
    return { ...defaultOptions };
  }
}

export function saveOptions(options: GeneratorOptions): void {
  try { localStorage.setItem(storageKey, JSON.stringify(options)); } catch { /* per-device convenience only */ }
}
