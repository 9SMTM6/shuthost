# ShutHost [WIP]

A neat little (well, at one time it was) helper that manages the standby state of unix hosts with Wake-On-Lan configured, with Web-GUI.
Since WOL doesn't define a way to shut down (maybe simply for security reasons), and unix doensn't either, this adds node_agents as services to these hosts, that can issue shutdown commands (signed with HMAC and protected against replay attacks with timestamps) and provide status.

The GUI doesn't provide authorization, you'll have to do that yourself (e.g. NGINX Proxy Manager).

Note that LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

## Known issues:

* requires static IPs on the hosts to control
* UI blocks somewhat when shutting down/starting host
* if the host misses the initial shutdown, a "full cycle" is required to send it again (release lease, take lease)
    * I'm considering regularely "syncing" states, maybe with explicit config on host (seems best) or coordinator-wide
* the coordinator looses state on update
    * since its not that much, and currently only acts on state changes, not problematic, but could be fixed with persistence with e.g. sqlite. Should be considered before adding status syncing
* docker is problematic:
    * its currently untested
    * according to what I've seen, podman (and likely docker) on macos wont be able to transfer WOL packages to the host LAN, and docker on WSL would also need additional config, if at all possible.
    * thus on these targets you need to use a VM with dedicated LAN IP, or simply use the binary - its still just a single file to start.
* windows agent support currently not planned, due to large differences

## Potential Features:
* BSD support might happen, 
    * requires using cross though, which I wont do locally. This also means refactoring the github pipeline
    * to be able to build it locally I'd have to introduce features
* uninstalls
* endpoint on server that allows node_agents to register themselfes. Unclear how to deal with authorisation:
    * server secret?
    * also page is supposed to be behind reverse proxy, which would have to be dealt with on top...


<!-- TODO:
    // poll hosts in the backend with variable polling frequency (whether there is a frontend active or not, should be able to tell with ws_tx.receiver_count() - needs proper updates when the socket was closed, fails ATM)
    // Then add rework wording/UI of GUI leases to be understandable without understanding leases (if someone doesnt need them).
    // Then add a bunch of documentation to explain:
    coordinator: * binary exposes server on localhost only, reach it from docker (bind localhost (NOT `0.0.0.0`) and in docker `http://host.containers.internal:<port>`)
    // - shuthost architecture
    // - how leases work
    // - authentification requirements (exclusions/oidc if I set it up)

    // fix issue with blocking UI
    // consider adding oidc with the correct endpoints exposed
    // consider using a sqlite database for persistence after all
    // -->
