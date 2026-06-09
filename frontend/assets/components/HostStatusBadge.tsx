import { Switch, Match } from 'solid-js';

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
