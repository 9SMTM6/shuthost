import type { Component } from 'solid-js';
import { Show, createSignal, onMount, onCleanup } from 'solid-js';
import { useSearchParams } from '@solidjs/router';
import { serverData } from '../serverData';

// Login error message map — mirrors the LOGIN_ERROR_* constants defined in the Rust coordinator.
const ERROR_MESSAGES: Record<string, { title: string; body: string }> = {
    insecure: {
        title: 'Insecure connection',
        body: 'Your connection is not detected as HTTPS. Authentication cookies are set with Secure=true and will be ignored by browsers over plain HTTP. Serve the app over TLS or configure a reverse proxy that sets X-Forwarded-Proto: https.',
    },
    token: {
        title: 'Invalid token',
        body: 'The access token you entered is incorrect. Please try again.',
    },
    unknown: {
        title: 'Login error',
        body: 'An unknown error occurred during login. Please try again.',
    },
    oidc: {
        title: 'SSO error',
        body: 'An error occurred during SSO authentication. Please try again.',
    },
    session_expired: {
        title: 'Session expired',
        body: 'Your session has expired. Please log in again.',
    },
};

const TokenLoginForm = (() => {
    const [showToken, setShowToken] = createSignal(false);

    const eyeIcon = (
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="18" height="18" aria-hidden="true">
            <path d="M12 5c-7 0-10 7-10 7s3 7 10 7 10-7 10-7-3-7-10-7zm0 12a5 5 0 1 1 0-10 5 5 0 0 1 0 10z" />
            <circle cx="12" cy="12" r="3" />
        </svg>
    );

    const eyeOffIcon = (
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" width="18" height="18" aria-hidden="true">
            <path d="M2 4l2-2 18 18-2 2-3.4-3.4C14.7 19.5 13.4 20 12 20 5 20 2 13 2 13s1.1-2.4 3.2-4.7L2 4zm6.2 6.2L9.7 11.7A3 3 0 0 0 12 15a3 3 0 0 0 2.8-4l1.5 1.5A5 5 0 0 1 12 17a5 5 0 0 1-3.8-6.8zM12 5c2 0 3.7.6 5 .1l2.1 2.1C20.3 8.4 22 11 22 11s-3 7-10 7c-1.4 0-2.7-.5-3.8-1.2l1.5-1.5c.7.4 1.5.7 2.3.7a5 5 0 0 0 5-5c0-.8-.3-1.6-.7-2.3l1.5-1.5C18.5 6.7 15.6 5 12 5z" />
        </svg>
    );

    return (
        <form method="post" action="/login" class="space-y-4">
            <div>
                <label class="block text-sm mb-1" for="token">Access Token</label>
                <div class="flex items-stretch gap-2 max-w-sm mx-auto">
                    <input
                        id="token"
                        name="token"
                        type={showToken() ? 'text' : 'password'}
                        required
                        autofocus
                        autocomplete="current-password"
                        spellcheck={false}
                        class="w-full rounded border border-[#e5e5e5] dark:border-[#3e3e42] bg-white dark:bg-[#252526] text-black dark:text-[#cccccc] px-3 py-2"
                    />
                    <button
                        type="button"
                        class="px-3 py-2 rounded border border-[#e5e5e5] dark:border-[#3e3e42] text-[#616161] dark:text-[#a0a0a0] hover:bg-[#f0f0f0] dark:hover:bg-[#252526] leading-none"
                        aria-label={showToken() ? 'Hide token' : 'Show token'}
                        aria-pressed={showToken()}
                        title={showToken() ? 'Hide token' : 'Show token'}
                        onClick={() => setShowToken(v => !v)}
                    >
                        {showToken() ? eyeOffIcon : eyeIcon}
                    </button>
                </div>
            </div>
            <button
                type="submit"
                class="btn btn-green w-full text-xs sm:text-sm px-3 py-2 mt-3 border border-transparent"
            >
                Login
            </button>
        </form>
    );
}) satisfies Component<any>;

const OidcLoginForm = (() => (
    <a
        href="/oidc/login"
        class="w-full text-xs sm:text-sm px-3 py-2 mt-3 rounded bg-[#005fb8] text-white hover:bg-[#004a94] dark:bg-[#0078d4] dark:hover:bg-[#006cbe] border border-transparent text-center block"
    >
        Login with SSO
    </a>
)) satisfies Component<any>;

export const LoginPage = (() => {
    const [searchParams] = useSearchParams();

    onMount(() => {
        document.body.classList.add('flex', 'flex-col', 'login-page', 'disable-nav');
        document.head.title = 'Login - ShutHost Coordinator';
    });
    onCleanup(() => {
        document.body.classList.remove('flex', 'flex-col', 'login-page', 'disable-nav');
        document.head.title = 'ShutHost Coordinator';
    });

    const errorKey = () => {
        const v = searchParams['error'];
        if (Array.isArray(v)) return v[0] ?? null;
        return v ?? null;
    };
    const errorInfo = () => {
        const key = errorKey();
        if (!key) return null;
        return ERROR_MESSAGES[key] ?? ERROR_MESSAGES['unknown'];
    };

    return (
        <div class="flex flex-col items-center justify-center flex-1 p-4 gap-4 w-full">
            <Show when={errorInfo()}>
                {info => (
                    <div class="max-w-md w-full alert alert-warning" role="alert">
                        <div class="alert-title">{info().title}</div>
                        <p>{info().body}</p>
                    </div>
                )}
            </Show>

            <section class="section-container max-w-md w-full" aria-labelledby="login-title">
                <header class="py-3 text-center">
                    <h1 id="login-title" class="text-lg font-semibold">Sign in</h1>
                </header>
                <div class="p-4">
                    <Show when={serverData.authMode === 'token'}>
                        <TokenLoginForm />
                    </Show>
                    <Show when={serverData.authMode === 'oidc'}>
                        <OidcLoginForm />
                    </Show>
                </div>
            </section>
        </div>
    );
}) satisfies Component<any>;
