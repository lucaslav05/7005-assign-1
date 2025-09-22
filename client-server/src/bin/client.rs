use nix::sys::socket::{send, UnixAddr};
use nix::{
    sys::socket::{
        connect, recv, socket, AddressFamily, MsgFlags, SockFlag, SockType,
    },
    unistd::close,
};
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::Path;
use std::{env, process};
use serde::Serialize;

#[derive(Serialize)]
struct CaesMsg {
    message: String,
    shift_val: String,
}

fn main() {
    let (sock_path, msg, shift) = parse_args();

    let client_msg = CaesMsg {
        message: msg,
        shift_val: shift,
    };

    let sock = create_socket();

    connect_to_server(&sock, &*sock_path);
 
    send_message(&sock, &client_msg);

    let mut buf = vec![0; client_msg.message.len()];
    receive_message(&sock, &mut buf);

    if let Ok(response) = std::str::from_utf8(&buf) {
        println!("Encrypted Message: {}", response);
        let decrypt_msg = String::from_utf8(decrypt_message(response.as_bytes(), client_msg.shift_val.parse().unwrap())).unwrap();
        println!("Decrypted Message: {}", decrypt_msg);
    }

    close_socket(sock);
}

fn parse_args() -> (String, String, String) {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("usage: client socket_path message shift_value");
        process::exit(1);
    }

    let socket_path = args[1].clone();
    let message = args[2].clone();
    let shift_val = args[3].clone();


    (socket_path, message, shift_val)
}

fn create_socket() -> OwnedFd {
    socket(AddressFamily::Unix, SockType::Stream, SockFlag::empty(), None)
        .expect("create_socket: failed to create socket")
}

fn connect_to_server(sock: &OwnedFd, path: &str) {
    let sock_addr = UnixAddr::new(Path::new(path))
        .expect("connect_to_server: invalid socket path");
    connect(sock.as_raw_fd(), &sock_addr).expect("connect_to_server: connect failed");
}

fn send_message(sock: &OwnedFd, msg: &CaesMsg) {
    let bytes = serde_json::to_vec(msg).expect("Failed to serialize message");
    let bytes_sent = send(sock.as_raw_fd(), &bytes, MsgFlags::empty())
        .expect("send_message: send failed");
    println!("Sent {} bytes", bytes_sent);
}

fn receive_message(sock: &OwnedFd, buf: &mut [u8]){
    let bytes_read = recv(sock.as_raw_fd(), buf, MsgFlags::empty())
        .expect("send_message: send failed");
    println!("Received {} bytes", bytes_read);
}

fn decrypt_message(msg: &[u8], shift: u32) -> Vec<u8> {
    let mut result = Vec::new();
    let shiftby = 26 - ((shift % 26) as u8);

    for c in msg {
        if c.is_ascii_whitespace() {
            result.push(*c);
            continue;
        }

        if c.is_ascii_uppercase() {
            let shifted = ((c - b'A' + shiftby) % 26) + b'A';
            result.push(shifted);
        } else if c.is_ascii_lowercase() {
            let shifted = ((c - b'a' + shiftby) % 26) + b'a';
            result.push(shifted);
        } else {
            result.push(*c);
        }
    }
    result
}

fn close_socket(sock: OwnedFd) {
    close(sock).expect("close_socket: failed to close socket");
}