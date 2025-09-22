use libc::_exit;
use nix::sys::socket::recv;
use nix::sys::socket::{socket, Backlog, SockaddrStorage, UnixAddr};
use nix::{
    errno::Errno,
    sys::{
        signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal},
        socket::{
            accept, bind, getpeername, listen, send, AddressFamily, MsgFlags, SockFlag, SockType,
        },
        wait::{waitpid, WaitPidFlag, WaitStatus},
    },
    unistd::{close, fork, ForkResult},
};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::{env, fs, process};
use serde::Deserialize;
use Signal::SIGCHLD;

extern "C" fn sigchld_handler(_signal: libc::c_int) {
    while let Ok(WaitStatus::StillAlive) = waitpid(None, Some(WaitPidFlag::WNOHANG)) {}
}

#[derive(Deserialize)]
struct CaesMsg {
    message: String,
    shift_val: String,
}

fn main() {

    let socket_path = parse_args();

    let sock = create_socket(&socket_path);

    listen_for_connections(&sock);

    let handler = SigHandler::Handler(sigchld_handler);
    let sa = SigAction::new(handler, SaFlags::empty(), SigSet::empty());
    unsafe {
        sigaction(SIGCHLD, &sa).expect("server: sigaction failed");
    };

    println!("server: waiting for connections...");
    loop {
        let session_sock = accept_client(&sock);

        match unsafe { fork() }.expect("server: fork failed") {
            ForkResult::Child => {
                close_socket(&sock);

                let incoming = receive_message(&session_sock);

                let shift: i32 = incoming.shift_val.parse().unwrap();
                let msg = encrypt_message(incoming.message.as_bytes(), shift);

                send_message(&session_sock, &msg);

                close_socket(&session_sock);
                unsafe { _exit(0) }
            }
            _ => drop(session_sock),
        }
    }
}

fn parse_args() -> String {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: server socket_path");
        process::exit(1);
    }

    let socket_path = args[1].clone();
    socket_path
}

fn create_socket(path: &str) -> OwnedFd {
    let sock = socket(AddressFamily::Unix, SockType::Stream, SockFlag::empty(), None)
        .expect("create_socket: failed to create socket");

    let _ = fs::remove_file(path);

    let sockaddr = UnixAddr::new(path.as_bytes()).expect("bad socket path");
    bind(sock.as_raw_fd(), &sockaddr).expect("server: bind failed");

    sock
}

fn listen_for_connections(sock: &OwnedFd) {
    let backlog = Backlog::new(10).unwrap();
    listen(sock, backlog).expect("server listen failed");
}

fn accept_client(sock: &OwnedFd) -> OwnedFd {
    loop {
        match accept(sock.as_raw_fd()) {
            Err(Errno::EINTR) => continue,
            Ok(raw_fd) => {
                if let Ok(_saddr) = getpeername::<SockaddrStorage>(raw_fd) {
                    println!("server: got connection from client");
                }
                return unsafe { OwnedFd::from_raw_fd(raw_fd) };
            }
            _ => panic!("server: accept failed"),
        }
    }
}

fn receive_message(sock: &OwnedFd) -> CaesMsg {
    let mut buf = [0u8; 1024];
    let nbytes = recv(sock.as_raw_fd(), &mut buf, MsgFlags::empty())
        .expect("server: receive failed");

    let data = &buf[..nbytes];
    serde_json::from_slice(data).expect("server: failed to parse JSON")
}

fn encrypt_message(msg: &[u8], shift: i32) -> Vec<u8> {
    let mut result = Vec::new();
    let shiftby = ((shift % 26 + 26) % 26) as u8;

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

fn send_message(sock: &OwnedFd, msg: &[u8]) {
    send(sock.as_raw_fd(), msg, MsgFlags::empty()).expect("server: send failed");
}

fn close_socket(sock: &OwnedFd) {
    close(sock.as_raw_fd()).expect("server: close failed");
}