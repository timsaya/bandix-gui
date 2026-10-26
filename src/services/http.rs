use crate::i18n::{tf, tr};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 8686;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(900);
const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct Endpoint {
    host: String,
    port: u16,
}

impl Endpoint {
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim().trim_end_matches('/');
        if input.is_empty() {
            return Err(tr("请输入服务地址").to_string());
        }
        if input.starts_with("https://") {
            return Err(tr("Bandix 当前仅支持 http:// 地址").to_string());
        }

        let authority = input.strip_prefix("http://").unwrap_or(input);
        let authority = authority.split('/').next().unwrap_or(authority);
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
                let port = port
                    .parse::<u16>()
                    .map_err(|_| tr("端口号无效").to_string())?;
                (host.to_string(), port)
            }
            _ => (authority.to_string(), DEFAULT_PORT),
        };

        if host.is_empty() || host.contains(char::is_whitespace) {
            return Err(tr("服务地址无效").to_string());
        }
        Ok(Self { host, port })
    }

    pub fn resolve(&self) -> Result<SocketAddr, String> {
        (self.host.as_str(), self.port)
            .to_socket_addrs()
            .map_err(|error| tf("无法解析地址：{0}", &[format!("{}", error)]))?
            .find(|address| address.is_ipv4())
            .ok_or_else(|| tr("没有找到可用的 IPv4 地址").to_string())
    }

    pub fn display_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    pub fn connect(&self) -> Result<TcpStream, String> {
        TcpStream::connect_timeout(&self.resolve()?, CONNECT_TIMEOUT).map_err(|error| {
            tf(
                "无法连接 {0}:{1}：{2}",
                &[
                    format!("{}", self.host),
                    format!("{}", self.port),
                    format!("{}", error),
                ],
            )
        })
    }

    pub fn get(&self, path: &str) -> Result<HttpResponse, String> {
        self.request("GET", path, None)
    }

    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<HttpResponse, String> {
        let mut stream = self.connect()?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .map_err(|error| error.to_string())?;

        let body = body.unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.host,
            self.port,
            body.len(),
        ).into_bytes();
        // Bandix's server parses its first read immediately. Send the JSON with
        // the headers so a small write request isn't parsed with an empty body.
        request.extend_from_slice(body);
        stream
            .write_all(&request)
            .map_err(|error| error.to_string())?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| error.to_string())?;
        parse_response(&response)
    }

    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let response = self.get(path)?;
        if response.status != 200 {
            return Err(response.error_message());
        }

        let envelope: ApiEnvelope<T> = serde_json::from_slice(&response.body)
            .map_err(|error| tf("JSON 解析失败：{0}", &[format!("{}", error)]))?;
        if envelope.status != "success" {
            return Err(envelope
                .message
                .unwrap_or_else(|| tr("Bandix 接口返回错误").to_string()));
        }
        envelope
            .data
            .ok_or_else(|| tr("Bandix 接口未返回 data").to_string())
    }

    pub fn send_json<T: Serialize>(
        &self,
        method: &str,
        path: &str,
        payload: &T,
    ) -> Result<(), String> {
        let body = serde_json::to_vec(payload)
            .map_err(|error| tf("JSON 编码失败：{0}", &[format!("{}", error)]))?;
        let response = self.request(method, path, Some(&body))?;
        if !(200..300).contains(&response.status) {
            return Err(response.error_message());
        }

        let envelope: ApiEnvelope<serde_json::Value> = serde_json::from_slice(&response.body)
            .map_err(|error| tf("JSON 解析失败：{0}", &[format!("{}", error)]))?;
        if envelope.status != "success" {
            return Err(envelope
                .message
                .unwrap_or_else(|| tr("Bandix 接口返回错误").to_string()));
        }
        Ok(())
    }

    pub fn send_json_with_response<P: Serialize, T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        payload: &P,
    ) -> Result<T, String> {
        let body = serde_json::to_vec(payload)
            .map_err(|error| tf("JSON 编码失败：{0}", &[format!("{}", error)]))?;
        let response = self.request(method, path, Some(&body))?;
        if !(200..300).contains(&response.status) {
            return Err(response.error_message());
        }

        let envelope: ApiEnvelope<T> = serde_json::from_slice(&response.body)
            .map_err(|error| tf("JSON 解析失败：{0}", &[format!("{}", error)]))?;
        if envelope.status != "success" {
            return Err(envelope
                .message
                .unwrap_or_else(|| tr("Bandix 接口返回错误").to_string()));
        }
        envelope
            .data
            .ok_or_else(|| tr("Bandix 接口未返回 data").to_string())
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    fn error_message(&self) -> String {
        let message = serde_json::from_slice::<ApiEnvelope<serde_json::Value>>(&self.body)
            .ok()
            .and_then(|envelope| envelope.message)
            .filter(|message| !message.trim().is_empty());
        match message {
            Some(message) => format!("HTTP {}：{message}", self.status),
            None => format!("HTTP {}", self.status),
        }
    }
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    status: String,
    data: Option<T>,
    message: Option<String>,
}

fn parse_response(response: &[u8]) -> Result<HttpResponse, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| tr("无效的 HTTP 响应").to_string())?;
    let head = String::from_utf8_lossy(&response[..header_end]);
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| tr("无效的 HTTP 状态码").to_string())?;
    Ok(HttpResponse {
        status,
        body: response[header_end + 4..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn endpoint_accepts_host_url_and_custom_port() {
        assert_eq!(
            Endpoint::parse("192.168.1.1").unwrap().display_url(),
            "http://192.168.1.1:8686"
        );
        assert_eq!(
            Endpoint::parse("http://router.local:9000/")
                .unwrap()
                .display_url(),
            "http://router.local:9000"
        );
    }

    #[test]
    fn json_post_delivers_a_complete_utf8_body_to_a_single_read_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            // Match the Bandix server, which handles only its first read.
            let mut request = [0_u8; 4096];
            let count = stream.read(&mut request).unwrap();
            let body = br#"{"status":"success","data":null,"message":null}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            request[..count].to_vec()
        });
        let endpoint = Endpoint::parse(&address.to_string()).unwrap();
        let payload = serde_json::json!({"mac": "aa:bb:cc:dd:ee:ff", "hostname": "笔记本 \"A\""});
        let result = endpoint.send_json("POST", "/api/traffic/bindings", &payload);
        let request = server.join().unwrap();
        result.unwrap();
        let header_end = request
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .unwrap();
        let headers = std::str::from_utf8(&request[..header_end]).unwrap();
        let body = &request[header_end + 4..];
        let content_length: usize = headers
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(body.len(), content_length);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(body).unwrap(),
            payload
        );
    }

    #[test]
    fn http_errors_include_the_server_message_and_allow_non_json_responses() {
        let json = HttpResponse {
            status: 500,
            body: br#"{"status":"error","message":"Cannot save hostname"}"#.to_vec(),
        };
        assert_eq!(json.error_message(), "HTTP 500：Cannot save hostname");
        let text = HttpResponse {
            status: 502,
            body: b"Bad Gateway".to_vec(),
        };
        assert_eq!(text.error_message(), "HTTP 502");
    }
}
