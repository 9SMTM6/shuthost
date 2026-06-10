import { Title } from '@solidjs/meta';
import { Footer } from '../components/Footer';
import { SimpleHeader } from '../components/Header';
import { JsErrorBox } from '../components/JsErrorBox';
import type { AnyComponent } from '../helpers/utils/solid';

/**
 * Static HTML shell served to all routes before JS loads.
 * This is mostly to be able to have the JS error box available as soon as possible,
 * while having it properly integrated in the layout
 *
 * Renders only the page chrome (header, footer, JS error box) with an empty
 * main area. JS calls render() after load, clears this, and mounts the real
 * component tree for the correct route.
 */
export const PrerenderedShell = (() => (
    <>
        <Title>ShutHost Coordinator</Title>
        <SimpleHeader />
        <main id="main-content" class="main flex flex-col" tabindex="-1">
            <JsErrorBox />
        </main>
        <Footer />
    </>
)) satisfies AnyComponent;
