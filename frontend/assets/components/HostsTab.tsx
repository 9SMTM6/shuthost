import type { Component } from 'solid-js';
import { For, Show, createMemo } from 'solid-js';
import agentGotchasHtml from '../agent_install_requirements_gotchas.md?raw';
import { state } from '../stores/appStore';
import type { LeaseSource } from '../stores/appStore';
import { serverData } from '../serverData';
import { demoSubpath, demoUpdateLease } from '../demo';
import { CopyButton } from './CopyButton';

// ==========================
// Helpers (shared with ClientsTab)
// ==========================

export const apiFetch = async (url: string, options?: RequestInit): Promise<Response> => {
    const resp = await fetch(url, options);
    if (resp.status === 401) {
        window.location.assign('/login');
        throw new Error('Unauthorized');
    }
    if (!resp.ok) {
        const msg = `HTTP ${resp.status}: ${resp.statusText}`;
        const errorDiv = document.getElementById('js-error') as HTMLDivElement | null;
        const messageEl = document.getElementById('js-error-message') as HTMLParagraphElement | null;
        if (errorDiv && messageEl) { messageEl.textContent = msg; errorDiv.hidden = false; }
        throw new Error(msg);
    }
    return resp;
};

const formatLeaseSource = (lease: LeaseSource): string =>
    lease.type === 'Client' ? lease.value : '';

export const getFormattedLeases = (leases: LeaseSource[]): string => {
    const clientLeases = leases.filter(l => l.type === 'Client');
    return clientLeases.length > 0 ? clientLeases.map(formatLeaseSource).join(', ') : 'None';
};

export const sortActiveFirst = <T,>(
    items: T[],
    isActive: (item: T) => boolean,
    getName: (item: T) => string,
): T[] => {
    const compare = (a: T, b: T) => getName(a).localeCompare(getName(b));
    return [
        ...items.filter(isActive).toSorted(compare),
        ...items.filter(i => !isActive(i)).toSorted(compare),
    ];
};

// ==========================
// Installer commands
// ==========================

const makeInstallCommands = (broadcastPort: number | undefined) => {
    const baseUrl = window.location.origin + (serverData.isDemo ? demoSubpath : '');
    const bpArg = broadcastPort !== undefined ? ` -- --broadcast-port ${broadcastPort}` : '';
    return {
        hostSh: `curl -fsSL ${baseUrl}/download/host_agent_installer.sh | sh -s ${baseUrl}${bpArg}`,
        hostPs1: `curl.exe -sSLO '${baseUrl}/download/host_agent_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\host_agent_installer.ps1 ${baseUrl}${bpArg}`,
    };
};

// ==========================
// HostRow
// ==========================

const HostRow: Component<{ hostName: string }> = (props) => {
    const leases = () => state.leaseMap[props.hostName] ?? [];
    const status = () => state.statusMap[props.hostName] ?? 'Loading...';
    const hasWebInterfaceLease = () => leases().some(l => l.type === 'WebInterface');
    const hasClients = () => state.clients.length > 0;

    const updateLease = async (action: 'take' | 'release') => {
        if (serverData.isDemo) { await demoUpdateLease(props.hostName, action); return; }
        try {
            await apiFetch(`/api/lease/${props.hostName}/${action}`, { method: 'POST' });
        } catch (err) {
            if (err instanceof Error && err.message === 'Unauthorized') return;
            console.error(`Failed to ${action} lease for ${props.hostName}:`, err);
        }
    };

    return (
        <tr class="table-row" role="row" data-hostname={props.hostName} data-has-lease={String(hasWebInterfaceLease())}>
            <th class="table-cell" scope="row">{props.hostName}</th>
            <td class="table-cell status" aria-label="Status">{status()}</td>
            <Show when={hasClients()}>
                <td class="table-cell leases" aria-label="Leases">{getFormattedLeases(leases())}</td>
            </Show>
            <td class="table-cell" aria-label="Actions">
                <div class="actions-cell">
                    <button
                        class="btn btn-green take-lease"
                        type="button"
                        onClick={() => updateLease('take')}
                        aria-label={hasClients() ? 'Take Lease' : 'Start'}
                    >
                        {hasClients() ? 'Take Lease' : 'Start'}
                    </button>
                    <button
                        class="btn btn-red release-lease"
                        type="button"
                        onClick={() => updateLease('release')}
                        aria-label={hasClients() ? 'Release Lease' : 'Shutdown'}
                    >
                        {hasClients() ? 'Release Lease' : 'Shutdown'}
                    </button>
                </div>
            </td>
        </tr>
    );
};

// ==========================
// HostsTab
// ==========================

export const HostsTab: Component<{ configPath: string }> = (props) => {
    const sortedHosts = createMemo(() =>
        sortActiveFirst(
            state.hosts,
            h => h in state.statusMap,
            h => h,
        )
    );

    const cmds = createMemo(() => makeInstallCommands(state.broadcastPort));
    const hasClients = () => state.clients.length > 0;

    return (
        <section id="hosts-tab" class="tab-content active" aria-labelledby="tab-hosts" role="tabpanel" tabindex="0">
            {/* Install Instructions Panel */}
            <section class="section-container mt-0 py-0" aria-labelledby="host-install-title">
                <details class="collapsible-details" aria-labelledby="host-install-title">
                    <summary class="collapsible-header py-2" aria-controls="host-install-content" id="host-install-header">
                        <h2 class="section-title mb-0 text-base" id="host-install-title">Install Host Agent</h2>
                        <span class="collapsible-icon" aria-hidden="true" />
                    </summary>
                    <div id="host-install-content" class="collapsible-content" role="group" aria-labelledby="host-install-title">
                        <p class="mb-1 text-sm">Run one of the following commands in your terminal:</p>

                        <p class="mb-1 text-xs font-semibold">For Linux/macOS:</p>
                        <div class="code-container py-2">
                            <CopyButton targetId="host-install-command-sh" label="Copy install command" />
                            <code id="host-install-command-sh" class="code-block">{cmds().hostSh}</code>
                        </div>

                        <p class="mb-1 text-xs font-semibold">For Windows (PowerShell):</p>
                        <div class="code-container py-2">
                            <CopyButton targetId="host-install-command-ps1" label="Copy install command" />
                            <code id="host-install-command-ps1" class="code-block">{cmds().hostPs1}</code>
                        </div>

                        <p class="description-text text-xs">
                            Adjust options as needed. The command will already include the configured broadcast port,
                            so no manual change is required. Add the output to the hosts section of your config on the
                            Coordinator Host:
                        </p>
                        <div class="code-container">
                            <CopyButton targetId="host-config-location" label="Copy config location" />
                            <code id="host-config-location" class="code-block">{props.configPath}</code>
                        </div>

                        {/* Inlined at build time from agent_install_requirements_gotchas.md */}
                        <div innerHTML={agentGotchasHtml} />
                    </div>
                </details>
            </section>

            {/* Hosts Table */}
            <section class="section-container mt-4" aria-labelledby="hosts-table-title">
                <h2 id="hosts-table-title" class="sr-only">Hosts Table</h2>
                <div class="table-wrapper">
                    <table class="actions-table w-full" aria-describedby="hosts-table-title">
                        <thead>
                            <tr id="host-table-header">
                                <th class="table-header" scope="col">Host</th>
                                <th class="table-header" scope="col">Status</th>
                                <Show when={hasClients()}>
                                    <th class="table-header" id="host-table-leases-header" scope="col">Leases</th>
                                </Show>
                                <th class="table-header" scope="col">Actions</th>
                            </tr>
                        </thead>
                        <tbody id="host-table-body" class="divide-y divide-gray-200" aria-live="polite">
                            <For each={sortedHosts()}>
                                {(hostName) => <HostRow hostName={hostName} />}
                            </For>
                        </tbody>
                    </table>
                </div>
            </section>
        </section>
    );
};
