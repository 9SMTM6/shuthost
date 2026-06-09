import { Power, PowerOff } from 'lucide-solid';
import { For, Show } from 'solid-js';
import { apiFetch } from '../../helpers/apiFetch';
import {
    type ClientLease,
    type LeaseSource,
    state,
} from '../../helpers/appStore';
import type { AnyComponent } from '../../helpers/utils';

export const HostLeasesSection = ((props: { hostname: string }) => {
    const leases = () =>
        state.leaseMap[props.hostname] ?? ([] as LeaseSource[]);
    const hasWebInterfaceLease = () =>
        leases().some((l) => l.type === 'WebInterface');
    const clientLeases = () =>
        leases().filter((l): l is ClientLease => l.type === 'Client');

    const updateLease = async (action: 'take' | 'release') => {
        try {
            await apiFetch(`/api/lease/${props.hostname}/${action}`, {
                method: 'POST',
            });
        } catch (err) {
            if (err instanceof Error && err.message === 'Unauthorized') return;
            console.error(
                `Failed to ${action} lease for ${props.hostname}:`,
                err,
            );
        }
    };

    return (
        <section
            class="section-container mb-4"
            aria-labelledby="host-leases-title"
        >
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
                        <tr
                            class="table-row"
                            data-has-lease={String(hasWebInterfaceLease())}
                        >
                            <th class="table-cell" scope="row">
                                <span class="block">Web Interface</span>
                                <Show when={!hasWebInterfaceLease()}>
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
                                        onClick={() => updateLease('take')}
                                        aria-label="Take web interface lease"
                                    >
                                        <Power size={14} aria-hidden="true" />
                                        Take
                                    </button>
                                    <button
                                        class="btn btn-height btn-red release-lease"
                                        type="button"
                                        onClick={() => updateLease('release')}
                                        aria-label="Release web interface lease"
                                    >
                                        <PowerOff
                                            size={14}
                                            aria-hidden="true"
                                        />
                                        Release
                                    </button>
                                </div>
                            </td>
                        </tr>

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
    );
}) satisfies AnyComponent;
