import { Title } from '@solidjs/meta';
import { useSearchParams } from '@solidjs/router';
import { Eye, EyeOff, LogIn } from 'lucide-solid';
import { createSignal, Show } from 'solid-js';
import { Footer } from '../components/Footer';
import { SimpleHeader } from '../components/Header';
import { JsErrorBox } from '../components/JsErrorBox';
import type { AnyComponent } from '../helpers/component';
import { serverData } from '../helpers/serverData';

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

    const eyeIcon = <Eye size={18} aria-hidden="true" />;
    const eyeOffIcon = <EyeOff size={18} aria-hidden="true" />;

    return (
        <form method="post" action="/login" class="space-y-4">
            <div>
                <label class="block text-sm mb-1" for="token">
                    Access Token
                </label>
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
                        onClick={() => setShowToken((v) => !v)}
                    >
                        {showToken() ? eyeOffIcon : eyeIcon}
                    </button>
                </div>
            </div>
            <button
                type="submit"
                class="btn btn-green w-full text-xs sm:text-sm px-3 py-2 mt-3 border border-transparent"
            >
                <LogIn size={14} aria-hidden="true" />
                Login
            </button>
        </form>
    );
}) satisfies AnyComponent;

const OidcLoginForm = (() => (
    <a
        href="/oidc/login"
        rel="external"
        class="inline-flex items-center justify-center gap-1 w-full text-xs sm:text-sm px-3 py-2 mt-3 rounded bg-[#005fb8] text-white hover:bg-[#004a94] dark:bg-[#0078d4] dark:hover:bg-[#006cbe] border border-transparent"
    >
        <LogIn size={14} aria-hidden="true" />
        Login with SSO
    </a>
)) satisfies AnyComponent;

export const LoginPage = (() => {
    const [searchParams] = useSearchParams();

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
        <>
            <Title>Login - ShutHost Coordinator</Title>
            <SimpleHeader />
            <main id="main-content" class="main flex flex-col" tabindex="-1">
                <JsErrorBox />
                <div class="flex flex-col items-center justify-center flex-1 p-4 gap-4 w-full">
                    <Show when={errorInfo()}>
                        {(info) => (
                            <div
                                class="max-w-md w-full alert alert-warning"
                                role="alert"
                            >
                                <div class="alert-title">{info().title}</div>
                                <p>{info().body}</p>
                            </div>
                        )}
                    </Show>

                    <section
                        class="section-container max-w-md w-full"
                        aria-labelledby="login-title"
                    >
                        <header class="py-3 text-center">
                            <h1 id="login-title" class="text-lg font-semibold">
                                Sign in
                            </h1>
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
            </main>
            <Footer />
        </>
    );
}) satisfies AnyComponent;
