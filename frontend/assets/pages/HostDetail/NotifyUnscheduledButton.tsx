import type { AnyComponent } from '../../helpers/utils';
import { Bell, BellOff, LoaderCircle } from 'lucide-solid';
import { createSignal, Match, onMount, Show, Switch } from 'solid-js';
import {
    checkHostUnscheduledSubscription,
    subscribeToHostUnscheduled,
    unsubscribeFromHostUnscheduled,
} from '../../helpers/pushSubscription';

const notifyUnscheduledDescription =
    'Get a push notification when this host starts up or shuts down without being triggered by ShutHost.';

export const NotifyUnscheduledButton = ((props: { hostname: string }) => {
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
        <div class="flex flex-col items-center gap-1">
            <button
                type="button"
                class={`btn btn-height sm:px-5 sm:py-3 sm:text-base ${isSubscribed() ? 'btn-red' : 'btn-green'}`}
                disabled={isChecking() || loading()}
                onClick={handleClick}
                aria-describedby="notify-unscheduled-description"
                title={notifyUnscheduledDescription}
            >
                <Switch>
                    <Match when={isChecking() || loading()}>
                        <LoaderCircle
                            size={16}
                            class="animate-spin"
                            aria-hidden="true"
                        />
                    </Match>
                    <Match when={isSubscribed()}>
                        <BellOff size={20} aria-hidden="true" />
                    </Match>
                    <Match when={!isSubscribed()}>
                        <Bell size={20} aria-hidden="true" />
                    </Match>
                </Switch>
                <span class="flex flex-col text-center leading-tight">
                    <span>
                        {isSubscribed() ? 'Unsubscribe from' : 'Subscribe to'}
                    </span>
                    <span>unscheduled events</span>
                </span>
            </button>
            <p
                id="notify-unscheduled-description"
                class="text-xs text-[#616161] dark:text-[#9d9d9d] text-center max-w-[20rem] touch-description"
            >
                {notifyUnscheduledDescription}
            </p>
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
}) satisfies AnyComponent;
