use anyhow::Context;
use std::io::stdin;
use std::thread;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use hyper::StatusCode;
use reqwest::{Client as ReqwestClient, Response};
use tokio::io::AsyncBufReadExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tungstenite::protocol::Message as TungsteniteMsg;
use uuid::Uuid;
use JNP2_Rust_Chatter::common::{ReqData::*, *};

const CMD_EXIT: &str = "/exit"; // exits the entire app
const CMD_LOBBY: &str = "/lobby"; // goes back to the lobby

type WSStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn print_greeting() {
    println!("==========================");
    println!("=   Welcome to Chatter   =");
    println!("==========================");
    println!();
}

fn get_line(prompt: &str) -> Result<String, std::io::Error> {
    println!("{}", prompt);
    let mut line = String::new();
    match stdin().read_line(&mut line) {
        Ok(_) => return Ok(line.trim().to_string()),
        Err(err) => Err(err),
    }
}

fn get_nonempty_line(what: &str) -> String {
    let prompt = format!("Enter {}", what);
    let invalid = format!("Invalid {}. Please try again", what);
    loop {
        match get_line(&*prompt) {
            Ok(res) => {
                if res.is_empty() || res == SERVER_SIGNATURE {
                    println!("{}", invalid);
                } else {
                    break res;
                }
            }
            Err(err) => println!("Error: {}", err),
        }
    }
}

fn get_header<'a, T>(resp: &'a Response, header: &'a str) -> anyhow::Result<T>
where
    T: serde::Deserialize<'a>,
{
    let header_value = resp
        .headers()
        .get(header)
        .context(format!("No {}  was found in the request!", header))?;
    Ok(serde_json::from_slice(header_value.as_bytes())?)
}

async fn post<T>(
    reqwest_client: &ReqwestClient,
    endpoint: &str,
    body: &T,
) -> anyhow::Result<Response>
where
    T: ?Sized + serde::Serialize,
{
    let data = serde_json::to_string(&body)?;
    let resp = reqwest_client
        .post("http://".to_string() + &get_addr_str(Protocol::HTTP) + endpoint)
        .body(data)
        .send()
        .await?;
    Ok(resp)
}

async fn login(client_name: &str, reqwest_client: &ReqwestClient) -> anyhow::Result<Option<Uuid>> {
    let body = LoginData(ClientName(client_name.to_string()));
    let resp = post(reqwest_client, LOGIN_ENDPOINT, &body).await?;
    get_header(&resp, CLIENT_UUID_HEADER)
}

async fn register(
    client_name: &str,
    reqwest_client: &ReqwestClient,
    ws_stream: &mut WSStream,
) -> Uuid {
    let fail_msg = "Error in registration!";
    let body = RegistrationData(ClientName(client_name.to_string()));
    let body = serde_json::to_string(&body).unwrap();
    ws_stream
        .send(TungsteniteMsg::Text(body))
        .await
        .expect(fail_msg);
    login(client_name, reqwest_client)
        .await
        .expect(fail_msg)
        .expect(fail_msg)
}

async fn get_room(room_name: &str, reqwest_client: &ReqwestClient) -> anyhow::Result<Option<Uuid>> {
    let body = GetRoomData(RoomName(room_name.to_string()));
    let resp = post(reqwest_client, GET_ROOM_ENDPOINT, &body).await?;
    get_header(&resp, ROOM_UUID_HEADER)
}

async fn create_room(room_name: &str, reqwest_client: &ReqwestClient) -> anyhow::Result<Uuid> {
    let body = CreateRoomData(RoomName(room_name.to_string()));
    let resp = post(reqwest_client, CREATE_ROOM_ENDPOINT, &body).await?;
    get_header(&resp, ROOM_UUID_HEADER)
}

async fn join_room(
    client_uuid: Uuid,
    client_name: &str,
    room_uuid: Uuid,
    reqwest_client: &ReqwestClient,
) -> anyhow::Result<bool> {
    let body = JoinRoomData(
        ClientName(client_name.to_string()),
        ClientUuid(client_uuid),
        RoomUuid(room_uuid),
    );
    let resp = post(reqwest_client, JOIN_ROOM_ENDPOINT, &body).await?;
    get_header(&resp, SUCCESS_HEADER)
}

async fn send_msg(
    reqwest_client: &ReqwestClient,
    msg: ChatMessage,
    room_uuid: Uuid,
) -> anyhow::Result<Response> {
    let body = SendMsgData(msg, RoomUuid(room_uuid));
    post(reqwest_client, SEND_MSG_ENDPOINT, &body).await
}

async fn leave_room(
    reqwest_client: &ReqwestClient,
    client_uuid: Uuid,
    room_uuid: Uuid,
) -> anyhow::Result<Response> {
    let body = LeaveRoomData(RoomUuid(room_uuid), ClientUuid(client_uuid));
    post(reqwest_client, LEAVE_ROOM_ENDPOINT, &body).await
}

async fn exit_app(reqwest_client: &ReqwestClient, client_uuid: Uuid) -> anyhow::Result<Response> {
    let body = ExitAppData(ClientUuid(client_uuid));
    post(reqwest_client, EXIT_APP_ENDPOINT, &body).await
}

async fn register_or_login(
    reqwest_client: &ReqwestClient,
    ws_stream: &mut WSStream,
) -> (String, Uuid) {
    let client_name = get_nonempty_line("username");
    let client_uuid = match login(&client_name, reqwest_client)
        .await
        .expect("Error during login!")
    {
        Some(uuid) => {
            println!("Welcome back, {}", &client_name);
            uuid
        }
        None => {
            println!("Nice to meet you, {}", &client_name);
            register(&client_name, reqwest_client, ws_stream).await
        }
    };
    (client_name, client_uuid)
}

async fn do_get_room(reqwest_client: &ReqwestClient) -> Option<(String, Uuid)> {
    let room_name = get_nonempty_line("room name");

    if room_name == CMD_EXIT {
        None
    } else {
        let room_uuid = match get_room(&room_name, reqwest_client)
            .await
            .expect("Error finding room!")
        {
            Some(uuid) => uuid,
            None => {
                println!("Created room '{}'", &room_name);
                create_room(&room_name, reqwest_client)
                    .await
                    .expect("Error creating room!")
            }
        };
        Some((room_name, room_uuid))
    }
}

async fn do_join_room(
    client_uuid: Uuid,
    client_name: &str,
    room_uuid: Uuid,
    room_name: &str,
    reqwest_client: &ReqwestClient,
) {
    let success = join_room(client_uuid, client_name, room_uuid, reqwest_client)
        .await
        .expect("Error joining room!");
    if success {
        println!("Joined room '{}'", room_name);
    } else {
        panic!("Error joining room");
    }
}

async fn keep_alive(client_uuid: Uuid) {
    const HEARTBEAT_TIMEOUT: u64 = 2000;
    let heartbeat_data = HeartbeatData(ClientUuid(client_uuid));
    let heartbeat_str = serde_json::to_string(&heartbeat_data).expect("Parsing heartbeat failed");
    let client = ReqwestClient::new();

    loop {
        thread::sleep(Duration::from_millis(HEARTBEAT_TIMEOUT));
        let heartbeat_str = heartbeat_str.clone();

        let resp = client
            .post("http://".to_string() + &get_addr_str(Protocol::HTTP) + HEARTBEAT_ENDPOINT)
            .body(heartbeat_str)
            .send()
            .await
            .expect("Heartbeat request failed and app will close");
        if resp.status() != StatusCode::OK {
            panic!("Heartbeat request failed and app will close");
        }
    }
}

async fn chat_client() {
    print_greeting();
    let reqwest_client = ReqwestClient::new();
    let (mut ws_stream, _) = connect_async("ws://".to_string() + &get_addr_str(Protocol::WS))
        .await
        .expect("Failed to connect to the WS server!");

    let (client_name, client_uuid) = register_or_login(&reqwest_client, &mut ws_stream).await;
    let keep_alive_handle = tokio::spawn(keep_alive(client_uuid));

    let check_resp = |resp: Result<Response, _>| {
        if resp.expect("Failed to send message!").status() != StatusCode::OK {
            panic!("Failed to send message!");
        }
    };

    loop {
        if let Some((room_name, room_uuid)) = do_get_room(&reqwest_client).await {
            do_join_room(
                client_uuid,
                &client_name,
                room_uuid,
                &room_name,
                &reqwest_client,
            )
            .await;

            let (tx_stdin, rx) = mpsc::channel::<String>(1);
            let mut rx = ReceiverStream::new(rx);
            let stdin_loop = async move {
                loop {
                    let mut line = String::new();
                    let mut buf_stdin = tokio::io::BufReader::new(tokio::io::stdin());
                    buf_stdin.read_line(&mut line).await.unwrap();
                    tx_stdin.send(line.trim().to_string()).await.unwrap();

                    if line.trim_end_matches("") == CMD_LOBBY {
                        break;
                    }
                }
            };
            let stdin_loop = tokio::task::spawn(stdin_loop);

            loop {
                if keep_alive_handle.is_finished() {
                    return;
                }
                tokio::select! {
                    ws_msg = ws_stream.next() => {
                        match ws_msg {
                            Some(msg) => match msg {
                                Ok(msg) => match msg {
                                    TungsteniteMsg::Text(json_str) => {
                                        let mut msg = serde_json::from_str::<ChatMessage>(&json_str).unwrap();
                                        if msg.author == client_name {
                                            msg.author = String::from("YOU");
                                        }
                                        println!("{}", msg);
                                    }
                                    _ => { eprintln!("Received an invalid type of messageQ"); }
                                }
                                Err(_) => { eprintln!("WS server went away!"); return; }
                            }
                            None => { eprintln!("No message!"); return; }
                        }
                    },
                    stdin_msg = rx.next() => {
                        match stdin_msg {
                            Some(msg) => {
                                let msg = ChatMessage::new(&client_name, &msg);
                                if msg.contents == CMD_EXIT {
                                    ws_stream.close(None).await.expect("Closing ws stream failed!");
                                    check_resp(exit_app(&reqwest_client, client_uuid).await);
                                    return;
                                } else if msg.contents == CMD_LOBBY {
                                    check_resp(leave_room(&reqwest_client, client_uuid, room_uuid).await);
                                    break;
                                } else {
                                    check_resp(send_msg(&reqwest_client, msg, room_uuid).await);
                                };
                            },
                            None => return
                        }
                    }
                }
            }
            stdin_loop.abort(); // end listening for messages on this room
        } else {
            return;
        }
    }
}

#[tokio::main]
async fn main() {
    chat_client().await;
}
