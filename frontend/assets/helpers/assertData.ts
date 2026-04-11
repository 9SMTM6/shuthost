import { showJSError } from "./utils";

// biome-ignore lint/suspicious/noExplicitAny: Any is not problematic (and IIRC actually needed because of covariance) in this context.
type Checker<T = unknown> = (v: unknown) => v is T;
type AnyChecker = Record<string, Checker<any>>;

/** Infers the validated type from a checker predicate or object checker map. */
export type Infer<C> =
    C extends Checker<infer T>
        ? T
        : C extends AnyChecker
        ? {
              [K in keyof C]: C[K] extends Checker<infer T> ? T : never;
          }
        : never;

export function validateData<T>(
    label: string,
    x: unknown,
    check: Checker<T>,
): asserts x is T {
    if (!check(x)) {
        // Defer error display until the page is rendered.
        setTimeout(() => {
            showJSError(`Data validation error: ${label} is invalid`);
        });
        console.error(`${label} validation failed`, x);
    }
}

/** Common type-predicate helpers for use in checker objects. */
export const is = {
    string: (v: unknown): v is string => typeof v === 'string',
    boolean: (v: unknown): v is boolean => typeof v === 'boolean',
    number: (v: unknown): v is number => typeof v === 'number',
    optional:
        <T>(check: (v: unknown) => v is T) =>
        (v: unknown): v is T | undefined =>
            v === undefined || check(v),
    recordOf:
        <T>(check: (v: unknown) => v is T) =>
        (v: unknown): v is Record<string, T> =>
            typeof v === 'object' &&
            v !== null &&
            !Array.isArray(v) &&
            Object.values(v).every(check),
    object:
        <C extends AnyChecker>(checks: C) =>
        (v: unknown): v is Infer<C> =>
            typeof v === 'object' &&
            v !== null &&
            !Array.isArray(v) &&
            Object.entries(checks).every(([key, check]) =>
                check((v as Record<string, unknown>)[key]),
            ),
    oneOf:
        <const T extends readonly string[]>(...values: T) =>
        (v: unknown): v is T[number] =>
            typeof v === 'string' && (values as readonly string[]).includes(v),
};
