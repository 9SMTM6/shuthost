import { RotateCcw } from 'lucide-solid';
import { createMemo, For, Show } from 'solid-js';
import { AppLayout } from '../components/App';
import { CopyButton } from '../components/CopyButton';
import { apiFetch } from '../helpers/apiFetch';
import { applyMessage, state } from '../helpers/appStore';
import { demoSubpath, isDemoMode } from '../helpers/demo';
import { serverData } from '../helpers/serverData';
import type { AnyComponent } from '../helpers/utils';
import { formatRelativeTimestamp, sortActiveFirst } from '../helpers/utils';
import clientGotchasHtml from '../partials/client_install_requirements_gotchas.md?raw';

const formatLastUsed = (clientId: string): string => {
    if (state.dbData.status !== 'available') return '';
    const stats = state.dbData.payload.clientStats[clientId];
    return formatRelativeTimestamp(stats?.lastUsed);
};

// ==========================
// Shared client helpers
// ==========================

const resetLeases = async (clientId: string) => {
    if (isDemoMode) {
        // Demo: clear leases out of the store directly
        const newLeaseMap = { ...state.leaseMap };
        for (const host of Object.keys(newLeaseMap)) {
            newLeaseMap[host] = (newLeaseMap[host] ?? []).filter(
                (l) => l.type !== 'Client' || l.value !== clientId,
            );
        }
        applyMessage({
            type: 'ConfigChanged',
            payload: {
                hosts: state.hosts,
                clients: state.clients,
                hostConfigMap: state.hostConfigMap,
            },
        });
        // Force a LeaseUpdate for each host to clear the demo state
        for (const host of Object.keys(newLeaseMap)) {
            applyMessage({
                type: 'LeaseUpdate',
                payload: { host, leases: newLeaseMap[host] ?? [] },
            });
        }
        return;
    }
    try {
        await apiFetch(`/api/reset_leases/${clientId}`, {
            method: 'POST',
        });
    } catch (err) {
        if (err instanceof Error && err.message === 'Unauthorized') return;
        console.error(`Failed to reset leases for client ${clientId}:`, err);
    }
};

// ==========================
// ClientResetButton
// ==========================

const ClientResetButton = ((props: { clientId: string; leases: string[] }) => (
    <div class="actions-cell">
        <button
            class="btn btn-height btn-red reset-client"
            type="button"
            disabled={props.leases.length === 0}
            onClick={() => resetLeases(props.clientId)}
            aria-label="Reset Leases"
        >
            <RotateCcw size={14} aria-hidden="true" />
            Reset Leases
        </button>
    </div>
)) satisfies AnyComponent;

// ==========================
// ClientRow
// ==========================

const ClientRow = ((props: { clientId: string; leases: string[] }) => (
    <tr class="table-row" data-client-id={props.clientId}>
        <th class="table-cell" scope="row">
            {props.clientId}
        </th>
        <td class="table-cell leases" aria-label="Leases">
            {props.leases.join(', ') || 'None'}
        </td>
        <Show when={state.dbData.status === 'available'}>
            <td class="table-cell last-used" aria-label="Last Used">
                {formatLastUsed(props.clientId)}
            </td>
        </Show>
        <td class="table-cell" aria-label="Actions">
            <ClientResetButton
                clientId={props.clientId}
                leases={props.leases}
            />
        </td>
    </tr>
)) satisfies AnyComponent;

// ==========================
// ClientCard (mobile)
// ==========================

const ClientCard = ((props: { clientId: string; leases: string[] }) => (
    <li class="actions-card" data-client-id={props.clientId}>
        <p class="actions-card-id">{props.clientId}</p>
        <p class="actions-card-row">
            <span class="actions-card-label">Leases: </span>
            {props.leases.join(', ') || 'None'}
        </p>
        <Show when={state.dbData.status === 'available'}>
            <p class="actions-card-row">
                <span class="actions-card-label">Last Used: </span>
                {formatLastUsed(props.clientId)}
            </p>
        </Show>
        <ClientResetButton clientId={props.clientId} leases={props.leases} />
    </li>
)) satisfies AnyComponent;

const makeClientCommands = () => {
    const baseUrl = window.location.origin + demoSubpath;
    return {
        clientSh: `curl -sSL ${baseUrl}/download/client_installer.sh | sh -s ${baseUrl}`,
        clientPs1: `curl.exe -sSLO '${baseUrl}/download/client_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\client_installer.ps1 ${baseUrl}`,
    };
};

export const ClientsPage = (() => {
    // Build a map of clientId -> [hosts with that client's lease]
    const clientLeaseMap = createMemo(() => {
        const map = new Map<string, string[]>();
        for (const [host, leases] of Object.entries(state.leaseMap)) {
            for (const lease of leases) {
                if (lease.type === 'Client') {
                    const existing = map.get(lease.value) ?? [];
                    existing.push(host);
                    map.set(lease.value, existing);
                }
            }
        }
        for (const clientId of state.clients) {
            if (!map.has(clientId)) map.set(clientId, []);
        }
        return map;
    });

    const sortedClients = createMemo(() =>
        sortActiveFirst(
            Array.from(clientLeaseMap().entries()),
            ([, leases]) => leases.length > 0,
            ([id]) => id,
        ),
    );

    const cmds = createMemo(makeClientCommands);

    return (
        <AppLayout>
            {/* Install Instructions Panel */}
            <section
                class="section-container mt-0 py-0"
                aria-labelledby="client-install-title"
            >
                <details
                    class="collapsible-details"
                    aria-labelledby="client-install-title"
                >
                    <summary
                        class="collapsible-header py-2"
                        aria-controls="client-install-content"
                        id="client-install-header"
                    >
                        <h2
                            class="section-title mb-0 text-base"
                            id="client-install-title"
                        >
                            Install Client
                        </h2>
                        <span class="collapsible-icon" aria-hidden="true" />
                    </summary>
                    {/* biome-ignore lint/a11y/useSemanticElements: role="group" has no semantic HTML element equivalent outside of form contexts */}
                    <div
                        id="client-install-content"
                        class="collapsible-content"
                        role="group"
                        aria-labelledby="client-install-title"
                    >
                        <p class="mb-1 text-sm">
                            Run one of the following commands in your terminal:
                        </p>

                        <p class="mb-1 text-xs font-semibold">
                            For Linux/macOS:
                        </p>
                        <div class="code-container py-2">
                            <CopyButton
                                targetId="client-install-command-sh"
                                label="Copy install command"
                            />
                            <code
                                id="client-install-command-sh"
                                class="code-block"
                            >
                                {cmds().clientSh}
                            </code>
                        </div>

                        <p class="mb-1 text-xs font-semibold">
                            For Windows (PowerShell):
                        </p>
                        <div class="code-container py-2">
                            <CopyButton
                                targetId="client-install-command-ps1"
                                label="Copy install command"
                            />
                            <code
                                id="client-install-command-ps1"
                                class="code-block"
                            >
                                {cmds().clientPs1}
                            </code>
                        </div>

                        <p class="description-text text-xs">
                            Optionally specify a custom base client ID as the
                            second argument (otherwise random). The full client
                            ID will include your hostname. <strong>Tip:</strong>{' '}
                            Use separate clients for different use cases.
                        </p>
                        <p class="description-text text-xs">
                            Then, add the output to the clients section of your
                            config on the Coordinator Host:
                        </p>
                        <div class="code-container">
                            <CopyButton
                                targetId="client-config-location"
                                label="Copy config location"
                            />
                            <code
                                id="client-config-location"
                                data-config-location
                                class="code-block"
                            >
                                {serverData.configPath}
                            </code>
                        </div>

                        {/* Inlined at build time from partials/client_install_requirements_gotchas.md */}
                        <div innerHTML={clientGotchasHtml} />
                    </div>
                </details>
            </section>

            {/* Clients Table */}
            <section
                class="section-container mt-4"
                aria-labelledby="clients-table-title"
            >
                <h2 id="clients-table-title" class="sr-only">
                    Clients Table
                </h2>
                {/* Mobile card list */}
                <ul
                    id="client-card-list"
                    class="flex flex-col gap-3 py-3 md:hidden"
                    aria-live="polite"
                >
                    <For each={sortedClients()}>
                        {([clientId, leases]) => (
                            <ClientCard clientId={clientId} leases={leases} />
                        )}
                    </For>
                </ul>
                {/* Desktop table */}
                <div class="table-wrapper hidden md:block">
                    <table
                        class="actions-table w-full"
                        aria-describedby="clients-table-title"
                    >
                        <thead>
                            <tr>
                                <th class="table-header" scope="col">
                                    Client ID
                                </th>
                                <th class="table-header" scope="col">
                                    Leases
                                </th>
                                <Show
                                    when={state.dbData.status === 'available'}
                                >
                                    <th
                                        id="last-used-header"
                                        class="table-header"
                                        scope="col"
                                    >
                                        Last Used
                                    </th>
                                </Show>
                                <th class="table-header" scope="col">
                                    Actions
                                </th>
                            </tr>
                        </thead>
                        <tbody
                            id="client-table-body"
                            class="divide-y divide-gray-200"
                            aria-live="polite"
                        >
                            <For each={sortedClients()}>
                                {([clientId, leases]) => (
                                    <ClientRow
                                        clientId={clientId}
                                        leases={leases}
                                    />
                                )}
                            </For>
                        </tbody>
                    </table>
                </div>
            </section>
        </AppLayout>
    );
}) satisfies AnyComponent;
