import { Title } from '@solidjs/meta';
import { A } from '@solidjs/router';
import type { AnyComponent } from '../helpers/utils/solid';
import { Footer } from '../sharedComponents/Footer';
import { SimpleHeader } from '../sharedComponents/Header';

export const NotFoundPage = (() => {
    return (
        <>
            <Title>404 – Page Not Found | ShutHost Coordinator</Title>
            <SimpleHeader />
            <main
                id="main-content"
                class="main px-4 sm:px-6 lg:px-8 max-w-7xl mx-auto w-full"
                tabindex="-1"
            >
                <section class="py-16 sm:py-24 flex flex-col items-center text-center gap-4">
                    <h1 class="text-6xl font-bold text-gray-800 dark:text-gray-100">
                        404
                    </h1>
                    <p class="text-xl text-gray-600 dark:text-gray-400">
                        Page not found
                    </p>
                    <p class="text-gray-500 dark:text-gray-500">
                        The page you're looking for doesn't exist.
                    </p>
                    <A
                        href="/"
                        class="mt-4 rounded bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-700 dark:bg-blue-500 dark:hover:bg-blue-600"
                    >
                        Go to Home
                    </A>
                </section>
            </main>
            <Footer />
        </>
    );
}) satisfies AnyComponent;
