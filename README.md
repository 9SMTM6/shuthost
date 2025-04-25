# ShutHost [WIP]

A neat little helper that manages the standby state of unix hosts with Wake-On-Lan configured, with Web-GUI.
Since WOL doesn't define a way to shut down (maybe simply for security reasons), and unix doensn't either, this adds agents as services to these hosts, that can issue shutdown commands (signed with HMAC and protected against replay attacks with timestamps) and provide status.

The GUI doesn't provide authorization, you'll have to do that yourself (e.g. NGINX Proxy Manager).

Note that LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

## Known issues:

* UI still gets stuck on load sometimes. IDK why. Need to add more timeouts
* docker image creation is untested and probably doesnt work.
* untested on mac
* windows support currently not planned, due to large differences

## Planned features:

* I should check online status with a worker on the server, and provide updates per websockets. Currently naive implementation.
* documentation:
    * installation of client
    * settings required for wake on lan to work (as far as they are defined, show example of host that only WOLs from sleep)
    * reachability of hosts to wol command required, seems stricter than IP? (validate)
* endpoint on server that allows agents to register themselfes. Unclear how to deal with authorisation:
    * server secret?
    * also page is supposed to be behind reverse proxy, which would have to be dealt with on top...
* autobuild image in actions and push them to ghcr.io
* windows support currently not planned, due to large differences
* BSD support only planned if someone tests it themselfes (after every release? TBD)
* uninstall
