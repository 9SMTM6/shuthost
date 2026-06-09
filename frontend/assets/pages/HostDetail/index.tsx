import { Title } from '@solidjs/meta';
import { A, useParams } from '@solidjs/router';
import { Match, Show, Switch } from 'solid-js';
import { AppLayout } from '../../components/App';
import { HostStatusBadge } from '../../components/HostStatusBadge';
import { type OperationFailure, state } from '../../helpers/appStore';
import type { AnyComponent } from '../../helpers/utils';
import { HostInfoSection } from './HostInfoSection';
import { HostLeasesSection } from './HostLeasesSection';
import { NotificationSection } from './NotificationSection';

const OperationFailureBadge = ((props: {
    failure: OperationFailure | undefined;
}) => (
    <Show when={props.failure !== undefined}>
        <span class="host-status-badge bg-amber-100 text-amber-800 dark:bg-[rgba(245,158,11,0.15)] dark:text-[rgba(245,158,11,0.9)]">
            {props.failure?.operation} failed
        </span>
    </Show>
)) satisfies AnyComponent;

export const HostDetailPage = (() => {
    const params = useParams<{ hostname: string }>();
    const hostname = () => params.hostname;

    const isLoading = () => state.hosts.length === 0;
    const isKnown = () => state.hosts.includes(hostname());
    const status = () => state.statusMap[hostname()];
    const hostStats = () =>
        state.dbData.status === 'available'
            ? state.dbData.payload.hostStats[hostname()]
            : undefined;
    const hostConfig = () => state.hostConfigMap[hostname()];

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
                    <A
                        href="/hosts"
                        aria-label={`Back to hosts list — currently viewing ${hostname()}`}
                        class="group flex items-center gap-3 mb-6 flex-wrap hover:opacity-80 transition-opacity cursor-pointer"
                    >
                        <span class="shrink-0 text-[#616161] dark:text-[#9d9d9d] group-hover:-translate-x-0.5 transition-transform">
                            ←
                        </span>
                        <h2 class="section-title mb-0">{hostname()}</h2>
                        <HostStatusBadge status={status()} />
                        <OperationFailureBadge
                            failure={state.operationFailures[hostname()]}
                        />
                    </A>

                    <NotificationSection
                        hostname={hostname()}
                        status={status()}
                        operationFailure={state.operationFailures[hostname()]}
                    />

                    <Show
                        when={
                            state.dbData.status === 'available' ||
                            hostConfig() !== undefined
                        }
                    >
                        <HostInfoSection
                            hostStats={hostStats()}
                            hostConfig={hostConfig()}
                            isOnline={status() === 'online'}
                        />
                    </Show>

                    <HostLeasesSection hostname={hostname()} />
                </Match>
            </Switch>
        </AppLayout>
    );
}) satisfies AnyComponent;
