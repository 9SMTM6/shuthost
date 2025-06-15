# ShutHost [WIP]

A neat little (well, at one time it was) helper that manages the standby state of unix hosts with Wake-On-Lan (WOL) configured, with Web-GUI.

Note that LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

## Architecture

See [Architecture Documentation](coordinator/assets/architecture.md)

## Requirements:

For the requirements for the agent, see [Requirements to install the agent](coordinator/assets/agent_install_requirements_gotchas.md).

The coordinator must be run on a host that can reach the hosts you want to manage.
This requires either running the coordinator as a binary on the host, or running it in a docker container with the host network mode enabled - this does not work with the default network mode that docker uses on Windows and MacOS. It will also not work on WSL. On these Hosts, you will have to run the coordinator as a binary, or install a Linux VM with bridged networking.

Windows is currently not supported, even with the binary.

The coordinator exposes its server on `127.0.0.1` only by default - so on localhost, ipv4, without remote access. This is for security reasons.
To access it from Docker, use the address `http://host.containers.internal:<port>` within the Docker container.
Other container solutions might require additional configuration to access the coordinator.
On Podman, adding 
```yaml
    extra_hosts:
      - "host.docker.internal:host-gateway"
```
to the container that wants to access the coordinator should work.
Alternatively you can set the address the coordinator binds to in the configuration file.

## Security

The WebUI is not secured, so you should run it behind a reverse proxy that provides TLS and authentication.

The host agents are secured with HMAC signatures and timestamps against replay attacks, so they can only be used by the coordinator that knows these secrets.

The client is secured in the same way, so the coordinator only accepts requests from registered clients.

To use the convenience scripts suggested by the WebUI, you will have to configure exceptions in the authorization of your reverse proxy, so that the requests from the host agents and clients are not blocked. The WebUI will show you the required exceptions, alongside convenience configs for Authelia, NGINX Proxy Manager and generic forward-auth in traefik.

## Known issues

* if the host misses the initial shutdown, a "full cycle" is required to send it again (release lease, take lease)
    * I'm considering regularely "syncing" states, maybe with explicit config on host (seems best) or coordinator-wide
* the coordinator looses state on update
    * since its not that much, and currently only acts on state changes, not problematic, but could be fixed with persistence with e.g. sqlite. Should be considered before adding status syncing
* docker is currently untested
* windows agent support currently not planned, due to large differences
* Accessing the coordinator from Docker requires proper configuration of the network mode and binding to `localhost`. Misconfiguration may lead to connectivity issues.

## Planned Features

* add architecture documentation to WebUI

## Potential Features

* I might add OIDC authorization, where I allow the required endpoints for all
    * I might consider enabling this by default, and/or showing some kind of error if the UI is shown without any authorization (detect by header presence)
* BSD support might happen, 
    * requires using cross though, which I wont do locally. This also means refactoring the github pipeline
    * to be able to build it locally I'd have to introduce features
* uninstalls
* endpoint on server that allows host_agents to register themselfes. Unclear how to deal with authorisation:
    * server secret?
    * also page is supposed to be behind reverse proxy, which would have to be dealt with on top...

<!-- TODO:
    // poll hosts in the backend with variable polling frequency (whether there is a frontend active or not, should be able to tell with ws_tx.receiver_count() - needs proper updates when the socket was closed, fails ATM)
    // -->
