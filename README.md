# What is this?

This is a repository of my final project for the [Languages and tools for programming II (pol. Języki i Narzędzia Programowania II / JNP2)](https://usosweb.mimuw.edu.pl/kontroler.php?_action=katalog2%2Fprzedmioty%2FpokazPrzedmiot&prz_kod=1000-224bJNP2&lang=en) course offered by the Faculty of Mathematics, Informatics and Mechanics at the University of Warsaw in the 2021/2022 summer semester.

The Rust group had its first edition at the time of writing this project. It was a successful experiment carried out by experienced rustaceans:
- [Piotr Wojtczak](https://github.com/StarostaGit),
- [Andrzej Głuszak](https://github.com/agluszak).

# Authors

Myself and my friend [Jan Zembowicz](https://github.com/JWZ1996).

# Brief description

The project I decided to write was a simple command-line multi-room chat application. It uses both HTTP and WebSocket protocols.

# Full description 

You can find a more in-depth description [**here**](https://github.com/kfernandez31/JNP2-Rust-Chatter/blob/main/description.md).


## Running the app
You can run a single instance of the server like this:
```
cargo run --bin server <address>
```
where the `<address>` (ex. 0.0.0.0) is optional and replaced by a default value if not provided.
Multiple clients can be run like so:
```
cargo run --bin server
```
