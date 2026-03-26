import type { Component } from 'solid-js';
import { For } from 'solid-js';
import { Title } from '@solidjs/meta';

export type Author = {
    name: string;
    email?: string | null;
};

export type DependencyEntry = {
    name: string;
    version: string;
    ecosystem: 'Rust' | 'Npm';
    license: string;
    license_html: string;
    authors: Author[];
    repository?: string | null;
};

export type AboutPageProps = {
    description: string;
    repository: string;
    version: string;
    entries: DependencyEntry[];
    licenses: Record<string, string>;
};

export const AboutPage = ((props: AboutPageProps) => (
    <>
    <Title>About &amp; Licenses - ShutHost Coordinator</Title>
    <main
        id="main-content"
        class="main px-4 sm:px-6 lg:px-8 max-w-7xl mx-auto w-full"
        tabindex="-1"
    >
        <section class="py-4 sm:py-6" aria-labelledby="about-page-title">

            {/* About Section */}
            <section class="section-container" aria-labelledby="about-shuthost-title">
                <h1 id="about-page-title" class="section-title px-4 pt-4 text-xl sm:text-2xl">About ShutHost</h1>
                <div class="m-4 text-sm sm:text-base text-[#616161] dark:text-[#9d9d9d] space-y-3">
                    <p>{props.description}</p>
                    <p>
                        <a href={props.repository} class="link font-medium">
                            ShutHost v{props.version}
                        </a>{' '}
                        is licensed under{' '}
                        <a href="#license-GPL-2.0-only" class="link font-medium">GPL 2.0-only</a>.
                    </p>
                </div>
            </section>

            {/* Dependencies Section */}
            <section class="section-container mt-4" aria-labelledby="dependencies-title">
                <h2 id="dependencies-title" class="section-title px-4 pt-4 text-lg sm:text-xl">
                    Open Source Dependencies
                </h2>
                <p class="mx-4 mb-4 text-sm sm:text-base text-[#616161] dark:text-[#9d9d9d]">
                    We are grateful to the open source community and the authors of the following
                    libraries that make this project possible.
                </p>

                <div class="table-wrapper" tabindex="0">
                    <table class="info-table w-full text-sm" aria-describedby="dependencies-title">
                        <thead class="bg-[#f3f3f3] dark:bg-[#252526]">
                            <tr>
                                <th scope="col">Name</th>
                                <th scope="col">Version</th>
                                <th scope="col">License</th>
                                <th scope="col">Authors/Publisher</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For each={props.entries}>
                                {(entry) => (
                                    <tr>
                                        <td>
                                            {entry.repository ? (
                                                <a href={entry.repository} class="link font-medium">
                                                    {entry.name}
                                                </a>
                                            ) : (
                                                entry.name
                                            )}
                                        </td>
                                        <td>{entry.version}</td>
                                        <td innerHTML={entry.license_html} />
                                        <td>
                                            <For each={entry.authors}>
                                                {(author, i) => (
                                                    <>
                                                        {author.email ? (
                                                            <a href={`mailto:${author.email}`} class="link">
                                                                {author.name}
                                                            </a>
                                                        ) : (
                                                            author.name
                                                        )}
                                                        {i() < entry.authors.length - 1 ? ', ' : ''}
                                                    </>
                                                )}
                                            </For>
                                        </td>
                                    </tr>
                                )}
                            </For>
                        </tbody>
                    </table>
                </div>
            </section>

            {/* Licenses Section */}
            <section class="section-container mt-4" aria-labelledby="licenses-title">
                <h2 id="licenses-title" class="section-title m-4 sm:m-6 text-lg sm:text-xl">License Texts</h2>
                <div class="space-y-6">
                    <For each={Object.entries(props.licenses)}>
                        {([id, text]) => (
                            <div>
                                <h3 id={`license-${id}`} class="m-4 mb-2 license-title">{id}</h3>
                                <pre class="license-listing">{text}</pre>
                            </div>
                        )}
                    </For>
                </div>
            </section>

        </section>
    </main>
    </>
)) satisfies Component<any>;
