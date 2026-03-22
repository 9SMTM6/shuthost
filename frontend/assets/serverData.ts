/// <reference lib="dom" />

export type ServerData = {
    configPath: string;
    showLogout: boolean;
    authWarning: boolean;
    isDemo: boolean;
    demoSubpath: string;
};

const el = document.getElementById('server-data');
if (!el?.textContent) throw new Error('Missing #server-data element');

export const serverData: ServerData = JSON.parse(el.textContent);
