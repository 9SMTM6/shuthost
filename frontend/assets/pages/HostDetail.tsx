import { Title } from '@solidjs/meta';
import { A, useParams } from '@solidjs/router';
import {
    ArrowLeft,
    Bell,
    BellOff,
    BellRing,
    CircleDashed,
    Crosshair,
    LoaderCircle,
    Power,
    PowerOff,
    RefreshCw,
} from 'lucide-solid';
import { createSignal, For, Match, onMount, Show, Switch } from 'solid-js';
import { AppLayout } from '../components/App';
import { CopyButton } from '../components/CopyButton';
import { apiFetch } from '../helpers/apiFetch';
import {
    type HostConfig,
    type HostStats,
    type OperationFailure,
    state,
} from '../helpers/appStore';
import { buildData } from '../helpers/buildData';
import { demoSubpath, demoUpdateLease, isDemoMode } from '../helpers/demo';
import {
    checkHostOnlineForSubscription,
    checkHostOperationFailedSubscription,
    checkHostUnscheduledSubscription,
    subscribeToHostOnlineFor,
    subscribeToHostOnlineForOneshot,
    subscribeToHostOperationFailed,
    subscribeToHostUnscheduled,
    unsubscribeFromHostOnlineFor,
    unsubscribeFromHostOperationFailed,
    unsubscribeFromHostUnscheduled,
} from '../helpers/pushSubscription';
import type { AnyComponent } from '../helpers/utils';
import { formatRelativeTimestamp, safeExternalUrl } from '../helpers/utils';

type ClientLease = { type: 'Client'; value: string };

// --- Sub-components ---

export const HostStatusBadge = (props: {
    status: 'online' | 'offline' | 'waking' | 'shutting_down' | undefined;
}) => (
    <Switch
        fallback={
            <span class="host-status-badge bg-gray-100 text-gray-500 dark:bg-[#2d2d30] dark:text-[#858585]">
                unknown
            </span>
        }
    >
        <Match when={props.status === 'online'}>
            <span class="host-status-badge bg-green-100 text-green-800 dark:bg-[rgba(46,193,100,0.15)] dark:text-[rgba(46,193,100,0.9)]">
                online
            </span>
        </Match>
        <Match when={props.status === 'offline'}>
            <span class="host-status-badge bg-red-100 text-red-800 dark:bg-[rgba(244,135,113,0.15)] dark:text-[rgba(244,135,113,0.9)]">
                offline
            </span>
        </Match>
        <Match when={props.status === 'waking'}>
            <span class="host-status-badge bg-yellow-100 text-yellow-800 dark:bg-[rgba(234,179,8,0.15)] dark:text-[rgba(234,179,8,0.9)]">
                waking
            </span>
        </Match>
        <Match when={props.status === 'shutting_down'}>
            <span class="host-status-badge bg-orange-100 text-orange-800 dark:bg-[rgba(249,115,22,0.15)] dark:text-[rgba(249,115,22,0.9)]">
                shutting down
            </span>
        </Match>
    </Switch>
);

const NotifyUnscheduledButton = (props: { hostname: string }) => {
    const notifyUnscheduledDescription =
        'Get a push notification when this host starts up or shuts down without being triggered by ShutHost.';
    const [subscribed, setSubscribed] = createSignal<boolean | null>(null);
    const [loading, setLoading] = createSignal(false);
    const [error, setError] = createSignal<string | null>(null);

    onMount(async () => {
        try {
            const result = await checkHostUnscheduledSubscription(
                props.hostname,
            );
            setSubscribed(result);
        } catch {
            setSubscribed(false);
        }
    });

    const handleClick = async () => {
        if (subscribed() === null || loading()) return;
        setLoading(true);
        setError(null);
        const wasSubscribed = subscribed();
        try {
            if (wasSubscribed) {
                await unsubscribeFromHostUnscheduled(props.hostname);
                setSubscribed(false);
            } else {
                await subscribeToHostUnscheduled(props.hostname);
                setSubscribed(true);
            }
        } catch (err) {
            console.error(
                `Failed to ${wasSubscribed ? 'unsubscribe from' : 'subscribe to'} unscheduled events for ${props.hostname}:`,
                err,
            );
            setError('Failed. Please try again.');
        } finally {
            setLoading(false);
        }
    };

    const isChecking = () => subscribed() === null;
    const isSubscribed = () => subscribed() === true;

    return (
        <div class="flex flex-col items-center gap-1">
            <button
                type="button"
                class={`btn btn-height sm:px-5 sm:py-3 sm:text-base ${isSubscribed() ? 'btn-red' : 'btn-green'}`}
                disabled={isChecking() || loading()}
                onClick={handleClick}
                aria-describedby="notify-unscheduled-description"
                title={notifyUnscheduledDescription}
            >
                <Switch>
                    <Match when={isChecking() || loading()}>
                        <LoaderCircle
                            size={16}
                            class="animate-spin"
                            aria-hidden="true"
                        />
                    </Match>
                    <Match when={isSubscribed()}>
                        <BellOff size={20} aria-hidden="true" />
                    </Match>
                    <Match when={!isSubscribed()}>
                        <Bell size={20} aria-hidden="true" />
                    </Match>
                </Switch>
                <span class="flex flex-col text-center leading-tight">
                    <span>
                        {isSubscribed() ? 'Unsubscribe from' : 'Subscribe to'}
                    </span>
                    <span>unscheduled events</span>
                </span>
            </button>
            <p
                id="notify-unscheduled-description"
                class="text-xs text-[#616161] dark:text-[#9d9d9d] text-center max-w-[20rem] touch-description"
            >
                {notifyUnscheduledDescription}
            </p>
            <Show when={error() !== null}>
                <span
                    class="text-xs text-red-500 dark:text-[#f48771]"
                    aria-live="polite"
                >
                    {error()}
                </span>
            </Show>
        </div>
    );
};

const OperationFailureBadge = (props: {
    failure: OperationFailure | undefined;
}) => (
    <Show when={props.failure !== undefined}>
        <span class="host-status-badge bg-amber-100 text-amber-800 dark:bg-[rgba(245,158,11,0.15)] dark:text-[rgba(245,158,11,0.9)]">
            {props.failure?.operation} failed
        </span>
    </Show>
);

const NotifyOperationFailedButton = (props: { hostname: string }) => {
    const description =
        'Get a push notification when a shutdown or startup command for this host fails or times out.';
    const [subscribed, setSubscribed] = createSignal<boolean | null>(null);
    const [loading, setLoading] = createSignal(false);
    const [error, setError] = createSignal<string | null>(null);

    onMount(async () => {
        try {
            const result = await checkHostOperationFailedSubscription(
                props.hostname,
            );
            setSubscribed(result);
        } catch {
            setSubscribed(false);
        }
    });

    const handleClick = async () => {
        if (subscribed() === null || loading()) return;
        setLoading(true);
        setError(null);
        const wasSubscribed = subscribed();
        try {
            if (wasSubscribed) {
                await unsubscribeFromHostOperationFailed(props.hostname);
                setSubscribed(false);
            } else {
                await subscribeToHostOperationFailed(props.hostname);
                setSubscribed(true);
            }
        } catch (err) {
            console.error(
                `Failed to ${
                    wasSubscribed ? 'unsubscribe from' : 'subscribe to'
                } operation-failed events for ${props.hostname}:`,
                err,
            );
            setError('Failed. Please try again.');
        } finally {
            setLoading(false);
        }
    };

    const isChecking = () => subscribed() === null;
    const isSubscribed = () => subscribed() === true;

    return (
        <div class="flex flex-col items-center gap-1">
            <button
                type="button"
                class={`btn btn-height sm:px-5 sm:py-3 sm:text-base ${
                    isSubscribed() ? 'btn-red' : 'btn-green'
                }`}
                disabled={isChecking() || loading()}
                onClick={handleClick}
                aria-describedby="notify-operation-failed-description"
                title={description}
            >
                <Switch>
                    <Match when={isChecking() || loading()}>
                        <LoaderCircle
                            size={16}
                            class="animate-spin"
                            aria-hidden="true"
                        />
                    </Match>
                    <Match when={isSubscribed()}>
                        <BellOff size={20} aria-hidden="true" />
                    </Match>
                    <Match when={!isSubscribed()}>
                        <Bell size={20} aria-hidden="true" />
                    </Match>
                </Switch>
                <span class="flex flex-col text-center leading-tight">
                    <span>
                        {isSubscribed() ? 'Unsubscribe from' : 'Subscribe to'}
                    </span>
                    <span>failed operations</span>
                </span>
            </button>
            <p
                id="notify-operation-failed-description"
                class="text-xs text-[#616161] dark:text-[#9d9d9d] text-center max-w-[20rem] touch-description"
            >
                {description}
            </p>
            <p class="text-xs text-[#7a7a7a] dark:text-[#8f8f8f] text-center max-w-[20rem]">
                Experimental: subscriptions for failed operations are still
                experimental.
                <a
                    class="text-xs text-blue-600 dark:text-blue-400 underline ml-1"
                    href={safeExternalUrl(
                        `${buildData.repository}/issues/new?title=${encodeURIComponent(
                            `Feedback: failed operations subscription for ${props.hostname}`,
                        )}&body=${encodeURIComponent(
                            'Describe what happened and include steps to reproduce if possible.',
                        )}`,
                    )}
                    target="_blank"
                    rel="external noopener noreferrer"
                >
                    Give feedback
                </a>
            </p>
            <Show when={error() !== null}>
                <span
                    class="text-xs text-red-500 dark:text-[#f48771]"
                    aria-live="polite"
                >
                    {error()}
                </span>
            </Show>
        </div>
    );
};

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

const NotifyDurationButton = (props: {
    hostname: string;
    isOnline: boolean;
}) => {
    const [duration, setDuration] = createSignal(unitDefaults.minutes);
    const [unit, setUnit] = createSignal<DurationUnit>('minutes');
    const [durationModified, setDurationModified] = createSignal(false);

    // Permanent subscription state (null = still loading)
    const [permanentSub, setPermanentSub] =
        createSignal<PermanentSubState | null>(null);
    const [permanentLoading, setPermanentLoading] = createSignal(false);
    const [permanentError, setPermanentError] = createSignal<string | null>(
        null,
    );

    // One-shot subscribe state
    const [oneshotLoading, setOneshotLoading] = createSignal(false);
    const [oneshotSuccess, setOneshotSuccess] = createSignal(false);
    const [oneshotError, setOneshotError] = createSignal<string | null>(null);

    onMount(async () => {
        try {
            const durationSecs = await checkHostOnlineForSubscription(
                props.hostname,
            );
            if (durationSecs != null) {
                // Pre-fill the UI with the existing subscription's duration.
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
            {/* Lückentext: "Notify me after online for [30] [min]" */}
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
            {/* Action buttons */}
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
                    class={`btn btn-height sm:px-4 sm:py-2 sm:text-base ${
                        isPermanentlySubscribed() ? 'btn-red' : 'btn-green'
                    }`}
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
            <p class="text-xs text-[#7a7a7a] dark:text-[#8f8f8f] text-center max-w-[20rem]">
                Experimental: recurring and one-time "notify after online"
                subscriptions are still experimental.
                <a
                    class="text-xs text-blue-600 dark:text-blue-400 underline ml-1"
                    href={safeExternalUrl(
                        `${buildData.repository}/issues/new?title=${encodeURIComponent(
                            `Feedback: notify-after-online subscription for ${props.hostname}`,
                        )}&body=${encodeURIComponent(
                            'Describe what happened and include steps to reproduce if possible.',
                        )}`,
                    )}
                    target="_blank"
                    rel="external noopener noreferrer"
                >
                    Give feedback
                </a>
            </p>
            {/* Feedback */}
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
};

const buildHostUpdateCommands = (
    hostStats: HostStats | undefined,
): { sh?: string; ps1?: string } | null => {
    if (hostStats == null || hostStats.agentVersion === buildData.version) {
        return null;
    }

    const baseUrl = window.location.origin + demoSubpath;
    const initSystem = hostStats.initSystem;
    const scriptPath = hostStats.scriptPath;
    const os = hostStats.operatingSystem;

    let shScriptPathArg = '';
    let ps1ScriptPathArg = '';
    if (scriptPath != null) {
        if (initSystem === 'self-extracting-shell') {
            shScriptPathArg = ` --script-path '${scriptPath}'`;
        } else if (initSystem === 'self-extracting-pwsh') {
            ps1ScriptPathArg = ` -ScriptPath '${scriptPath}'`;
        } else {
            console.error(
                `Host has scriptPath '${scriptPath}' but init system '${
                    initSystem ?? 'unknown'
                }' is not a self-extracting type`,
            );
        }
    }

    const shCmd = `curl -fsSL ${baseUrl}/download/host_agent_installer.sh | sh -s ${baseUrl} --update${shScriptPathArg}`;
    const ps1Cmd = `curl.exe -sSLO '${baseUrl}/download/host_agent_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\host_agent_installer.ps1 ${baseUrl} -Update${ps1ScriptPathArg}`;

    if (initSystem === 'self-extracting-pwsh' || os === 'windows') {
        return { ps1: ps1Cmd };
    } else if (os == null) {
        return { sh: shCmd, ps1: ps1Cmd };
    }

    return { sh: shCmd };
};

const HostInfoSection = (props: {
    hostStats: HostStats | undefined;
    hostConfig: HostConfig | undefined;
    isOnline: boolean;
}) => {
    const lastOnline = props.hostStats?.lastOnline ?? null;
    const agentVersion = props.hostStats
        ? (props.hostStats.agentVersion ?? `<= 1.7.1`)
        : 'unknown';
    const agentVersionNote =
        agentVersion === 'unknown'
            ? 'No record of this host connecting yet.'
            : agentVersion === '<= 1.7.1'
              ? 'Predates version reporting (added in 1.8.0).'
              : undefined;
    const isSelfExtracting =
        props.hostStats?.initSystem === 'self-extracting-shell' ||
        props.hostStats?.initSystem === 'self-extracting-pwsh';
    const initSystemNote = isSelfExtracting
        ? 'Runs as a standalone script, not a registered service. Autostart must be configured manually (e.g. via the init system).'
        : undefined;
    const enforceStateNote = props.hostConfig?.enforceState
        ? 'Periodically corrects power state to match current leases.'
        : 'Edge-triggered only — reacts to lease changes, no periodic correction.';
    const lastOnlinePrecise =
        !props.isOnline && lastOnline != null
            ? new Date(lastOnline).toLocaleString()
            : undefined;

    return (
        <section
            class="section-container p-4 mb-4"
            aria-labelledby="host-info-title"
        >
            <h3 id="host-info-title" class="section-title text-base">
                Information
            </h3>
            <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Agent version
                </dt>
                <dd class="text-[#616161] dark:text-[#9d9d9d]" title={agentVersionNote}>
                    {agentVersion}
                </dd>
                <Show when={agentVersionNote != null}>
                    <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                        {agentVersionNote}
                    </dd>
                </Show>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Init system
                </dt>
                <dd class="text-[#616161] dark:text-[#9d9d9d]" title={initSystemNote}>
                    {
                        (
                            {
                                systemd: 'systemd',
                                openrc: 'OpenRC',
                                'self-extracting-shell': 'Self-extracting (sh)',
                                'self-extracting-pwsh':
                                    'Self-extracting (PowerShell)',
                                launchd: 'launchd',
                                unknown: 'Unknown',
                            } as const
                        )[props.hostStats?.initSystem ?? 'unknown']
                    }
                </dd>
                <Show when={initSystemNote != null}>
                    <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                        {initSystemNote}
                    </dd>
                </Show>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Operating system
                </dt>
                <dd class="text-[#616161] dark:text-[#9d9d9d]">
                    {
                        (
                            {
                                linux: 'Linux',
                                windows: 'Windows',
                                macos: 'macOS',
                                unknown: 'Unknown',
                            } as const
                        )[props.hostStats?.operatingSystem ?? 'unknown']
                    }
                </dd>
                <Show when={props.hostStats?.scriptPath}>
                    <dt class="font-medium text-black dark:text-[#cccccc]">
                        Install script
                    </dt>
                    <dd class="text-[#616161] dark:text-[#9d9d9d] break-all">
                        {props.hostStats?.scriptPath}
                    </dd>
                </Show>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Enforce state
                </dt>
                <dd
                    class="text-[#616161] dark:text-[#9d9d9d] inline-flex items-center gap-1"
                    title={enforceStateNote}
                >
                    <Show
                        when={props.hostConfig?.enforceState}
                        fallback={
                            <>
                                <CircleDashed
                                    size={14}
                                    class="text-[#9d9d9d]"
                                    aria-hidden="true"
                                />
                                No
                            </>
                        }
                    >
                        <Crosshair
                            size={14}
                            class="text-green-600 dark:text-[rgba(46,193,100,0.9)]"
                            aria-hidden="true"
                        />
                        Yes
                    </Show>
                </dd>
                <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                    {enforceStateNote}
                </dd>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Last online
                </dt>
                <dd
                    class="text-[#616161] dark:text-[#9d9d9d]"
                    title={lastOnlinePrecise}
                >
                    {props.isOnline
                        ? 'Currently online'
                        : formatRelativeTimestamp(lastOnline)}
                </dd>
                <Show when={lastOnlinePrecise != null}>
                    <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                        {lastOnlinePrecise}
                    </dd>
                </Show>
            </dl>
            <HostUpdateCommands hostStats={props.hostStats} />
        </section>
    );
};

const HostUpdateCommands = (props: { hostStats: HostStats | undefined }) => {
    const updateCmds = buildHostUpdateCommands(props.hostStats);

    return (
        <Show when={updateCmds != null}>
            <div class="mt-3 pt-3 border-t border-[#e5e5e5] dark:border-[#3e3e42]">
                <p class="text-sm font-medium text-black dark:text-[#cccccc] mb-1">
                    Update agent
                </p>
                <p class="text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-2">
                    Experimental: agent updates — self-extracting variants
                    especially — are experimental; use with caution.
                    <a
                        class="text-xs text-blue-600 dark:text-blue-400 underline ml-1"
                        href={safeExternalUrl(
                            `${buildData.repository}/issues/new?title=${encodeURIComponent(
                                'Feedback: agent update (self-extracting)',
                            )}&body=${encodeURIComponent(
                                'Describe what happened and include steps to reproduce if possible.',
                            )}`,
                        )}
                        target="_blank"
                        rel="external noopener noreferrer"
                    >
                        Give feedback
                    </a>
                </p>
                <Show when={updateCmds?.sh != null}>
                    <Show when={updateCmds?.ps1 != null}>
                        <p class="text-xs font-semibold text-[#616161] dark:text-[#9d9d9d] mb-1">
                            Linux/macOS:
                        </p>
                    </Show>
                    <div class="code-container py-2">
                        <CopyButton
                            targetId="host-update-command-sh"
                            label="Copy update command (sh)"
                        />
                        <code id="host-update-command-sh" class="code-block">
                            {updateCmds?.sh}
                        </code>
                    </div>
                </Show>
                <Show when={updateCmds?.ps1 != null}>
                    <Show when={updateCmds?.sh != null}>
                        <p class="text-xs font-semibold text-[#616161] dark:text-[#9d9d9d] mb-1 mt-2">
                            Windows (PowerShell):
                        </p>
                    </Show>
                    <div class="code-container py-2">
                        <CopyButton
                            targetId="host-update-command-ps1"
                            label="Copy update command (PowerShell)"
                        />
                        <code id="host-update-command-ps1" class="code-block">
                            {updateCmds?.ps1}
                        </code>
                    </div>
                </Show>
            </div>
        </Show>
    );
};

const HostLeasesSection = (props: {
    hasWebInterfaceLease: boolean;
    clientLeases: ClientLease[];
    updateLease: (action: 'take' | 'release') => Promise<void>;
}) => (
    <section class="section-container mb-4" aria-labelledby="host-leases-title">
        <div class="px-4 pt-4 pb-2">
            <h3 id="host-leases-title" class="section-title text-base">
                Leases
            </h3>
        </div>
        <div class="table-wrapper">
            <table class="actions-table w-full">
                <thead>
                    <tr>
                        <th class="table-header" scope="col">
                            Holder
                        </th>
                        <th class="table-header" scope="col">
                            Actions
                        </th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-gray-200" aria-live="polite">
                    {/* Web Interface lease — always shown; Take/Release toggled via CSS */}
                    <tr
                        class="table-row"
                        data-has-lease={String(props.hasWebInterfaceLease)}
                    >
                        <th class="table-cell" scope="row">
                            <span class="block">Web Interface</span>
                            <Show when={!props.hasWebInterfaceLease}>
                                <span class="text-xs text-[#616161] dark:text-[#9d9d9d] font-normal">
                                    no lease held
                                </span>
                            </Show>
                        </th>
                        <td class="table-cell">
                            <div class="actions-cell">
                                <button
                                    class="btn btn-height btn-green take-lease"
                                    type="button"
                                    onClick={() => props.updateLease('take')}
                                    aria-label="Take web interface lease"
                                >
                                    <Power size={14} aria-hidden="true" />
                                    Take
                                </button>
                                <button
                                    class="btn btn-height btn-red release-lease"
                                    type="button"
                                    onClick={() => props.updateLease('release')}
                                    aria-label="Release web interface lease"
                                >
                                    <PowerOff size={14} aria-hidden="true" />
                                    Release
                                </button>
                            </div>
                        </td>
                    </tr>

                    {/* Client-held leases (read-only) */}
                    <For each={props.clientLeases}>
                        {(lease) => (
                            <tr class="table-row">
                                <th class="table-cell" scope="row">
                                    {lease.value}
                                </th>
                                <td class="table-cell text-[#616161] dark:text-[#9d9d9d] text-xs">
                                    Client-held
                                </td>
                            </tr>
                        )}
                    </For>
                </tbody>
            </table>
        </div>
    </section>
);

// --- Page ---

export const HostDetailPage = (() => {
    const params = useParams<{ hostname: string }>();
    const hostname = () => params.hostname;

    const isLoading = () => state.hosts.length === 0;
    const isKnown = () => state.hosts.includes(hostname());
    const status = () => state.statusMap[hostname()];
    const leases = () => state.leaseMap[hostname()] ?? [];
    const hasWebInterfaceLease = () =>
        leases().some((l) => l.type === 'WebInterface');
    const clientLeases = () =>
        leases().filter((l): l is ClientLease => l.type === 'Client');
    const hostStats = (): HostStats | undefined =>
        state.dbData.status === 'available'
            ? state.dbData.payload.hostStats[hostname()]
            : undefined;
    const hostConfig = (): HostConfig | undefined =>
        state.hostConfigMap[hostname()];

    const updateLease = async (action: 'take' | 'release') => {
        if (isDemoMode) {
            await demoUpdateLease(hostname(), action);
            return;
        }
        try {
            await apiFetch(`/api/lease/${hostname()}/${action}`, {
                method: 'POST',
            });
        } catch (err) {
            if (err instanceof Error && err.message === 'Unauthorized') return;
            console.error(`Failed to ${action} lease for ${hostname()}:`, err);
        }
    };

    return (
        <AppLayout>
            <Title>{hostname()} - ShutHost Coordinator</Title>
            <Switch>
                <Match when={isLoading()}>
                    <p class="description-text">Loading…</p>
                </Match>
                <Match when={!isLoading() && !isKnown()}>
                    <div class="alert alert-error">
                        <p class="alert-title">Host not found</p>
                        <p>
                            No host named <strong>{hostname()}</strong> is known
                            to this coordinator.
                        </p>
                    </div>
                </Match>
                <Match when={!isLoading() && isKnown()}>
                    {/* Name + status badge — acts as back-navigation link */}
                    <A
                        href="/hosts"
                        aria-label={`Back to hosts list — currently viewing ${hostname()}`}
                        class="group flex items-center gap-3 mb-6 flex-wrap hover:opacity-80 transition-opacity cursor-pointer"
                    >
                        <ArrowLeft
                            size={18}
                            aria-hidden="true"
                            class="shrink-0 text-[#616161] dark:text-[#9d9d9d] group-hover:-translate-x-0.5 transition-transform"
                        />
                        <h2 class="section-title mb-0">{hostname()}</h2>
                        <HostStatusBadge status={status()} />
                        <OperationFailureBadge
                            failure={state.operationFailures[hostname()]}
                        />
                    </A>

                    {/* Notifications — centered, prominent, above information */}
                    {/* 1-col: Unscheduled → OperationFailed → Duration (stacked)
                        sm (2-col): Unscheduled | OperationFailed, then Duration full-width
                        lg (3-col): Unscheduled | Duration (middle) | OperationFailed */}
                    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
                        <div class="flex justify-center">
                            <NotifyUnscheduledButton hostname={hostname()} />
                        </div>
                        <div class="flex justify-center lg:order-3">
                            <NotifyOperationFailedButton
                                hostname={hostname()}
                            />
                        </div>
                        <div class="flex justify-center sm:col-span-2 lg:col-span-1 lg:order-2">
                            <NotifyDurationButton
                                hostname={hostname()}
                                isOnline={status() === 'online'}
                            />
                        </div>
                    </div>

                    <Show when={state.dbData.status === 'available' || hostConfig() !== undefined}>
                        <HostInfoSection
                            hostStats={hostStats()}
                            hostConfig={hostConfig()}
                            isOnline={status() === 'online'}
                        />
                    </Show>

                    <HostLeasesSection
                        hasWebInterfaceLease={hasWebInterfaceLease()}
                        clientLeases={clientLeases()}
                        updateLease={updateLease}
                    />
                </Match>
            </Switch>
        </AppLayout>
    );
}) satisfies AnyComponent;
