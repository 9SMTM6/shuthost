export const sortActiveFirst = <T>(
    items: T[],
    isActive: (item: T) => boolean,
    getName: (item: T) => string,
): T[] => {
    const compare = (a: T, b: T) => getName(a).localeCompare(getName(b));
    return [
        ...items.filter(isActive).toSorted(compare),
        ...items.filter((i) => !isActive(i)).toSorted(compare),
    ];
};
