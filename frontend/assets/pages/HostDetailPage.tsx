import { Title } from '@solidjs/meta';
import { A, useParams } from '@solidjs/router';
import { Bell, BellRing, LoaderCircle, Power, PowerOff } from 'lucide-solid';
import { createSignal, For, Show } from 'solid-js';
import { AppLayout } from '../components/App';
import { apiFetch } from '../helpers/apiFetch';
import { state } from '../helpers/appStore';
import type { AnyComponent } from '../helpers/component';
import { demoUpdateLease, isDemoMode } from '../helpers/demo';
import { subscribeToHostOnline } from '../helpers/pushSubscription';

type NotifyState = 'idle' | 'loading' | 'subscribed' | 'error';

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
        leases().filter(
            (l): l is { type: 'Client'; value: string } => l.type === 'Client',
        );

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

    // Notification: online for longer than duration
    const [notifyDuration, setNotifyDuration] = createSignal('30');
    const [notifyDurationState, setNotifyDurationState] =
        createSignal<NotifyState>('idle');
    const handleNotifyDuration = async () => {
        setNotifyDurationState('loading');
        try {
            // TODO: Replace with actual "online for longer than duration" subscription endpoint
            throw new Error('Not yet implemented');
        } catch {
            setNotifyDurationState('error');
        }
    };

    // Notification: unscheduled events (startup/shutdown not triggered by ShutHost)
    const [notifyUnscheduledState, setNotifyUnscheduledState] =
        createSignal<NotifyState>('idle');
    const handleNotifyUnscheduled = async () => {
        setNotifyUnscheduledState('loading');
        try {
            // TODO: Replace with real unscheduled-event subscription once backend supports it.
            // Using subscribe-host-online as a placeholder.
            await subscribeToHostOnline(hostname());
            setNotifyUnscheduledState('subscribed');
        } catch (err) {
            console.error(
                `Failed to subscribe to unscheduled events for ${hostname()}:`,
                err,
            );
            setNotifyUnscheduledState('error');
        }
    };

    return (
        <AppLayout>
            <Title>{hostname()} - ShutHost Coordinator</Title>
            <nav aria-label="Breadcrumb" class="mb-4">
                <A href="/hosts" class="link text-sm">
                    ← Hosts
                </A>
            </nav>

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
                {/* Name + status badge */}
                <div class="flex items-center gap-3 mb-6 flex-wrap">
                    <h2 class="section-title mb-0">{hostname()}</h2>
                    <Show when={status() === 'online'}>
                        <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-green-100 text-green-800 dark:bg-[rgba(46,193,100,0.15)] dark:text-[rgba(46,193,100,0.9)]">
                            online
                        </span>
                    </Show>
                    <Show when={status() === 'offline'}>
                        <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-red-100 text-red-800 dark:bg-[rgba(244,135,113,0.15)] dark:text-[rgba(244,135,113,0.9)]">
                            offline
                        </span>
                    </Show>
                    <Show when={status() === undefined}>
                        <span class="px-2 py-0.5 rounded-full text-xs font-semibold bg-gray-100 text-gray-500 dark:bg-[#2d2d30] dark:text-[#858585]">
                            unknown
                        </span>
                    </Show>
                </div>

                {/* Information */}
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
                        {/* TODO: Requires backend to store agent_version from StartupBroadcast and expose it via WebSocket */}
                        <dd class="text-[#616161] dark:text-[#9d9d9d]">—</dd>
                    </dl>
                </section>

                {/* Notifications */}
                <section
                    class="section-container p-4 mb-4"
                    aria-labelledby="host-notifications-title"
                >
                    <h3
                        id="host-notifications-title"
                        class="section-title text-base"
                    >
                        Notifications
                    </h3>

                    {/* Notify when online for longer than duration */}
                    <div class="mb-4 pb-4 border-b border-[#e5e5e5] dark:border-[#3e3e42]">
                        <p class="font-medium text-sm text-black dark:text-[#cccccc] mb-1">
                            Notify when online for longer than…
                        </p>
                        <p class="description-text text-xs mb-2">
                            Get a push notification when this host has been
                            continuously online for longer than the given
                            duration.
                        </p>
                        <div class="flex items-center gap-2 flex-wrap">
                            <input
                                type="number"
                                min="1"
                                class="w-20 px-2 py-1 text-sm border border-[#e5e5e5] dark:border-[#3e3e42] rounded bg-white dark:bg-[#252526] text-black dark:text-[#cccccc]"
                                value={notifyDuration()}
                                onInput={(e) =>
                                    setNotifyDuration(e.currentTarget.value)
                                }
                                aria-label="Duration in minutes"
                            />
                            <span class="text-sm text-[#616161] dark:text-[#9d9d9d]">
                                minutes
                            </span>
                            <button
                                type="button"
                                class="btn btn-green"
                                disabled={
                                    notifyDurationState() === 'loading' ||
                                    notifyDurationState() === 'subscribed'
                                }
                                onClick={handleNotifyDuration}
                            >
                                <Show when={notifyDurationState() === 'loading'}>
                                    <LoaderCircle size={14} class="animate-spin" aria-hidden="true" />
                                </Show>
                                <Show when={notifyDurationState() !== 'loading'}>
                                    <Bell size={14} aria-hidden="true" />
                                </Show>
                                Subscribe
                            </button>
                            <Show when={notifyDurationState() === 'subscribed'}>
                                <span
                                    class="text-xs text-green-600 dark:text-[rgba(46,193,100,0.9)] inline-flex items-center gap-1"
                                    aria-live="polite"
                                >
                                    <BellRing size={14} aria-hidden="true" />
                                    Subscribed
                                </span>
                            </Show>
                            <Show when={notifyDurationState() === 'error'}>
                                <span
                                    class="text-xs text-red-500 dark:text-[#f48771]"
                                    aria-live="polite"
                                >
                                    Not yet implemented
                                </span>
                            </Show>
                        </div>
                    </div>

                    {/* Notify on unscheduled events */}
                    <div>
                        <p class="font-medium text-sm text-black dark:text-[#cccccc] mb-1">
                            Notify on unscheduled events
                        </p>
                        <p class="description-text text-xs mb-2">
                            Get a push notification when this host starts up or
                            shuts down without being triggered by ShutHost.
                        </p>
                        <div class="flex items-center gap-2 flex-wrap">
                            <button
                                type="button"
                                class="btn btn-green"
                                disabled={
                                    notifyUnscheduledState() === 'loading' ||
                                    notifyUnscheduledState() === 'subscribed'
                                }
                                onClick={handleNotifyUnscheduled}
                            >
                                <Show when={notifyUnscheduledState() === 'loading'}>
                                    <LoaderCircle size={14} class="animate-spin" aria-hidden="true" />
                                </Show>
                                <Show when={notifyUnscheduledState() !== 'loading'}>
                                    <Bell size={14} aria-hidden="true" />
                                </Show>
                                Subscribe
                            </button>
                            <Show
                                when={notifyUnscheduledState() === 'subscribed'}
                            >
                                <span
                                    class="text-xs text-green-600 dark:text-[rgba(46,193,100,0.9)] inline-flex items-center gap-1"
                                    aria-live="polite"
                                >
                                    <BellRing size={14} aria-hidden="true" />
                                    Subscribed
                                </span>
                            </Show>
                            <Show when={notifyUnscheduledState() === 'error'}>
                                <span
                                    class="text-xs text-red-500 dark:text-[#f48771]"
                                    aria-live="polite"
                                >
                                    Failed to subscribe. Please try again.
                                </span>
                            </Show>
                        </div>
                    </div>
                </section>

                {/* Leases */}
                <section
                    class="section-container mb-4"
                    aria-labelledby="host-leases-title"
                >
                    <div class="px-4 pt-4 pb-2">
                        <h3
                            id="host-leases-title"
                            class="section-title text-base"
                        >
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
                            <tbody
                                class="divide-y divide-gray-200"
                                aria-live="polite"
                            >
                                {/* Web Interface lease — always shown; Take/Release toggled via CSS */}
                                <tr
                                    class="table-row"
                                    data-has-lease={String(
                                        hasWebInterfaceLease(),
                                    )}
                                >
                                    <th class="table-cell" scope="row">
                                        Web Interface
                                    </th>
                                    <td class="table-cell">
                                        <div class="actions-cell">
                                            <button
                                                class="btn btn-green take-lease"
                                                type="button"
                                                onClick={() =>
                                                    updateLease('take')
                                                }
                                                aria-label="Take web interface lease"
                                            >
                                                <Power size={14} aria-hidden="true" />
                                                Take
                                            </button>
                                            <button
                                                class="btn btn-red release-lease"
                                                type="button"
                                                onClick={() =>
                                                    updateLease('release')
                                                }
                                                aria-label="Release web interface lease"
                                            >
                                                <PowerOff size={14} aria-hidden="true" />
                                                Release
                                            </button>
                                        </div>
                                    </td>
                                </tr>

                                {/* Client-held leases (read-only) */}
                                <For each={clientLeases()}>
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
            </Show>
        </AppLayout>
    );
}) satisfies AnyComponent;
