# ShutHost [WIP]

A neat little helper that manages the standby state of unix hosts with Wake-On-Lan configured, with Web-GUI.
Since WOL doesn't define a way to shut down (maybe simply for security reasons), and unix doensn't either, this adds agents as services to these hosts, that can issue shutdown commands (signed with HMAC and protected against replay attacks with timestamps) and provide status.

The GUI doesn't provide authorization, you'll have to do that yourself (e.g. NGINX Proxy Manager).

Note that LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

## Known issues:

* UI still gets stuck on load sometimes. IDK precisely why. Need to add timeouts when communicating with other hosts
* docker doesnt seem like it'll happen:
    * according to what I've seen, podman (and likely docker) on macos wont be able to transfer WOL packages to the host LAN, and docker on WSL would also need additional config, if at all possible.
    * thus I'm packaging things into a single binary - with embedded agents and static files - instead 
* only tested setups currently:
    * agent on unraid and linux systemd
    * controller on macos apple silicon
* windows support currently not planned, due to large differences

## Planned features:

* documentation:
    * installation of client
    * settings required for wake on lan to work (as far as they are defined, show example of host that only WOLs from sleep)
    * need for static IP
    * reachability of hosts to wol command required, seems stricter than IP? (validate)
    * binary exposes server on localhost only, reach it from docker (bind localhost (NOT `0.0.0.0`) and in docker `http://host.containers.internal:<port>`)
* endpoint on server that allows agents to register themselfes. Unclear how to deal with authorisation:
    * server secret?
    * also page is supposed to be behind reverse proxy, which would have to be dealt with on top...
    * NEW PLAN: Do a broadcast from agent with its information:
        * https://chatgpt.com/share/6814d08c-07a8-8008-8c12-2a0b1f03fb59
        * this tests that the hosts can probably reach each other (IDK if thats always guaranteed to work both ways)
        * this avoids normal security, so no exception required on reverse proxy
* windows support currently not planned, due to large differences
* BSD support not really planned (hard to get  working toolchain to macos)
* uninstalls
