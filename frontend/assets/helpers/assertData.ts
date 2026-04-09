// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyChecker = Record<string, (v: unknown) => v is any>;

/** Infers the validated type from a checker object — each predicate `v is T` contributes its `T`. */
export type Infer<C extends AnyChecker> = { [K in keyof C]: C[K] extends (v: unknown) => v is infer T ? T : never };

export function assertData<C extends AnyChecker>(label: string, x: unknown, checks: C): asserts x is Infer<C> {
    if (typeof x !== 'object' || x === null) throw new Error(`${label}: not an object`);
    const d = x as Record<string, unknown>;
    for (const [key, check] of Object.entries(checks)) {
        if (!check(d[key])) setTimeout(() => {throw new Error(`${label}: invalid field "${key}"`);}, 0);
    }
}

/** Common type-predicate helpers for use in checker objects. */
export const is = {
    string:   (v: unknown): v is string  => typeof v === 'string',
    boolean:  (v: unknown): v is boolean => typeof v === 'boolean',
    number:   (v: unknown): v is number  => typeof v === 'number',
    optional: <T>(check: (v: unknown) => v is T) =>
        (v: unknown): v is T | undefined => v === undefined || check(v),
    recordOf: <T>(check: (v: unknown) => v is T) =>
        (v: unknown): v is Record<string, T> =>
            typeof v === 'object' && v !== null && !Array.isArray(v) &&
            Object.values(v).every(check),
    oneOf: <const T extends readonly string[]>(...values: T) =>
        (v: unknown): v is T[number] =>
            typeof v === 'string' && (values as readonly string[]).includes(v),
};
