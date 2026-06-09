import type { AnyComponent } from '../../helpers/utils';
import { Bell, BellOff, BellRing, LoaderCircle, RefreshCw } from 'lucide-solid';
import { createSignal, Match, onMount, Show, Switch } from 'solid-js';
import {
    checkHostOnlineForSubscription,
    subscribeToHostOnlineFor,
    subscribeToHostOnlineForOneshot,
    unsubscribeFromHostOnlineFor,
} from '../../helpers/pushSubscription';

const unitDefaults = { minutes: 30 as number, hours: 3, days: 1 } as const;

type DurationUnit = keyof typeof unitDefaults;

const secondsPerUnit: Record<DurationUnit, number> = {
    minutes: 60,
    hours: 3600,
    days: 86400,
};

type PermanentSubState =
    | { subscribed: false }
    | { subscribed: true; duration: number; unit: DurationUnit };

export const NotifyDurationButton = ((props: {
    hostname: string;
    isOnline: boolean;
}) => {
    const [duration, setDuration] = createSignal(unitDefaults.minutes);
    const [unit, setUnit] = createSignal<DurationUnit>('minutes');
    const [durationModified, setDurationModified] = createSignal(false);
    const [permanentSub, setPermanentSub] =
        createSignal<PermanentSubState | null>(null);
    const [permanentLoading, setPermanentLoading] = createSignal(false);
    const [permanentError, setPermanentError] = createSignal<string | null>(
        null,
    );
    const [oneshotLoading, setOneshotLoading] = createSignal(false);
    const [oneshotSuccess, setOneshotSuccess] = createSignal(false);
    const [oneshotError, setOneshotError] = createSignal<string | null>(null);

    onMount(async () => {
        try {
            const durationSecs = await checkHostOnlineForSubscription(
                props.hostname,
            );
            if (durationSecs != null) {
                let unit: DurationUnit = 'minutes';
                let value = durationSecs / 60;
                if (durationSecs % 86400 === 0) {
                    unit = 'days';
                    value = durationSecs / 86400;
                } else if (durationSecs % 3600 === 0) {
                    unit = 'hours';
                    value = durationSecs / 3600;
                }
                setUnit(unit);
                setDuration(value);
                setPermanentSub({ subscribed: true, duration: value, unit });
            } else {
                setPermanentSub({ subscribed: false });
            }
        } catch {
            setPermanentSub({ subscribed: false });
        }
    });

    const handleUnitChange = (newUnit: DurationUnit) => {
        setUnit(newUnit);
        if (!durationModified()) {
            setDuration(unitDefaults[newUnit]);
        }
    };

    const handleDurationInput = (value: string) => {
        const parsed = Number(value);
        if (parsed > 0) setDuration(parsed);
        setDurationModified(true);
    };

    const handleOneshotClick = async () => {
        if (oneshotLoading()) return;
        setOneshotLoading(true);
        setOneshotError(null);
        setOneshotSuccess(false);
        try {
            await subscribeToHostOnlineForOneshot(
                props.hostname,
                duration() * secondsPerUnit[unit()],
            );
            setOneshotSuccess(true);
            setTimeout(() => setOneshotSuccess(false), 4000);
        } catch {
            setOneshotError('Failed. Please try again.');
        } finally {
            setOneshotLoading(false);
        }
    };

    const handlePermanentClick = async () => {
        const current = permanentSub();
        if (current === null || permanentLoading()) return;
        setPermanentLoading(true);
        setPermanentError(null);
        try {
            if (current.subscribed) {
                await unsubscribeFromHostOnlineFor(props.hostname);
                setPermanentSub({ subscribed: false });
            } else {
                const durationSecs = duration() * secondsPerUnit[unit()];
                await subscribeToHostOnlineFor(props.hostname, durationSecs);
                setPermanentSub({
                    subscribed: true,
                    duration: duration(),
                    unit: unit(),
                });
            }
        } catch {
            setPermanentError('Failed. Please try again.');
        } finally {
            setPermanentLoading(false);
        }
    };

    const handleResubscribeClick = async () => {
        if (permanentLoading()) return;
        setPermanentLoading(true);
        setPermanentError(null);
        try {
            const durationSecs = duration() * secondsPerUnit[unit()];
            await subscribeToHostOnlineFor(props.hostname, durationSecs);
            setPermanentSub({
                subscribed: true,
                duration: duration(),
                unit: unit(),
            });
        } catch {
            setPermanentError('Failed. Please try again.');
        } finally {
            setPermanentLoading(false);
        }
    };

    const isCheckingPermanent = () => permanentSub() === null;
    const isPermanentlySubscribed = () => permanentSub()?.subscribed === true;
    const isPermanentDurationDifferent = () => {
        const sub = permanentSub();
        return (
            sub?.subscribed === true &&
            (duration() !== sub.duration || unit() !== sub.unit)
        );
    };

    return (
        <div class="flex flex-col items-center gap-2">
            <div class="flex items-center gap-1.5 flex-wrap justify-center text-sm text-black dark:text-[#cccccc]">
                <span>Notify me after online for</span>
                <input
                    type="number"
                    min="1"
                    class="w-16 px-2 py-1.5 text-sm border border-[#e5e5e5] dark:border-[#3e3e42] rounded bg-white dark:bg-[#252526] text-black dark:text-[#cccccc]"
                    value={duration()}
                    onInput={(e) => handleDurationInput(e.currentTarget.value)}
                    aria-label="Duration"
                />
                <select
                    class="px-2 py-1.5 text-sm border border-[#e5e5e5] dark:border-[#3e3e42] rounded bg-white dark:bg-[#252526] text-black dark:text-[#cccccc]"
                    value={unit()}
                    onChange={(e) =>
                        handleUnitChange(e.currentTarget.value as DurationUnit)
                    }
                    aria-label="Duration unit"
                >
                    <option value="minutes">min</option>
                    <option value="hours">hr</option>
                    <option value="days">day</option>
                </select>
            </div>
            <div class="flex gap-2 flex-wrap justify-center">
                <button
                    type="button"
                    class="btn btn-green sm:px-4 sm:py-2 sm:text-base"
                    disabled={oneshotLoading() || !props.isOnline}
                    onClick={handleOneshotClick}
                    title={
                        props.isOnline
                            ? 'Get a one-time notification when this host has been online for the given duration'
                            : 'Host is not currently online'
                    }
                >
                    <Show
                        when={oneshotLoading()}
                        fallback={<Bell size={16} aria-hidden="true" />}
                    >
                        <LoaderCircle
                            size={16}
                            class="animate-spin"
                            aria-hidden="true"
                        />
                    </Show>
                    once
                </button>
                <Show when={isPermanentDurationDifferent()}>
                    <button
                        type="button"
                        class="btn btn-yellow sm:px-4 sm:py-2 sm:text-base"
                        disabled={permanentLoading()}
                        onClick={handleResubscribeClick}
                        title="Update the recurring notification to use the currently selected duration"
                    >
                        <Show
                            when={permanentLoading()}
                            fallback={
                                <RefreshCw size={16} aria-hidden="true" />
                            }
                        >
                            <LoaderCircle
                                size={16}
                                class="animate-spin"
                                aria-hidden="true"
                            />
                        </Show>
                        update
                    </button>
                </Show>
                <button
                    type="button"
                    class={`btn btn-height sm:px-4 sm:py-2 sm:text-base ${isPermanentlySubscribed() ? 'btn-red' : 'btn-green'}`}
                    disabled={isCheckingPermanent() || permanentLoading()}
                    onClick={handlePermanentClick}
                    title={(() => {
                        const sub = permanentSub();
                        if (sub?.subscribed === true) {
                            return `Unsubscribe from recurring notifications (currently set to ${sub.duration} ${sub.unit})`;
                        }
                        return 'Subscribe to recurring notifications when this host stays online longer than the given duration';
                    })()}
                >
                    <Switch>
                        <Match
                            when={isCheckingPermanent() || permanentLoading()}
                        >
                            <LoaderCircle
                                size={16}
                                class="animate-spin"
                                aria-hidden="true"
                            />
                        </Match>
                        <Match when={isPermanentlySubscribed()}>
                            <BellOff size={16} aria-hidden="true" />
                        </Match>
                        <Match when={!isPermanentlySubscribed()}>
                            <Bell size={16} aria-hidden="true" />
                        </Match>
                    </Switch>
                    {isPermanentlySubscribed() ? 'unsubscribe' : 'always'}
                </button>
            </div>
            <Show when={oneshotSuccess()}>
                <span
                    class="text-xs text-green-600 dark:text-[rgba(46,193,100,0.9)] inline-flex items-center gap-1"
                    aria-live="polite"
                >
                    <BellRing size={12} aria-hidden="true" />
                    Subscribed — you'll be notified once
                </span>
            </Show>
            <Show when={isPermanentlySubscribed()}>
                {(() => {
                    const sub = permanentSub() as {
                        subscribed: true;
                        duration: number;
                        unit: string;
                    };
                    return (
                        <span
                            class="text-xs text-green-600 dark:text-[rgba(46,193,100,0.9)] inline-flex items-center gap-1"
                            aria-live="polite"
                        >
                            <BellRing size={12} aria-hidden="true" />
                            Recurring notifications active (every {sub.duration}{' '}
                            {sub.unit})
                        </span>
                    );
                })()}
            </Show>
            <Show when={oneshotError() !== null}>
                <span
                    class="text-xs text-red-500 dark:text-[#f48771]"
                    aria-live="polite"
                >
                    {oneshotError()}
                </span>
            </Show>
            <Show when={permanentError() !== null}>
                <span
                    class="text-xs text-red-500 dark:text-[#f48771]"
                    aria-live="polite"
                >
                    {permanentError()}
                </span>
            </Show>
        </div>
    );
}) satisfies AnyComponent;
