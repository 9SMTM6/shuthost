import { A, useNavigate } from '@solidjs/router';
import { Power, PowerOff, TriangleAlert } from 'lucide-solid';
import { createMemo, For, Show } from 'solid-js';
import { AppLayout } from '../components/App';
import { CopyButton } from '../components/CopyButton';
import { HostStatusBadge } from '../components/HostStatusBadge';
import { apiFetch, ApiFetchUnauthorizedError } from '../helpers/apiFetch';
import type { LeaseSource, Status } from '../helpers/appStore';
import { state } from '../helpers/appStore';
import { demo, demoSubpath, isDemoMode } from '../helpers/demo';
import { serverData } from '../helpers/serverData';
import type { AnyComponent } from '../helpers/utils';
import { sortActiveFirst } from '../helpers/utils';
import agentGotchasHtml from '../partials/agent_install_requirements_gotchas.md?raw';

const formatLeaseSource = (lease: LeaseSource) =>
    lease.type === 'Client' ? lease.value : '';

const getFormattedLeases = (leases: LeaseSource[]) => {
    const clientLeases = leases.filter((l) => l.type === 'Client');
    return clientLeases.length > 0
        ? clientLeases.map(formatLeaseSource).join(', ')
        : 'None';
};

const statusDisplayMap = {
    online: 'online',
    offline: 'offline',
    waking: 'waking',
    shutting_down: 'shutting',
} as const satisfies Record<Status, string>;

const getStatusLabel = (status?: Status) =>
    status === undefined ? 'Loading...' : statusDisplayMap[status];

const statusReserveLabel = (
    [getStatusLabel(), ...Object.values(statusDisplayMap)] as const
).reduce(
    (longest, label) => (label.length > longest.length ? label : longest),
    getStatusLabel(),
);

// ==========================
// Installer commands
// ==========================

const makeInstallCommands = () => {
    const baseUrl = window.location.origin + demoSubpath;
    const bpArg = `--broadcast-port ${serverData.broadcastPort}`;
    return {
        hostSh: `curl -fsSL ${baseUrl}/download/host_agent_installer.sh | sh -s ${baseUrl} -- ${bpArg}`,
        hostPs1: `curl.exe -sSLO '${baseUrl}/download/host_agent_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\host_agent_installer.ps1 ${baseUrl} ${bpArg}`,
    };
};

// ==========================
// Shared host helpers
// ==========================

const updateLease = async (hostName: string, action: 'take' | 'release') => {
    if (isDemoMode) return demo.updateLease(hostName, action);
    try {
        await apiFetch(`/api/lease/${hostName}/${action}`, {
            method: 'POST',
        });
    } catch (err) {
        if (err instanceof ApiFetchUnauthorizedError) return;
        console.error(`Failed to ${action} lease for ${hostName}:`, err);
    }
};

const HostNameLink = ((props: { hostName: string; class?: string }) => (
    <A
        href={`/hosts/${props.hostName}`}
        class={['link', props.class].filter(Boolean).join(' ')}
    >
        {props.hostName}
    </A>
)) satisfies AnyComponent;

const HostStatusDisplay = ((props: { hostName: string }) => (
    <>
        <HostStatusBadge status={state.statusMap[props.hostName]} />
        <Show when={state.operationFailures[props.hostName] !== undefined}>
            <span
                class="ml-1.5 inline-flex"
                title={`Last ${state.operationFailures[props.hostName]?.operation} command failed`}
            >
                <TriangleAlert
                    size={16}
                    class="text-amber-600 dark:text-[rgba(245,158,11,0.9)]"
                    aria-label={`Last ${state.operationFailures[props.hostName]?.operation} command failed`}
                    role="img"
                />
            </span>
        </Show>
    </>
)) satisfies AnyComponent;

const HostLeaseButtons = ((props: { hostName: string }) => {
    const hasClients = () => state.clients.length > 0;
    return (
        <>
            <button
                class="btn btn-height btn-green take-lease"
                type="button"
                onClick={() => updateLease(props.hostName, 'take')}
                aria-label={hasClients() ? 'Take Lease' : 'Start'}
            >
                <Power size={14} aria-hidden="true" />
                {hasClients() ? 'Take Lease' : 'Start'}
            </button>
            <button
                class="btn btn-height btn-red release-lease"
                type="button"
                onClick={() => updateLease(props.hostName, 'release')}
                aria-label={hasClients() ? 'Release Lease' : 'Shutdown'}
            >
                <PowerOff size={14} aria-hidden="true" />
                {hasClients() ? 'Release Lease' : 'Shutdown'}
            </button>
        </>
    );
}) satisfies AnyComponent;

// ==========================
// HostRow
// ==========================

const HostRow = ((props: { hostName: string }) => {
    const leases = () => state.leaseMap[props.hostName] ?? [];
    const hasWebInterfaceLease = () =>
        leases().some((l) => l.type === 'WebInterface');
    const hasClients = () => state.clients.length > 0;

    return (
        <tr
            class="table-row"
            data-hostname={props.hostName}
            data-has-lease={String(hasWebInterfaceLease())}
        >
            <th class="table-cell" scope="row">
                <HostNameLink
                    hostName={props.hostName}
                    class="block btn-height"
                />
            </th>
            <td class="table-cell status" aria-label="Status">
                <HostStatusDisplay hostName={props.hostName} />
            </td>
            <td class="table-cell actions" aria-label="Actions">
                <div class="actions-cell">
                    <HostLeaseButtons hostName={props.hostName} />
                </div>
            </td>
            <Show when={hasClients()}>
                <td class="table-cell leases" aria-label="Leases">
                    {getFormattedLeases(leases())}
                </td>
            </Show>
        </tr>
    );
}) satisfies AnyComponent;

// ==========================
// HostCard (mobile)
// ==========================

const HostCard = ((props: { hostName: string }) => {
    const navigate = useNavigate();
    const leases = () => state.leaseMap[props.hostName] ?? [];
    const hasWebInterfaceLease = () =>
        leases().some((l) => l.type === 'WebInterface');
    const hasClients = () => state.clients.length > 0;

    const navitateToDetail = (e: { target: Element }) => {
        if (!e.target.closest('a, button')) {
            navigate(`/hosts/${props.hostName}`);
        }
    };

    return (
        <li
            class="actions-card cursor-pointer"
            data-hostname={props.hostName}
            data-has-lease={String(hasWebInterfaceLease())}
            onClick={navitateToDetail}
            onKeyUp={(e) => {
                if (e.key === 'Enter' || e.key === ' ') navitateToDetail(e);
            }}
        >
            <div class="actions-card-header">
                <HostNameLink hostName={props.hostName} class="font-medium" />
                <span class="actions-card-status">
                    <HostStatusDisplay hostName={props.hostName} />
                </span>
            </div>
            <Show when={hasClients()}>
                <p class="actions-card-row">
                    <span class="actions-card-label">Leases: </span>
                    {getFormattedLeases(leases())}
                </p>
            </Show>
            <div class="actions-cell">
                <HostLeaseButtons hostName={props.hostName} />
            </div>
        </li>
    );
}) satisfies AnyComponent;

export const HostsPage = (() => {
    const sortedHosts = createMemo(() =>
        sortActiveFirst(
            state.hosts,
            (h) => h in state.statusMap,
            (h) => h,
        ),
    );

    const cmds = createMemo(() => makeInstallCommands());
    const hasClients = () => state.clients.length > 0;
    const actionReserveLabel = createMemo(() => {
        const labels = hasClients()
            ? ['Take Lease', 'Release Lease']
            : ['Start', 'Shutdown'];
        return labels.reduce(
            (longest, label) =>
                label.length > longest.length ? label : longest,
            labels[0] ?? 'Start',
        );
    });

    return (
        <AppLayout>
            {/* Install Instructions Panel */}
            <section
                class="section-container mt-0 py-0"
                aria-labelledby="host-install-title"
            >
                <details
                    class="collapsible-details"
                    aria-labelledby="host-install-title"
                >
                    <summary
                        class="collapsible-header py-2"
                        aria-controls="host-install-content"
                        id="host-install-header"
                    >
                        <h2
                            class="section-title mb-0 text-base"
                            id="host-install-title"
                        >
                            Install Host Agent
                        </h2>
                        <span class="collapsible-icon" aria-hidden="true" />
                    </summary>
                    {/* biome-ignore lint/a11y/useSemanticElements: role="group" has no semantic HTML element equivalent outside of form contexts */}
                    <div
                        id="host-install-content"
                        class="collapsible-content"
                        role="group"
                        aria-labelledby="host-install-title"
                    >
                        <p class="mb-1 text-sm">
                            Run one of the following commands in your terminal:
                        </p>

                        <p class="mb-1 text-xs font-semibold">
                            For Linux/macOS:
                        </p>
                        <div class="code-container py-2">
                            <CopyButton
                                targetId="host-install-command-sh"
                                label="Copy install command"
                            />
                            <code
                                id="host-install-command-sh"
                                class="code-block"
                            >
                                {cmds().hostSh}
                            </code>
                        </div>

                        <p class="mb-1 text-xs font-semibold">
                            For Windows (PowerShell):
                        </p>
                        <div class="code-container py-2">
                            <CopyButton
                                targetId="host-install-command-ps1"
                                label="Copy install command"
                            />
                            <code
                                id="host-install-command-ps1"
                                class="code-block"
                            >
                                {cmds().hostPs1}
                            </code>
                        </div>

                        <p class="description-text text-xs">
                            Adjust options as needed. The command will already
                            include the configured broadcast port, so no manual
                            change is required. Add the output to the hosts
                            section of your config on the Coordinator Host:
                        </p>
                        <div class="code-container">
                            <CopyButton
                                targetId="host-config-location"
                                label="Copy config location"
                            />
                            <code
                                id="host-config-location"
                                data-config-location
                                class="code-block"
                            >
                                {serverData.configPath}
                            </code>
                        </div>

                        {/* Inlined at build time from partials/agent_install_requirements_gotchas.md */}
                        <div innerHTML={agentGotchasHtml} />
                    </div>
                </details>
            </section>

            {/* Hosts Table */}
            <section
                class="section-container mt-4"
                aria-labelledby="hosts-table-title"
            >
                <h2 id="hosts-table-title" class="sr-only">
                    Hosts Table
                </h2>
                {/* Mobile card list */}
                <ul
                    id="host-card-list"
                    class="flex flex-col gap-3 md:hidden"
                    aria-live="polite"
                >
                    <For each={sortedHosts()}>
                        {(hostName) => <HostCard hostName={hostName} />}
                    </For>
                </ul>
                {/* Desktop table */}
                <div class="table-wrapper hidden md:block">
                    <table
                        class="actions-table w-full"
                        aria-describedby="hosts-table-title"
                    >
                        <thead>
                            <tr id="host-table-header">
                                <th class="table-header" scope="col">
                                    Host
                                </th>
                                <th
                                    class="table-header status-column"
                                    scope="col"
                                >
                                    <span class="reserve-container">
                                        <span>Status</span>
                                        <span
                                            aria-hidden="true"
                                            class="reserve-label"
                                        >
                                            {statusReserveLabel}
                                        </span>
                                    </span>
                                </th>
                                <th
                                    class="table-header actions-column"
                                    scope="col"
                                >
                                    <span class="reserve-container">
                                        <span>Actions</span>
                                        <span
                                            aria-hidden="true"
                                            class="reserve-label"
                                        >
                                            {actionReserveLabel()}
                                        </span>
                                    </span>
                                </th>
                                <Show when={hasClients()}>
                                    <th
                                        class="table-header"
                                        id="host-table-leases-header"
                                        scope="col"
                                    >
                                        Leases
                                    </th>
                                </Show>
                            </tr>
                        </thead>
                        <tbody
                            id="host-table-body"
                            class="divide-y divide-gray-200"
                            aria-live="polite"
                        >
                            <For each={sortedHosts()}>
                                {(hostName) => <HostRow hostName={hostName} />}
                            </For>
                        </tbody>
                    </table>
                </div>
            </section>
        </AppLayout>
    );
}) satisfies AnyComponent;
