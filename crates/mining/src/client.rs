use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::{
    ApiErrorBody, ChallengeResponse, DEFAULT_USER_AGENT, HeartbeatRequest, HeartbeatResponse,
    MiningError, StatusResponse, SubmitRequest, SubmitResponse, localized_message,
};

#[derive(Debug, Clone)]
pub(crate) struct MiningClient {
    base_url: String,
    timeout: Duration,
    http_client: Arc<Mutex<Client>>,
}

impl MiningClient {
    pub(crate) fn new(base_url: &str, timeout: Duration) -> Result<Self, MiningError> {
        let http_client = Self::build_http_client(timeout)?;
        Ok(Self {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            timeout,
            http_client: Arc::new(Mutex::new(http_client)),
        })
    }

    pub(crate) fn reset_session(&self) -> Result<(), MiningError> {
        let rebuilt = Self::build_http_client(self.timeout)?;
        *self
            .http_client
            .lock()
            .expect("mining client mutex poisoned") = rebuilt;
        Ok(())
    }

    pub(crate) fn get_status(&self) -> Result<StatusResponse, MiningError> {
        let response = self.get_status_snapshot()?;
        if !response.enabled {
            return Err(MiningError::PoolDisabled);
        }
        if response
            .current_round
            .as_ref()
            .is_none_or(|round| !round.is_open())
        {
            return Err(MiningError::NoOpenRound);
        }
        Ok(response)
    }

    pub(crate) fn get_status_snapshot(&self) -> Result<StatusResponse, MiningError> {
        self.get_json("/mining-api/status")
            .map_err(|error| match error {
                MiningError::Json(error) => {
                    MiningError::Message(format!("状态响应解析失败：{}", error))
                }
                other => other,
            })
    }

    pub(crate) fn get_challenge(&self) -> Result<ChallengeResponse, MiningError> {
        let response: ChallengeResponse = self.post_empty_json("/mining-api/challenge")?;
        if !response.ok {
            if response.result == "daily win limit reached"
                || response.message == "daily win limit reached"
            {
                return Err(MiningError::DailyLimit);
            }
            let message = if !response.message.trim().is_empty() {
                localized_message(&response.message, "挑战被矿池拒绝")
            } else if !response.result.trim().is_empty() {
                crate::result_label(&response.result)
            } else {
                "挑战被矿池拒绝".to_string()
            };
            return Err(MiningError::ChallengeRejected(message));
        }
        Ok(response)
    }

    pub(crate) fn heartbeat(
        &self,
        challenge_id: i32,
        round_id: i32,
    ) -> Result<HeartbeatResponse, MiningError> {
        let response: HeartbeatResponse = self.post_json(
            "/mining-api/heartbeat",
            &HeartbeatRequest {
                challenge_id,
                round_id,
            },
        )?;
        if response.result.trim().eq_ignore_ascii_case("round_closed") {
            return Err(MiningError::RoundClosed);
        }
        Ok(response)
    }

    pub(crate) fn submit(
        &self,
        challenge_id: i32,
        round_id: i32,
        nonce: usize,
        digest: &str,
        preference: &str,
    ) -> Result<SubmitResponse, MiningError> {
        self.post_json(
            "/mining-api/submit",
            &SubmitRequest {
                challenge_id,
                round_id,
                nonce: nonce.to_string(),
                digest: digest.to_string(),
                preference: preference.to_string(),
            },
        )
    }

    fn build_http_client(timeout: Duration) -> Result<Client, MiningError> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        Ok(Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .connect_timeout(timeout.min(Duration::from_secs(10)))
            .cookie_store(true)
            .build()?)
    }

    fn cloned_http_client(&self) -> Client {
        self.http_client
            .lock()
            .expect("mining client mutex poisoned")
            .clone()
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, MiningError> {
        let response = self
            .cloned_http_client()
            .get(format!("{}{}", self.base_url, path))
            .send()?;
        self.decode_response(response)
    }

    fn post_json<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        payload: &B,
    ) -> Result<T, MiningError> {
        let response = self
            .cloned_http_client()
            .post(format!("{}{}", self.base_url, path))
            .json(payload)
            .send()?;
        self.decode_response(response)
    }

    fn post_empty_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, MiningError> {
        let response = self
            .cloned_http_client()
            .post(format!("{}{}", self.base_url, path))
            .header(CONTENT_TYPE, "application/json")
            .body(Vec::new())
            .send()?;
        self.decode_response(response)
    }

    fn decode_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<T, MiningError> {
        let status = response.status();
        let text = response.text()?;
        if !status.is_success() {
            let error = serde_json::from_str::<ApiErrorBody>(&text).ok();
            if status.as_u16() == 401 {
                return Err(MiningError::Message(
                    "登录状态已失效，请重新登录".to_string(),
                ));
            }
            let message = error
                .as_ref()
                .map(|item| {
                    if !item.message.trim().is_empty() {
                        localized_message(&item.message, "服务端返回错误")
                    } else if !item.reason.trim().is_empty() {
                        localized_message(&item.reason, "服务端返回错误")
                    } else {
                        localized_message(&text, "服务端返回错误")
                    }
                })
                .unwrap_or_else(|| localized_message(&text, "服务端返回错误"));
            return Err(MiningError::Message(format!(
                "请求失败（状态码 {}）：{}",
                status.as_u16(),
                message
            )));
        }
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(test)]
mod tests {
    use super::MiningClient;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn get_challenge_posts_empty_json_body_like_go_client() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set timeout");
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();

            assert!(request.starts_with("POST /mining-api/challenge HTTP/1.1\r\n"));
            assert!(
                request.contains("content-type: application/json\r\n")
                    || request.contains("Content-Type: application/json\r\n")
            );
            assert!(
                request.contains("content-length: 0\r\n")
                    || request.contains("Content-Length: 0\r\n")
            );
            assert!(
                request.ends_with("\r\n\r\n"),
                "request should end after headers without JSON body: {request}"
            );
            assert!(
                !request.contains("\r\n\r\n{}"),
                "request unexpectedly contained an empty object body: {request}"
            );

            let response_body = "{\"ok\":true,\"challenge_id\":1,\"round_id\":2,\"difficulty_bits\":8,\"memory_cost_mb\":64,\"parallelism\":1,\"seed\":\"s\",\"session_salt\":\"x\",\"time_cost\":1,\"visitor_id\":\"v\"}";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body,
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let client = MiningClient::new(&format!("http://{}", address), Duration::from_secs(2))
            .expect("create client");
        let challenge = client.get_challenge().expect("fetch challenge");

        assert!(challenge.ok);
        assert_eq!(challenge.challenge_id, 1);
        assert_eq!(challenge.round_id, 2);

        server.join().expect("server thread");
    }

    #[test]
    fn reset_session_rebuilds_cookie_store() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("listener addr");
        let server = thread::spawn(move || {
            for request_index in 0..3 {
                let (mut stream, _) = listener.accept().expect("accept request");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("set timeout");
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).expect("read request");
                let request = String::from_utf8_lossy(&buffer[..read]).into_owned();

                match request_index {
                    0 => assert!(
                        !request.to_ascii_lowercase().contains("\r\ncookie:"),
                        "first request should not have cookies: {request}"
                    ),
                    1 => assert!(
                        request.contains("Cookie: sid=first\r\n")
                            || request.contains("cookie: sid=first\r\n"),
                        "second request should reuse cookie jar: {request}"
                    ),
                    2 => assert!(
                        !request.to_ascii_lowercase().contains("\r\ncookie:"),
                        "third request should clear cookies after reset: {request}"
                    ),
                    _ => unreachable!(),
                }

                let response_body = "{\"enabled\":true,\"current_round\":{\"id\":1,\"difficulty_bits\":16,\"status\":\"open\"},\"inventory_remaining\":1,\"pool_stats\":{\"invite_unused\":1,\"balance_unused\":1}}";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nSet-Cookie: sid=first; Path=/\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body,
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
        });

        let client = MiningClient::new(&format!("http://{}", address), Duration::from_secs(2))
            .expect("create client");
        client.get_status_snapshot().expect("first status request");
        client.get_status_snapshot().expect("second status request");
        client.reset_session().expect("reset session");
        client.get_status_snapshot().expect("third status request");

        server.join().expect("server thread");
    }
}
