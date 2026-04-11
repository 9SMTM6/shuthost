import { Title } from '@solidjs/meta';
import { A, useParams } from '@solidjs/router';
import {
    ArrowLeft,
    Bell,
    BellOff,
    LoaderCircle,
    Power,
    PowerOff,
} from 'lucide-solid';
import { createSignal, For, onMount, Show } from 'solid-js';
import { AppLayout } from '../components/App';
import { CopyButton } from '../components/CopyButton';
import { apiFetch } from '../helpers/apiFetch';
import { HostStats, state } from '../helpers/appStore';
import { buildData } from '../helpers/buildData';
import { demoSubpath, demoUpdateLease, isDemoMode } from '../helpers/demo';
import {
    checkHostUnscheduledSubscription,
    subscribeToHostUnscheduled,
    unsubscribeFromHostUnscheduled,
} from '../helpers/pushSubscription';
import type { AnyComponent } from '../helpers/utils';
import { formatRelativeTimestamp } from '../helpers/utils';

type ClientLease = { type: 'Client'; value: string };

// --- Sub-components ---

const HostStatusBadge = (props: {
    status: 'online' | 'offline' | undefined;
}) => (
    <>
        <Show when={props.status === 'online'}>
            <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-green-100 text-green-800 dark:bg-[rgba(46,193,100,0.15)] dark:text-[rgba(46,193,100,0.9)]">
                online
            </span>
        </Show>
        <Show when={props.status === 'offline'}>
            <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-red-100 text-red-800 dark:bg-[rgba(244,135,113,0.15)] dark:text-[rgba(244,135,113,0.9)]">
                offline
            </span>
        </Show>
        <Show when={props.status === undefined}>
            <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-gray-100 text-gray-500 dark:bg-[#2d2d30] dark:text-[#858585]">
                unknown
            </span>
        </Show>
    </>
);

const NotifyUnscheduledButton = (props: { hostname: string }) => {
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
        <div
            class="flex flex-col items-center gap-1"
            title="Get a push notification when this host starts up or shuts down without being triggered by ShutHost."
        >
            <button
                type="button"
                class={`btn sm:px-5 sm:py-3 sm:text-base ${isSubscribed() ? 'btn-red' : 'btn-green'}`}
                disabled={isChecking() || loading()}
                onClick={handleClick}
            >
                <Show when={isChecking() || loading()}>
                    <LoaderCircle
                        size={16}
                        class="animate-spin"
                        aria-hidden="true"
                    />
                </Show>
                <Show when={!isChecking() && !loading() && isSubscribed()}>
                    <BellOff size={20} aria-hidden="true" />
                </Show>
                <Show when={!isChecking() && !loading() && !isSubscribed()}>
                    <Bell size={20} aria-hidden="true" />
                </Show>
                <span class="flex flex-col text-center leading-tight">
                    <span>
                        {isSubscribed() ? 'Unsubscribe from' : 'Subscribe to'}
                    </span>
                    <span>unscheduled events</span>
                </span>
            </button>
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

// const unitDefaults = { minutes: 30 as number, hours: 3, days: 1 } as const;
// type DurationUnit = keyof typeof unitDefaults;

// TODO: When I implement this, I want to extend it to allow both permanent subscriptions when a host was running for longer than x, as well as a one-time notification of that nature. 
// const NotifyDurationButton = (_props: { hostname: string }) => {
//     const [notifyDuration, setNotifyDuration] = createSignal(unitDefaults.minutes);
//     const [notifyDurationUnit, setNotifyDurationUnit] = createSignal<DurationUnit>('minutes');
//     const [notifyDurationModified, setNotifyDurationModified] = createSignal(false);
//     const [notifyState, setNotifyState] = createSignal<NotifyState>('idle');

//     const handleDurationInput = (value: string) => {
//         setNotifyDuration(Number(value));
//         setNotifyDurationModified(true);
//     };

//     const handleUnitChange = (unit: DurationUnit) => {
//         setNotifyDurationUnit(unit);
//         if (!notifyDurationModified()) {
//             setNotifyDuration(unitDefaults[unit]);
//         }
//     };

//     const handle = async () => {
//         setNotifyState('loading');
//         try {
//             // TODO: Replace with actual "online for longer than duration" subscription endpoint
//             throw new Error('Not yet implemented');
//         } catch {
//             setNotifyState('error');
//         }
//     };

//     return (
//         <div
//             class="flex flex-col items-center gap-1"
//             title="Get a push notification when this host has been continuously online for longer than the given duration."
//         >
//             <div class="flex items-center gap-1.5">
//                 <button
//                     type="button"
//                     class="btn btn-green sm:px-5 sm:py-3 sm:text-base"
//                     disabled={notifyState() === 'loading' || notifyState() === 'subscribed'}
//                     onClick={handle}
//                     aria-label="Subscribe to online-too-long notification"
//                 >
//                     <Show when={notifyState() === 'loading'}>
//                         <LoaderCircle size={16} class="animate-spin" aria-hidden="true" />
//                     </Show>
//                     <Show when={notifyState() !== 'loading'}>
//                         <Bell size={16} aria-hidden="true" />
//                     </Show>
//                     Notify after online for
//                 </button>
//                 <input
//                     type="number"
//                     min="1"
//                     class="w-16 px-2 py-2 text-sm border border-[#e5e5e5] dark:border-[#3e3e42] rounded bg-white dark:bg-[#252526] text-black dark:text-[#cccccc]"
//                     value={notifyDuration()}
//                     onInput={(e) => handleDurationInput(e.currentTarget.value)}
//                     aria-label="Duration"
//                 />
//                 <select
//                     class="px-2 py-2 text-sm border border-[#e5e5e5] dark:border-[#3e3e42] rounded bg-white dark:bg-[#252526] text-black dark:text-[#cccccc]"
//                     value={notifyDurationUnit()}
//                     onChange={(e) => handleUnitChange(e.currentTarget.value as DurationUnit)}
//                     aria-label="Duration unit"
//                 >
//                     <option value="minutes">min</option>
//                     <option value="hours">hr</option>
//                     <option value="days">day</option>
//                 </select>
//             </div>
//             <Show when={notifyState() === 'subscribed'}>
//                 <span
//                     class="text-xs text-green-600 dark:text-[rgba(46,193,100,0.9)] inline-flex items-center gap-1"
//                     aria-live="polite"
//                 >
//                     <BellRing size={12} aria-hidden="true" />
//                     Subscribed
//                 </span>
//             </Show>
//             <Show when={notifyState() === 'error'}>
//                 <span class="text-xs text-red-500 dark:text-[#f48771]" aria-live="polite">
//                     Not yet implemented
//                 </span>
//             </Show>
//         </div>
//     );
// };

const HostInfoSection = (props: {
    hostStats: HostStats | undefined;
    isOnline: boolean;
}) => {
    const lastOnline = props.hostStats?.lastOnline ?? null;
    const agentVersion = props.hostStats?.agentVersion ?? null;

    let updateCmds: { sh?: string; ps1?: string } | null = null;
    if (props.hostStats != null && agentVersion !== buildData.version) {
        const baseUrl = window.location.origin + demoSubpath;
        const initSystem = props.hostStats.initSystem;
        const scriptPath = props.hostStats.scriptPath;
        const os = props.hostStats.operatingSystem;

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

        const shCmd = `curl -fsSL ${baseUrl}/download/host_agent_installer.sh | sh -s ${baseUrl} -- --update${shScriptPathArg}`;
        const ps1Cmd = `curl.exe -sSLO '${baseUrl}/download/host_agent_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\host_agent_installer.ps1 ${baseUrl} -Update${ps1ScriptPathArg}`;

        if (initSystem === 'self-extracting-pwsh' || os === 'windows') {
            updateCmds = { ps1: ps1Cmd };
        } else if (os == null) {
            updateCmds = { sh: shCmd, ps1: ps1Cmd };
        } else {
            updateCmds = { sh: shCmd };
        }
    }

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
                <dd class="text-[#616161] dark:text-[#9d9d9d]">
                    {agentVersion ?? `<= 1.7.1`}
                </dd>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Init system
                </dt>
                <dd class="text-[#616161] dark:text-[#9d9d9d]">
                    {props.hostStats?.initSystem ?? 'Unknown'}
                </dd>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Operating system
                </dt>
                <dd class="text-[#616161] dark:text-[#9d9d9d]">
                    {props.hostStats?.operatingSystem ?? 'Unknown'}
                </dd>
                <Show when={props.hostStats?.scriptPath != null}>
                    <dt class="font-medium text-black dark:text-[#cccccc]">
                        Install script
                    </dt>
                    <dd class="text-[#616161] dark:text-[#9d9d9d] break-all">
                        {props.hostStats?.scriptPath}
                    </dd>
                </Show>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Last online
                </dt>
                <dd class="text-[#616161] dark:text-[#9d9d9d]">
                    {props.isOnline
                        ? 'Currently online'
                        : formatRelativeTimestamp(lastOnline)}
                </dd>
            </dl>
            <Show when={updateCmds != null}>
                <div class="mt-3 pt-3 border-t border-[#e5e5e5] dark:border-[#3e3e42]">
                    <p class="text-sm font-medium text-black dark:text-[#cccccc] mb-1">
                        Update agent
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
        </section>
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
                                    class="btn btn-green take-lease"
                                    type="button"
                                    onClick={() => props.updateLease('take')}
                                    aria-label="Take web interface lease"
                                >
                                    <Power size={14} aria-hidden="true" />
                                    Take
                                </button>
                                <button
                                    class="btn btn-red release-lease"
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
            <Show when={isLoading()}>
                <p class="description-text">Loading…</p>
            </Show>

            <Show when={!isLoading() && !isKnown()}>
                <div class="alert alert-error">
                    <p class="alert-title">Host not found</p>
                    <p>
                        No host named <strong>{hostname()}</strong> is known to
                        this coordinator.
                    </p>
                </div>
            </Show>

            <Show when={!isLoading() && isKnown()}>
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
                </A>

                {/* Notifications — centered, prominent, above information */}
                <div class="flex justify-evenly gap-3 mb-6 flex-wrap">
                    <NotifyUnscheduledButton hostname={hostname()} />
                    {/* <NotifyDurationButton hostname={hostname()} /> */}
                </div>

                <Show when={state.dbData.status === 'available'}>
                    <HostInfoSection
                        hostStats={hostStats()}
                        isOnline={status() === 'online'}
                    />
                </Show>

                <HostLeasesSection
                    hasWebInterfaceLease={hasWebInterfaceLease()}
                    clientLeases={clientLeases()}
                    updateLease={updateLease}
                />
            </Show>
        </AppLayout>
    );
}) satisfies AnyComponent;
