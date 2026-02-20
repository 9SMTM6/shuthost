import { configs, assignedPortForConfig, getPidsListeningOnPort, validatePidIsExpected, killPidGracefully } from './tests/backend-utils';

export default async function globalTeardown() {
  console.log('Playwright global teardown: stopping backend processes');
  const backendBin = process.env['COVERAGE'] ? '../target/debug/shuthost_coordinator' : '../target/release/shuthost_coordinator';

  for (const configPath of Object.values(configs)) {
    const port = assignedPortForConfig(configPath);
    const pids = getPidsListeningOnPort(port);
    for (const pid of pids) {
      if (validatePidIsExpected(pid, backendBin)) {
        console.log(`terminating coordinator pid ${pid} on port ${port}`);
        killPidGracefully(pid);
      } else {
        console.log(`leaving pid ${pid} on port ${port} (not coordinator)`);
      }
    }
  }

  // also clean up demo mode backend if it was started
  const demoPort = assignedPortForConfig(undefined);
  if (!Object.values(configs).some((p) => assignedPortForConfig(p) === demoPort)) {
    const pids = getPidsListeningOnPort(demoPort);
    for (const pid of pids) {
      if (validatePidIsExpected(pid, backendBin)) {
        console.log(`terminating demo coordinator pid ${pid} on port ${demoPort}`);
        killPidGracefully(pid);
      } else {
        console.log(`leaving pid ${pid} on port ${demoPort} (not coordinator)`);
      }
    }
  }
}
