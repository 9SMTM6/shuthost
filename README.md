# ShutHost [WIP]

A neat little helper that manages the standby state of unix hosts with Wake-On-Lan configured, with Web-GUI.
Since WOL doesn't define a way to shut down (maybe simply for security reasons), and unix doensn't either, this adds node_agents as services to these hosts, that can issue shutdown commands (signed with HMAC and protected against replay attacks with timestamps) and provide status.

The GUI doesn't provide authorization, you'll have to do that yourself (e.g. NGINX Proxy Manager).

Note that LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

## Known issues:

* requires static IPs on the hosts to control
* requires that WOL works:
    * motherboard can do it (sometimes its not supported, or only supported in higher power levels, e.g. RAM based sleep) 
        * AND IS CONFIGURED accordingly
    * OS will also need to be configured accordingly
    * broadcasts need to be enabled on the network, and the hosts must be able to reach each other (TODO test for that)
* UI still gets stuck on load sometimes. IDK precisely why. Need to add timeouts when communicating with other hosts
* docker is problematic:
    * its currently untested
    * according to what I've seen, podman (and likely docker) on macos wont be able to transfer WOL packages to the host LAN, and docker on WSL would also need additional config, if at all possible.
    * thus on these targets you need to use a VM, or simply use the binary - its still just a single file to start.
* only tested setups currently:
    * node_agent on unraid and linux systemd
    * coordinator on macos apple silicon
* windows support currently not planned, due to large differences

## Planned features:

* documentation:
    * installation of client
    * settings required for wake on lan to work (as far as they are defined, show example of host that only WOLs from sleep)
    * need for static IP
    * reachability of hosts to wol command required, seems stricter than IP? (validate)
    * binary exposes server on localhost only, reach it from docker (bind localhost (NOT `0.0.0.0`) and in docker `http://host.containers.internal:<port>`)
* endpoint on server that allows node_agents to register themselfes. Unclear how to deal with authorisation:
    * server secret?
    * also page is supposed to be behind reverse proxy, which would have to be dealt with on top...
    * NEW PLAN: Do a broadcast from node_agent with its information:
        * https://chatgpt.com/share/6814d08c-07a8-8008-8c12-2a0b1f03fb59
        * this tests that the hosts can probably reach each other (IDK if thats always guaranteed to work both ways)
        * this avoids normal security, so no exception required on reverse proxy
* windows support currently not planned, due to large differences
* BSD support might happen, 
    * requires using cross though, which I wont do locally. This also means refactoring the github pipeline
    * to be able to build it locally I'd have to introduce features
* uninstalls

<!-- 
    // TODO: add endpoint to get leases, and enable/disable buttons based on lease status.
    // rework the way frontend updates work
    // poll hosts in the backend in paralell
    // poll hosts in the backend with variable polling frequency (whether there is a frontend active or not)
    // switch to TS or figure out some other way to better handle frontend logic.
    // push status updates in object form instead of with arrays
    // add a "lease" column to the table, showing lease status.
    // Add separate table to show configured clients, with a reset button to fix broken leases.
    // Then add rework wording/UI of GUI leases to be understandable without understanding leases (if someone doesnt need them).
    // Then add a bunch of documentation to explain:
    // - how to configure WOL and gotchas
    // - shuthost architecture
    // - how leases work
    // Then rework UI to be css-grid based, and test on mobile.
    // then consider different global layout (with tabs?) -->
