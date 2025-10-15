# mctrlrs
Minecraft Controller written in Rust (mc ctrl rs).

## What is this project about?
This is a Minecraft server sidecar, a web server that can be run alongside the
Minecraft server and can communicate to it using the RCON protocol.

It has some basic functionality to display the number of online players with a
list of them and also show show the tick stats.

However, the main goal of the project is to switch between multiple different
worlds. Me and my friends are playing Minecraft for a while and we periodically
create a new world when big enough update drops and changes a lot in the world
generation routines. It is fun though to come back to our old worlds from time
to time and explore what we did when we get nostalgic.

This is exactly what this tool is for - it allows switch between multiple world
directories. It will first shutdown the server, change the configuration file
to point to a different folder and then restart it back up.

This project comes with two modes: a server and a management tool.

A server can be started by using the `mctrlrs server` command and is
essentially a sidecar with the web interface.

A management tool is a CLI that allows to manage users for web interface
access, including enrolling new users with registration link that can be sent
to somebody to set their own password, reset their password or remove a user.
This functionality can be accessed with `mctrlrs manage user` subcommand.

It also allows to manage worlds: listing available worlds and switching between
them, similarly to what the web interface would do. This can be done via the
`mctrlrs manage world` subcommand.
