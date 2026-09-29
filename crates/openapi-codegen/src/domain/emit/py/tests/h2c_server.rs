use super::*;

pub(super) fn spawn_h2c_smoke_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let started = Instant::now();
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(accepted) => break accepted,
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(5) =>
                {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("h2c smoke server accept failed: {err}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut preface = [0_u8; 24];
        stream.read_exact(&mut preface).unwrap();
        assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
        write_frame(&mut stream, 4, 0, 0, &[]);

        loop {
            let (kind, flags, stream_id, payload) = read_frame(&mut stream);
            match kind {
                4 if flags & 0x1 == 0 => write_frame(&mut stream, 4, 0x1, 0, &[]),
                1 => {
                    assert_eq!(stream_id, 1);
                    assert!(
                        !payload.is_empty(),
                        "client HEADERS payload should carry HPACK block"
                    );
                    let body = br#"{"id":42,"name":"Ada","tag":"h2c"}"#;
                    let headers = response_headers(body.len());
                    write_frame(&mut stream, 1, 0x4, stream_id, &headers);
                    write_frame(&mut stream, 0, 0x1, stream_id, body);
                    thread::sleep(Duration::from_millis(200));
                    return;
                }
                _ => {}
            }
        }
    });
    (format!("http://{addr}"), handle)
}

pub(super) fn spawn_h2c_sequential_server(
    expected_requests: usize,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let mut stream = accept_h2c(listener, "sequential");
        let mut served = 0_usize;
        while served < expected_requests {
            let (kind, flags, stream_id, payload) = read_frame(&mut stream);
            match kind {
                4 if flags & 0x1 == 0 => write_frame(&mut stream, 4, 0x1, 0, &[]),
                1 => {
                    assert!(
                        !payload.is_empty(),
                        "client HEADERS payload should carry HPACK block"
                    );
                    let body = format!(r#"{{"stream":{stream_id}}}"#);
                    let headers = response_headers(body.len());
                    write_frame(&mut stream, 1, 0x4, stream_id, &headers);
                    write_frame(&mut stream, 0, 0x1, stream_id, body.as_bytes());
                    served += 1;
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(100));
    });
    (format!("http://{addr}"), handle)
}

pub(super) fn spawn_h2c_multiplex_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let mut stream = accept_h2c(listener, "multiplex");
        let mut stream_ids = Vec::new();
        while stream_ids.len() < 2 {
            let (kind, flags, stream_id, payload) = read_frame(&mut stream);
            match kind {
                4 if flags & 0x1 == 0 => write_frame(&mut stream, 4, 0x1, 0, &[]),
                1 => {
                    assert!(
                        !payload.is_empty(),
                        "client HEADERS payload should carry HPACK block"
                    );
                    stream_ids.push(stream_id);
                }
                _ => {}
            }
        }
        for stream_id in stream_ids.iter().rev() {
            let body = format!(r#"{{"stream":{stream_id}}}"#);
            let headers = response_headers(body.len());
            write_frame(&mut stream, 1, 0x4, *stream_id, &headers);
            write_frame(&mut stream, 0, 0x1, *stream_id, body.as_bytes());
        }
        thread::sleep(Duration::from_millis(100));
    });
    (format!("http://{addr}"), handle)
}

pub(super) fn spawn_h2c_bidi_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let mut stream = accept_h2c(listener, "bidi");
        let mut response_started = false;
        loop {
            let (kind, flags, stream_id, payload) = read_frame(&mut stream);
            match kind {
                4 if flags & 0x1 == 0 => write_frame(&mut stream, 4, 0x1, 0, &[]),
                1 => {
                    assert_eq!(stream_id, 1);
                    assert!(
                        !payload.is_empty(),
                        "client HEADERS payload should carry HPACK block"
                    );
                    let headers = streaming_response_headers();
                    write_frame(&mut stream, 1, 0x4, stream_id, &headers);
                    response_started = true;
                }
                0 if stream_id == 1 => {
                    assert!(response_started, "response HEADERS should be sent first");
                    if !payload.is_empty() {
                        let body = format!("ack:{}", String::from_utf8_lossy(&payload));
                        write_frame(
                            &mut stream,
                            0,
                            if flags & 0x1 != 0 { 0x1 } else { 0 },
                            stream_id,
                            body.as_bytes(),
                        );
                    } else if flags & 0x1 != 0 {
                        write_frame(&mut stream, 0, 0x1, stream_id, &[]);
                    }
                    if flags & 0x1 != 0 {
                        thread::sleep(Duration::from_millis(100));
                        return;
                    }
                }
                _ => {}
            }
        }
    });
    (format!("http://{addr}"), handle)
}

fn accept_h2c(listener: TcpListener, label: &str) -> TcpStream {
    let started = Instant::now();
    let (mut stream, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("h2c {label} server accept failed: {err}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut preface = [0_u8; 24];
    stream.read_exact(&mut preface).unwrap();
    assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
    write_frame(&mut stream, 4, 0, 0, &[]);
    stream
}

fn read_frame(stream: &mut TcpStream) -> (u8, u8, u32, Vec<u8>) {
    let mut head = [0_u8; 9];
    stream.read_exact(&mut head).unwrap();
    let len = ((head[0] as usize) << 16) | ((head[1] as usize) << 8) | head[2] as usize;
    let kind = head[3];
    let flags = head[4];
    let stream_id = u32::from_be_bytes([head[5], head[6], head[7], head[8]]) & 0x7fff_ffff;
    let mut payload = vec![0_u8; len];
    stream.read_exact(&mut payload).unwrap();
    (kind, flags, stream_id, payload)
}

fn write_frame(stream: &mut TcpStream, kind: u8, flags: u8, stream_id: u32, payload: &[u8]) {
    assert!(payload.len() <= 0x00ff_ffff);
    let len = payload.len();
    let mut head = [0_u8; 9];
    head[0] = ((len >> 16) & 0xff) as u8;
    head[1] = ((len >> 8) & 0xff) as u8;
    head[2] = (len & 0xff) as u8;
    head[3] = kind;
    head[4] = flags;
    head[5..9].copy_from_slice(&(stream_id & 0x7fff_ffff).to_be_bytes());
    stream.write_all(&head).unwrap();
    stream.write_all(payload).unwrap();
}

fn response_headers(content_len: usize) -> Vec<u8> {
    let mut out = vec![0x88]; // indexed static :status 200
    hpack_literal(&mut out, "content-type", "application/json");
    hpack_literal(&mut out, "content-length", &content_len.to_string());
    out
}

fn streaming_response_headers() -> Vec<u8> {
    let mut out = vec![0x88]; // indexed static :status 200
    hpack_literal(&mut out, "content-type", "application/json");
    out
}

fn hpack_literal(out: &mut Vec<u8>, name: &str, value: &str) {
    out.push(0x00);
    hpack_huffman_string(out, name);
    hpack_huffman_string(out, value);
}

fn hpack_huffman_string(out: &mut Vec<u8>, value: &str) {
    let bytes: &[u8] = match value {
        "content-type" => &[0x21, 0xea, 0x49, 0x6a, 0x4a, 0xc9, 0xf5, 0x59, 0x7f],
        "application/json" => &[
            0x1d, 0x75, 0xd0, 0x62, 0x0d, 0x26, 0x3d, 0x4c, 0x74, 0x41, 0xea,
        ],
        "content-length" => &[0x21, 0xea, 0x49, 0x6a, 0x4a, 0xd4, 0x16, 0xa9, 0x93, 0x3f],
        "34" => &[0x65, 0xaf],
        _ => {
            hpack_string(out, value);
            return;
        }
    };
    hpack_int(out, bytes.len(), 7, 0x80);
    out.extend_from_slice(bytes);
}

fn hpack_string(out: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    hpack_int(out, bytes.len(), 7, 0);
    out.extend_from_slice(bytes);
}

fn hpack_int(out: &mut Vec<u8>, mut value: usize, prefix_bits: u8, prefix: u8) {
    let max_prefix = (1_usize << prefix_bits) - 1;
    if value < max_prefix {
        out.push(prefix | value as u8);
        return;
    }
    out.push(prefix | max_prefix as u8);
    value -= max_prefix;
    while value >= 128 {
        out.push(((value % 128) as u8) | 0x80);
        value /= 128;
    }
    out.push(value as u8);
}
