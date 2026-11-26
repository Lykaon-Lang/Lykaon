//! M5e: the five real apps under examples/m5.
//!
//! The web server is not started here (it listens); the other four run.

use lykaon::run_source;

const DATA: &str = include_str!("../../examples/m5/data-report.lykaon");
const WORKER: &str = include_str!("../../examples/m5/worker.lykaon");
const TEXT: &str = include_str!("../../examples/m5/text-tool.lykaon");
const CLI: &str = include_str!("../../examples/m5/cli-tool.lykaon");

fn out(src: &str) -> String {
    let (out, code) = run_source(src, "program.lykaon", "");
    assert_eq!(code, 0, "the app failed: {out}");
    out.trim_end().to_string()
}

#[test]
fn m5e_data_report() {
    let text = out(DATA);
    assert!(text.contains("rows 8"), "{text}");
    assert!(text.contains("average price 111.88"), "{text}");
    assert!(text.contains("east    250"), "{text}");
    let _ = std::fs::remove_dir_all("output");
}

#[test]
fn m5e_worker() {
    let text = out(WORKER);
    assert_eq!(text, "1\n2\n3");
}

#[test]
fn m5e_text_tool() {
    let text = out(TEXT);
    assert_eq!(text, "3\ncat,sat,down");
}

#[test]
fn m5e_cli_tool() {
    let (text, code) = lykaon::run_source_tool(CLI, "cli-tool.lykaon", "", &["Lykaon".to_string()], true);
    assert_eq!(code, 0, "the CLI tool failed: {text}");
    assert_eq!(text.trim(), "hello Lykaon");
}


#[test]
fn m5e_web_server() {
    // The web server listens forever, so it runs with the test-only
    // LYKAON_SERVE_MAX_REQUESTS knob (one request) and on a free port.
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::{Duration, Instant};

    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock().unwrap();

    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let app = include_str!("../../examples/m5/web-server.lykaon");
    let src = app.replace("8080", &port.to_string());

    unsafe {
        std::env::set_var("LYKAON_SERVE_MAX_REQUESTS", "1");
    }
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || lykaon::run_source(&src, "web-server.lykaon", ""))
        .expect("spawn server");

    // Wait for the port, then make exactly one real HTTP request and read the
    // whole response (headers + body may arrive in more than one packet).
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut response = String::new();
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = s.write_all(
                    b"GET /hello HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                );
                let mut buf = [0u8; 4096];
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => response.push_str(&String::from_utf8_lossy(&buf[..n])),
                    }
                }
                break;
            }
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("server did not start on port {port}: {e}"),
        }
    }

    unsafe {
        std::env::remove_var("LYKAON_SERVE_MAX_REQUESTS");
    }
    let (srv_out, srv_code) = handle.join().expect("server thread panicked");
    let _ = std::fs::remove_file("shopdata");
    assert!(
        response.contains("hello"),
        "response: {response:?}\nserver exit {srv_code}: {srv_out}"
    );
}
