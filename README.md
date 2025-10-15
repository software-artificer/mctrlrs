# mctrlrs
Minecraft Controller written in Rust (mc ctrl rs).

## What is this project about?
This is a Minecraft server sidecar - a web server that can be run alongside the
Minecraft server and can communicate with it using the RCON protocol.

It has some basic functionality to display the number of online players, show a
list of them, and also show the tick stats.

However, the main goal of the project is to switch between multiple different
worlds. Me and my friends have been playing Minecraft for a while, and we
periodically create a new world when a big enough update drops and changes a lot
in the world generation routines. It is fun to come back to our old worlds from
time to time and explore what we did when we get nostalgic.

This is exactly what this tool is for - it allows switching between multiple
world directories. It will first shut down the server, change the configuration
file to point to a different folder, and then restart it back up.

This project comes with two modes: a server and a management tool.

A server can be started by using the `mctrlrs server` command and is
essentially a sidecar with a web interface.

The management tool is a CLI that allows you to manage users for web interface
access, including enrolling new users with a registration link that can be sent
to somebody to set their own password, reset their password, or remove a user.
This functionality can be accessed with the `mctrlrs manage user` subcommand.

It also allows you to manage worlds: list available worlds and switch between
them, similarly to what the web interface does. This can be done via the
`mctrlrs manage world` subcommand.

