import { showJSError } from './utils';

type Checker<T = never> = (v: unknown) => v is T;
type Literal = string | number | boolean;
type ComplexCheckerElement = Checker<unknown> | Literal | null | undefined;
type StructChecker = Record<string, ComplexCheckerElement>;
type InferChecked<T> = T extends Checker<infer U> ? U : T;
type OptionalObjectKeys<C extends StructChecker> = {
    [K in keyof C]: undefined extends InferChecked<C[K]> ? K : never;
}[keyof C];
type RequiredObjectKeys<C extends StructChecker> = {
    [K in keyof C]: undefined extends InferChecked<C[K]> ? never : K;
}[keyof C];
type InferObject<C extends StructChecker> = {
    [K in OptionalObjectKeys<C>]?: Exclude<InferChecked<C[K]>, undefined>;
} & {
    [K in RequiredObjectKeys<C>]: InferChecked<C[K]>;
};
/** Infers the validated type from a checker predicate or object checker map. */
export type Infer<C> =
    C extends Checker<infer T>
        ? T
        : C extends StructChecker
          ? InferObject<C>
          : never;

const warnValidationFailure = (label: string, x: unknown) => {
    // Defer error display until the page is rendered.
    setTimeout(() => {
        showJSError(`Data validation error: ${label} is invalid`);
    });
    console.error(`${label} validation failed`, x);
};

export function validateData<T>(
    label: string,
    x: unknown,
    check: Checker<T>,
): asserts x is T {
    if (!check(x)) {
        warnValidationFailure(label, x);
    }
}

export function validateDataAsync<T>(
    label: string,
    x: unknown,
    check: Checker<T>,
): void {
    queueMicrotask(() => {
        validateData(label, x, check);
    });
}

/** Common type-predicate helpers for use in checker objects. */
export const is = {
    string: ((v) => typeof v === 'string') as Checker<string>,
    boolean: ((v) => typeof v === 'boolean') as Checker<boolean>,
    number: ((v) => typeof v === 'number') as Checker<number>,
    optional: <T>(check: Checker<T>) =>
        ((v) => v === undefined || v === null || check(v)) as Checker<
            T | undefined | null
        >,
    literal: <const T extends Literal>(value: T) =>
        ((v) => v === value) as Checker<T>,
    object: <C extends StructChecker>(checks: C) =>
        ((v) =>
            typeof v === 'object' &&
            v !== null &&
            !Array.isArray(v) &&
            Object.entries(checks).every(([key, check]) =>
                typeof check === 'function'
                    ? check((v as Record<string, unknown>)[key])
                    : check === (v as Record<string, unknown>)[key],
            )) as Checker<Infer<C>>,
    recordOf: <T>(check: Checker<T>) =>
        ((v) =>
            typeof v === 'object' &&
            v !== null &&
            !Array.isArray(v) &&
            Object.values(v).every(check)) as Checker<Record<string, T>>,
    arrayOf: <T>(check: Checker<T>) =>
        ((v) => Array.isArray(v) && v.every(check)) as Checker<T[]>,
    oneOf: <const T extends readonly ComplexCheckerElement[]>(...values: T) =>
        ((v) =>
            values.some((value) =>
                typeof value === 'function' ? value(v) : value === v,
            )) as Checker<InferChecked<T[number]>>,
};
