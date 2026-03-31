import { Title } from '@solidjs/meta';
import type { Component } from 'solid-js';
import { Footer } from '../components/Footer';
import { SimpleHeader } from '../components/Header';
import { JsErrorBox } from '../components/JsErrorBox';

/**
 * Static HTML shell served to all routes before JS loads.
 *
 * Renders only the page chrome (header, footer, JS error box) with an empty
 * main area. JS calls render() after load, clears this, and mounts the real
 * component tree for the correct route.
 *
 * buildData is injected into the singleton by prerender.tsx before
 * renderToString() runs, so Header and Footer receive real asset URLs.
 */
export const PrerenderedShell: Component = () => (
    <>
        <Title>ShutHost Coordinator</Title>
        <SimpleHeader />
        <main id="main-content" class="main flex flex-col" tabindex="-1">
            <JsErrorBox />
        </main>
        <Footer />
    </>
);
