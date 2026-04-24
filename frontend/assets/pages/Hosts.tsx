import { A } from '@solidjs/router';
import { Power, PowerOff } from 'lucide-solid';
import { createMemo, For, Show } from 'solid-js';
import { AppLayout } from '../components/App';
import { CopyButton } from '../components/CopyButton';
import { apiFetch } from '../helpers/apiFetch';
import type { LeaseSource, Status } from '../helpers/appStore';
import { state } from '../helpers/appStore';
import { demoSubpath, demoUpdateLease, isDemoMode } from '../helpers/demo';
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
    shutting_down: 'shutting down',
} as const satisfies Record<Status, string>;

const getStatusLabel = (status?: Status) =>
    status === undefined ? 'Loading...' : statusDisplayMap[status];

const statusReserveLabel = (
    [getStatusLabel(), ...Object.values(statusDisplayMap)] as const
).reduce(
    (longest, label) =>
        label.length > longest.length ? label : longest,
    getStatusLabel(),
);

const actionReserveLabel = [
    'Start',
    'Take Lease',
    'Shutdown',
    'Release Lease',
].reduce((longest, label) =>
    label.length > longest.length ? label : longest,
    'Start',
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
// HostRow
// ==========================

const HostRow = ((props: { hostName: string }) => {
    const leases = () => state.leaseMap[props.hostName] ?? [];
    const status = () => getStatusLabel(state.statusMap[props.hostName]);
    const hasWebInterfaceLease = () =>
        leases().some((l) => l.type === 'WebInterface');
    const hasClients = () => state.clients.length > 0;

    const updateLease = async (action: 'take' | 'release') => {
        if (isDemoMode) {
            await demoUpdateLease(props.hostName, action);
            return;
        }
        try {
            await apiFetch(`/api/lease/${props.hostName}/${action}`, {
                method: 'POST',
            });
        } catch (err) {
            if (err instanceof Error && err.message === 'Unauthorized') return;
            console.error(
                `Failed to ${action} lease for ${props.hostName}:`,
                err,
            );
        }
    };

    return (
        <tr
            class="table-row"
            data-hostname={props.hostName}
            data-has-lease={String(hasWebInterfaceLease())}
        >
            <th class="table-cell" scope="row">
                <A href={`/hosts/${props.hostName}`} class="link block">
                    {props.hostName}
                </A>
            </th>
            <td class="table-cell status" aria-label="Status">
                {status()}
            </td>
            <Show when={hasClients()}>
                <td class="table-cell leases" aria-label="Leases">
                    {getFormattedLeases(leases())}
                </td>
            </Show>
            <td class="table-cell actions" aria-label="Actions">
                <div class="actions-cell">
                    <button
                        class="btn btn-green take-lease"
                        type="button"
                        onClick={() => updateLease('take')}
                        aria-label={hasClients() ? 'Take Lease' : 'Start'}
                    >
                        <Power size={14} aria-hidden="true" />
                        {hasClients() ? 'Take Lease' : 'Start'}
                    </button>
                    <button
                        class="btn btn-red release-lease"
                        type="button"
                        onClick={() => updateLease('release')}
                        aria-label={hasClients() ? 'Release Lease' : 'Shutdown'}
                    >
                        <PowerOff size={14} aria-hidden="true" />
                        {hasClients() ? 'Release Lease' : 'Shutdown'}
                    </button>
                </div>
            </td>
        </tr>
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
                <div class="table-wrapper">
                    <table
                        class="actions-table w-full"
                        aria-describedby="hosts-table-title"
                    >
                        <thead>
                            <tr id="host-table-header">
                                <th class="table-header" scope="col">
                                    Host
                                </th>
                                <th class="table-header status-column" scope="col">
                                    Status
                                    <span aria-hidden="true" class="reserve-label">
                                        {statusReserveLabel}
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
                                <th class="table-header actions-column" scope="col">
                                    Actions
                                    <span aria-hidden="true" class="reserve-label">
                                        {actionReserveLabel}
                                    </span>
                                </th>
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
