# ShutHost [WIP]

A neat little helper that manages the standby state of unix hosts with Wake-On-Lan configured, with Web-GUI.
Since WOL doesn't define a way to shut down (maybe simply for security reasons), and unix doensn't either, this adds agents as services to these hosts, that can issue shutdown commands (signed with HMAC and protected against replay attacks with timestamps) and provide status.

The GUI doesn't provide authorization, you'll have to do that yourself (e.g. NGINX Proxy Manager).

Note that LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

## Known issues:

* UI still gets stuck on load sometimes. IDK why. Need to add more timeouts
* docker has issues (see Containerfile too):
    * according to what I've seen, podman (and likely docker) on macos wont be able to transfer WOL packages to the host LAN, and docker on WSL would also need additional config, if at all possible.
    * so using docker kind of falls out of the question.
    * only on linux hosts should it work with some additional config - e.g. `--cap-add=NET_RAW`
    * move to single binary - with embedded agents for portability - instead, and show how to expose on localhost only, and reach it from docker (bind localhost (NOT `0.0.0.0`) and in docker `http://host.containers.internal:<port>`)
    * since we already have them for the agent, add a service files for unix hosts to set up the server
* untested on mac
* windows support currently not planned, due to large differences

## Planned features:

* documentation:
    * installation of client
    * settings required for wake on lan to work (as far as they are defined, show example of host that only WOLs from sleep)
    * reachability of hosts to wol command required, seems stricter than IP? (validate)
* endpoint on server that allows agents to register themselfes. Unclear how to deal with authorisation:
    * server secret?
    * also page is supposed to be behind reverse proxy, which would have to be dealt with on top...
* autobuild image in actions and push them to ghcr.io
* macos amd64 is probably not too hard, just needs doing.
* windows support currently not planned, due to large differences
* BSD support only planned if someone tests it themselfes (after every release? TBD)
* uninstall
