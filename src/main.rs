use std::{
    fs,
    io::{prelude::*, BufReader, Read, Write},
    net::TcpListener,
};

use hello::ThreadPool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    let pool = ThreadPool::build(-1)?;

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(_) => {
                eprintln!("Got failed connection, ignoring.");
                continue;
            }
        };

        let _ = pool.execute(|| {
            handle_connection(stream);
        });
    }

    println!("Shutting down.");
    Ok(())
}

fn handle_connection<T>(mut stream: T)
where
    T: Read + Write,
{
    let buf_reader = BufReader::new(&mut stream);
    let res = buf_reader.lines().next();
    let request_line = match res {
        Some(Ok(line)) => Some(line),
        _ => {
            eprintln!("Got malformed request.");
            None
        }
    };

    let (status_line, filename) = if request_line == Some("GET / HTTP/1.1".to_string()) {
        ("HTTP/1.1 200 OK", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };

    let contents = fs::read_to_string(filename).unwrap();
    let length = contents.len();

    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
    println!("{}", response);

    stream.write_all(response.as_bytes()).unwrap();
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_handle_connection_with_valid_request() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = Cursor::new(b"GET / HTTP/1.1".to_vec());
        stream.seek(std::io::SeekFrom::Start(0))?;
        handle_connection(&mut stream);

        let mut output = String::new();
        stream.seek(std::io::SeekFrom::Start(0))?;
        stream.read_to_string(&mut output)?;

        assert!(output.contains("HTTP/1.1 200 OK"));
        assert!(output.contains("Content-Length: "));
        Ok(())
    }

    #[test]
    fn test_handle_connection_invalid_request() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = Cursor::new(b"INVALID".to_vec());
        stream.seek(std::io::SeekFrom::Start(0))?;
        handle_connection(&mut stream);

        let mut output = String::new();
        stream.seek(std::io::SeekFrom::Start(0))?;
        stream.read_to_string(&mut output)?;

        assert!(output.contains("HTTP/1.1 404 NOT FOUND"));
        assert!(output.contains("Content-Length: "));
        Ok(())
    }
}
