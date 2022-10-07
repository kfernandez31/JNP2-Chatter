# Chatter - a command-line chat with rooms

This app is a simple command-line chat written fully in Rust.
It allows multiple users to communicate with each other through multiple rooms.

Once a user opens the chat, they can:
- connect as an existing user,
- register (and connect) as a new user.
Then, they can:
- connect to an existing room,
- create (and connect to) a new room.

Joining and leaving a room results in a notification of the event being sent to remaining users.

Chat history for each room is stored in hidden a directory created by the app under the home directory.

 - Communication architecture - 

Chatter uses a two-protocol communication style:
- HTTP for server control, 
- TCP WebSocket for asynchronous server responses. 
Such architecture provides convienient separation of control and broadcasting data flow, combining best of both worlds - HTTP transactions and error notifications along with WS agility. 
Data flow:
 - HTTP: CLIENT -> SERVER, transaction result handling in app protocol layer
 - WS  : SERVER -> CLIENT, no transaction result handling in app protocol layer (only TCP handshake) 

 The client uses HTTP to send messages, joining/leaving rooms, registration, heartbeat service.                    
 The server uses WebSockets to transfer messages to listening clients with room distingishing.
