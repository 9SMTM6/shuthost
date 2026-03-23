import type { Component } from 'solid-js';
import { For, Show, createMemo } from 'solid-js';
import clientGotchasHtml from '../client_install_requirements_gotchas.md?raw';
import { state, applyMessage } from '../stores/appStore';
import { serverData } from '../serverData';
import { demoSubpath } from '../demo';
import { apiFetch, sortActiveFirst } from './HostsTab';
import { CopyButton } from './CopyButton';

const RTF = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

const formatLastUsed = (clientId: string): string => {
    if (state.clientStats === null) return '';
    const stats = state.clientStats[clientId];
    if (!stats?.last_used) return 'Never';
    const date = new Date(stats.last_used);
    const diffMs = Date.now() - date.getTime();
    const oneYearMs = 365 * 24 * 60 * 60 * 1000;
    if (diffMs >= oneYearMs) return date.toLocaleString();
    const seconds = Math.round(diffMs / 1000);
    if (seconds < 45) return 'just now';
    if (seconds < 90) return RTF.format(-1, 'minute');
    const minutes = Math.round(seconds / 60);
    if (minutes < 60) return RTF.format(-minutes, 'minute');
    const hours = Math.round(minutes / 60);
    if (hours < 24) return RTF.format(-hours, 'hour');
    const days = Math.round(hours / 24);
    if (days < 7) return RTF.format(-days, 'day');
    if (days < 30) return RTF.format(-Math.round(days / 7), 'week');
    const months = Math.round(days / 30);
    if (months < 12) return RTF.format(-months, 'month');
    return date.toLocaleString();
};

// ==========================
// ClientRow
// ==========================

const ClientRow: Component<{ clientId: string; leases: string[] }> = (props) => {
    const resetLeases = async () => {
        if (serverData.isDemo) {
            // Demo: clear leases out of the store directly
            const newLeaseMap = { ...state.leaseMap };
            for (const host of Object.keys(newLeaseMap)) {
                newLeaseMap[host] = (newLeaseMap[host] ?? []).filter(
                    l => l.type !== 'Client' || l.value !== props.clientId,
                );
            }
            applyMessage({ type: 'ConfigChanged', payload: { hosts: state.hosts, clients: state.clients } });
            // Force a LeaseUpdate for each host to clear the demo state
            for (const host of Object.keys(newLeaseMap)) {
                applyMessage({ type: 'LeaseUpdate', payload: { host, leases: newLeaseMap[host] ?? [] } });
            }
            return;
        }
        try {
            await apiFetch(`/api/reset_leases/${props.clientId}`, { method: 'POST' });
        } catch (err) {
            if (err instanceof Error && err.message === 'Unauthorized') return;
            console.error(`Failed to reset leases for client ${props.clientId}:`, err);
        }
    };

    return (
        <tr class="table-row" role="row" data-client-id={props.clientId}>
            <th class="table-cell" scope="row">{props.clientId}</th>
            <td class="table-cell leases" aria-label="Leases">{props.leases.join(', ') || 'None'}</td>
            <Show when={state.clientStats !== null}>
                <td class="table-cell last-used" aria-label="Last Used">{formatLastUsed(props.clientId)}</td>
            </Show>
            <td class="table-cell" aria-label="Actions">
                <div class="actions-cell">
                    <button
                        class="btn btn-red reset-client"
                        type="button"
                        disabled={props.leases.length === 0}
                        onClick={resetLeases}
                        aria-label="Reset Leases"
                    >
                        Reset Leases
                    </button>
                </div>
            </td>
        </tr>
    );
};

// ==========================
// ClientsTab
// ==========================

const makeClientCommands = () => {
    const baseUrl = window.location.origin + (serverData.isDemo ? demoSubpath : '');
    return {
        clientSh: `curl -sSL ${baseUrl}/download/client_installer.sh | sh -s ${baseUrl}`,
        clientPs1: `curl.exe -sSLO '${baseUrl}/download/client_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\client_installer.ps1 ${baseUrl}`,
    };
};

export const ClientsTab: Component<{ configPath: string }> = (props) => {
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
        )
    );

    const cmds = createMemo(makeClientCommands);

    return (
        <section id="clients-tab" class="tab-content" aria-labelledby="tab-clients" role="tabpanel" tabindex="0">
            {/* Install Instructions Panel */}
            <section class="section-container mt-0 py-0" aria-labelledby="client-install-title">
                <details class="collapsible-details" aria-labelledby="client-install-title">
                    <summary class="collapsible-header py-2" aria-controls="client-install-content" id="client-install-header">
                        <h2 class="section-title mb-0 text-base" id="client-install-title">Install Client</h2>
                        <span class="collapsible-icon" aria-hidden="true" />
                    </summary>
                    <div id="client-install-content" class="collapsible-content" role="group" aria-labelledby="client-install-title">
                        <p class="mb-1 text-sm">Run one of the following commands in your terminal:</p>

                        <p class="mb-1 text-xs font-semibold">For Linux/macOS:</p>
                        <div class="code-container py-2">
                            <CopyButton targetId="client-install-command-sh" label="Copy install command" />
                            <code id="client-install-command-sh" class="code-block">{cmds().clientSh}</code>
                        </div>

                        <p class="mb-1 text-xs font-semibold">For Windows (PowerShell):</p>
                        <div class="code-container py-2">
                            <CopyButton targetId="client-install-command-ps1" label="Copy install command" />
                            <code id="client-install-command-ps1" class="code-block">{cmds().clientPs1}</code>
                        </div>

                        <p class="description-text text-xs">
                            Optionally specify a custom base client ID as the second argument (otherwise random). The
                            full client ID will include your hostname. <strong>Tip:</strong> Use separate clients for
                            different use cases.
                        </p>
                        <p class="description-text text-xs">
                            Then, add the output to the clients section of your config on the Coordinator Host:
                        </p>
                        <div class="code-container">
                            <CopyButton targetId="client-config-location" label="Copy config location" />
                            <code id="client-config-location" data-config-location class="code-block">{props.configPath}</code>
                        </div>

                        {/* Inlined at build time from client_install_requirements_gotchas.md */}
                        <div innerHTML={clientGotchasHtml} />
                    </div>
                </details>
            </section>

            {/* Clients Table */}
            <section class="section-container mt-4" aria-labelledby="clients-table-title">
                <h2 id="clients-table-title" class="sr-only">Clients Table</h2>
                <div class="table-wrapper">
                    <table class="actions-table w-full" aria-describedby="clients-table-title">
                        <thead>
                            <tr>
                                <th class="table-header" scope="col">Client ID</th>
                                <th class="table-header" scope="col">Leases</th>
                                <Show when={state.clientStats !== null}>
                                    <th id="last-used-header" class="table-header" scope="col">Last Used</th>
                                </Show>
                                <th class="table-header" scope="col">Actions</th>
                            </tr>
                        </thead>
                        <tbody id="client-table-body" class="divide-y divide-gray-200" aria-live="polite">
                            <For each={sortedClients()}>
                                {([clientId, leases]) => <ClientRow clientId={clientId} leases={leases} />}
                            </For>
                        </tbody>
                    </table>
                </div>
            </section>
        </section>
    );
};
