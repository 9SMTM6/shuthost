import { CircleDashed, Crosshair } from 'lucide-solid';
import { Show } from 'solid-js';
import { CopyButton } from '../../components/CopyButton';
import { buildData } from '../../helpers/buildData';
import { demoSubpath } from '../../helpers/demo';
import { formatRelativeTimestamp, safeExternalUrl } from '../../helpers/utils';
import type {
    HostConfig,
    HostHookConfig,
    HostStats,
} from '../../helpers/appStore';

const buildHostUpdateCommands = (
    hostStats: HostStats | undefined,
): { sh?: string; ps1?: string } | null => {
    if (hostStats == null || hostStats.agentVersion === buildData.version) {
        return null;
    }

    const baseUrl = window.location.origin + demoSubpath;
    const initSystem = hostStats.initSystem;
    const scriptPath = hostStats.scriptPath;
    const os = hostStats.operatingSystem;

    let shScriptPathArg = '';
    let ps1ScriptPathArg = '';
    if (scriptPath != null) {
        if (initSystem === 'self-extracting-shell') {
            shScriptPathArg = ` --script-path '${scriptPath}'`;
        } else if (initSystem === 'self-extracting-pwsh') {
            ps1ScriptPathArg = ` -ScriptPath '${scriptPath}'`;
        } else {
            console.error(
                `Host has scriptPath '${scriptPath}' but init system '${
                    initSystem ?? 'unknown'
                }' is not a self-extracting type`,
            );
        }
    }

    const shCmd = `curl -fsSL ${baseUrl}/download/host_agent_installer.sh | sh -s ${baseUrl} --update${shScriptPathArg}`;
    const ps1Cmd = `curl.exe -sSLO '${baseUrl}/download/host_agent_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\host_agent_installer.ps1 ${baseUrl} -Update${ps1ScriptPathArg}`;

    if (initSystem === 'self-extracting-pwsh' || os === 'windows') {
        return { ps1: ps1Cmd };
    } else if (os == null) {
        return { sh: shCmd, ps1: ps1Cmd };
    }

    return { sh: shCmd };
};

const HostHookCard = (props: {
    hookName: 'preStartup' | 'postShutdown';
    hook: HostHookConfig;
}) => {
    const formatHookActionLabel = (hook: NonNullable<typeof props.hook>) =>
        hook.action.type === 'exec' ? 'Exec' : `HTTP ${hook.action.method}`;

    const formatHookActionValue = (hook: NonNullable<typeof props.hook>) =>
        hook.action.type === 'exec' ? hook.action.program : hook.action.url;

    const formatHookTiming = (hook: NonNullable<typeof props.hook>) => {
        if (hook.delaySecs === 0) {
            return `Timeout: ${hook.timeoutSecs}s`;
        }
        return `Delay: ${hook.delaySecs}s · Timeout: ${hook.timeoutSecs}s`;
    };

    return (
        <div class="rounded border border-[#e5e5e5] dark:border-[#3e3e42] p-3 bg-[#fafafa] dark:bg-[#1f1f23]">
            <div class="flex flex-wrap items-baseline gap-2">
                <p class="text-sm font-semibold text-black dark:text-[#cccccc]">
                    {
                        {
                            preStartup: 'Before startup',
                            postShutdown: 'After shutdown',
                        }[props.hookName]
                    }
                </p>
                <p class="text-xs uppercase tracking-[0.08em] text-[#7a7a7a] dark:text-[#8f8f8f]">
                    {formatHookActionLabel(props.hook!)}
                </p>
            </div>
            <code class="inline-block text-sm text-[#333333] dark:text-[#dddddd] wrap-break-words bg-[#f4f4f4] dark:bg-[#2b2b2f] border border-[#e5e5e5] dark:border-[#3e3e42] rounded px-2 py-1 mt-1 whitespace-pre-wrap max-w-full">
                {formatHookActionValue(props.hook!)}
            </code>
            <p class="text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mt-2">
                {formatHookTiming(props.hook!)}
            </p>
        </div>
    );
};

export const HostInfoSection = (props: {
    hostStats: HostStats | undefined;
    hostConfig: HostConfig | undefined;
    isOnline: boolean;
}) => {
    const lastOnline = props.hostStats?.lastOnline ?? null;
    const agentVersion = props.hostStats
        ? (props.hostStats.agentVersion ?? `<= 1.7.1`)
        : 'unknown';
    const agentVersionNote =
        agentVersion === 'unknown'
            ? 'No record of this host connecting yet.'
            : agentVersion === '<= 1.7.1'
              ? 'Predates version reporting (added in 1.8.0).'
              : undefined;
    const isSelfExtracting =
        props.hostStats?.initSystem === 'self-extracting-shell' ||
        props.hostStats?.initSystem === 'self-extracting-pwsh';
    const initSystemNote = isSelfExtracting
        ? 'Runs as a standalone script, not a registered service. Autostart must be configured manually (e.g. via the init system).'
        : undefined;
    const enforceStateNote = props.hostConfig?.enforceState
        ? 'Periodically corrects power state to match current leases.'
        : 'Edge-triggered only — reacts to lease changes, no periodic correction.';

    const preStartupHook = props.hostConfig?.preStartup;
    const postShutdownHook = props.hostConfig?.postShutdown;

    const lastOnlinePrecise =
        !props.isOnline && lastOnline != null
            ? new Date(lastOnline).toLocaleString()
            : undefined;

    return (
        <section
            class="section-container p-4 mb-4"
            aria-labelledby="host-info-title"
        >
            <h3 id="host-info-title" class="section-title text-base">
                Information
            </h3>
            <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Agent version
                </dt>
                <dd
                    class="text-[#616161] dark:text-[#9d9d9d]"
                    title={agentVersionNote}
                >
                    {agentVersion}
                </dd>
                <Show when={agentVersionNote != null}>
                    <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                        {agentVersionNote}
                    </dd>
                </Show>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Init system
                </dt>
                <dd
                    class="text-[#616161] dark:text-[#9d9d9d]"
                    title={initSystemNote}
                >
                    {
                        {
                            systemd: 'systemd',
                            openrc: 'OpenRC',
                            'self-extracting-shell': 'Self-extracting (sh)',
                            'self-extracting-pwsh':
                                'Self-extracting (PowerShell)',
                            launchd: 'launchd',
                            unknown: 'Unknown',
                        }[props.hostStats?.initSystem ?? 'unknown']
                    }
                </dd>
                <Show when={initSystemNote != null}>
                    <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                        {initSystemNote}
                    </dd>
                </Show>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Operating system
                </dt>
                <dd class="text-[#616161] dark:text-[#9d9d9d]">
                    {
                        {
                            linux: 'Linux',
                            windows: 'Windows',
                            macos: 'macOS',
                            unknown: 'Unknown',
                        }[props.hostStats?.operatingSystem ?? 'unknown']
                    }
                </dd>
                <Show when={props.hostStats?.scriptPath}>
                    <dt class="font-medium text-black dark:text-[#cccccc]">
                        Install script
                    </dt>
                    <dd class="text-[#616161] dark:text-[#9d9d9d] break-all">
                        {props.hostStats?.scriptPath}
                    </dd>
                </Show>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Enforce state
                </dt>
                <dd
                    class="text-[#616161] dark:text-[#9d9d9d] inline-flex items-center gap-1"
                    title={enforceStateNote}
                >
                    <Show
                        when={props.hostConfig?.enforceState}
                        fallback={
                            <>
                                <CircleDashed
                                    size={14}
                                    class="text-[#9d9d9d]"
                                    aria-hidden="true"
                                />
                                No
                            </>
                        }
                    >
                        <Crosshair
                            size={14}
                            class="text-green-600 dark:text-[rgba(46,193,100,0.9)]"
                            aria-hidden="true"
                        />
                        Yes
                    </Show>
                </dd>
                <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                    {enforceStateNote}
                </dd>
                <dt class="font-medium text-black dark:text-[#cccccc]">
                    Last online
                </dt>
                <dd
                    class="text-[#616161] dark:text-[#9d9d9d]"
                    title={lastOnlinePrecise}
                >
                    {props.isOnline
                        ? 'Currently online'
                        : formatRelativeTimestamp(lastOnline)}
                </dd>
                <Show when={lastOnlinePrecise != null}>
                    <dd class="col-span-2 touch-description text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-1">
                        {lastOnlinePrecise}
                    </dd>
                </Show>
                <Show when={preStartupHook || postShutdownHook}>
                    <dt class="font-medium text-black dark:text-[#cccccc]">
                        Hooks
                    </dt>
                    <dd class="col-span-2 space-y-3">
                        <Show when={preStartupHook}>
                            {(hook) => (
                                <HostHookCard
                                    hookName="preStartup"
                                    hook={hook()}
                                />
                            )}
                        </Show>
                        <Show when={postShutdownHook}>
                            {(hook) => (
                                <HostHookCard
                                    hookName="postShutdown"
                                    hook={hook()}
                                />
                            )}
                        </Show>
                    </dd>
                </Show>
            </dl>
            <HostUpdateCommands hostStats={props.hostStats} />
        </section>
    );
};

const HostUpdateCommands = (props: { hostStats: HostStats | undefined }) => {
    const updateCmds = buildHostUpdateCommands(props.hostStats);

    return (
        <Show when={updateCmds != null}>
            <div class="mt-3 pt-3 border-t border-[#e5e5e5] dark:border-[#3e3e42]">
                <p class="text-sm font-medium text-black dark:text-[#cccccc] mb-1">
                    Update agent
                </p>
                <p class="text-xs text-[#7a7a7a] dark:text-[#8f8f8f] mb-2">
                    Experimental: agent updates — self-extracting variants
                    especially — are experimental; use with caution.
                    <a
                        class="text-xs text-blue-600 dark:text-blue-400 underline ml-1"
                        href={safeExternalUrl(
                            `${buildData.repository}/issues/new?title=${encodeURIComponent(
                                'Feedback: agent update (self-extracting)',
                            )}&body=${encodeURIComponent(
                                'Describe what happened and include steps to reproduce if possible.',
                            )}`,
                        )}
                        target="_blank"
                        rel="external noopener noreferrer"
                    >
                        Give feedback
                    </a>
                </p>
                <Show when={updateCmds?.sh != null}>
                    <Show when={updateCmds?.ps1 != null}>
                        <p class="text-xs font-semibold text-[#616161] dark:text-[#9d9d9d] mb-1">
                            Linux/macOS:
                        </p>
                    </Show>
                    <div class="code-container py-2">
                        <CopyButton
                            targetId="host-update-command-sh"
                            label="Copy update command (sh)"
                        />
                        <code id="host-update-command-sh" class="code-block">
                            {updateCmds?.sh}
                        </code>
                    </div>
                </Show>
                <Show when={updateCmds?.ps1 != null}>
                    <Show when={updateCmds?.sh != null}>
                        <p class="text-xs font-semibold text-[#616161] dark:text-[#9d9d9d] mb-1 mt-2">
                            Windows (PowerShell):
                        </p>
                    </Show>
                    <div class="code-container py-2">
                        <CopyButton
                            targetId="host-update-command-ps1"
                            label="Copy update command (PowerShell)"
                        />
                        <code id="host-update-command-ps1" class="code-block">
                            {updateCmds?.ps1}
                        </code>
                    </div>
                </Show>
            </div>
        </Show>
    );
};
