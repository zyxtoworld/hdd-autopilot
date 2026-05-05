use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use tempfile::tempdir;

use crate::api::ApiClient;
use crate::model::{
    AccountRunSummary, AuthCache, AuthConfig, ConfigResponse, DIFFICULTY_ORDER, HistoryItem,
    Powerups, RoundResultSummary, SessionSnapshot, StartResponse, StepRequest, StepResponse, Tile,
};
use crate::storage::{cache_from_login, get_session};

use super::auth::{ensure_authenticated, with_auth_retry};
use super::log::localized_difficulty_list;
use super::round::{
    RoundPlayContext, RoundProgress, merge_round_into_summary, play_round, remaining_plays,
};
use super::snapshot::{
    collect_tile_ids, fixed_click_queue, history_item_to_start_response,
    snapshot_from_start_response, snapshot_from_step_response,
};
use super::{AccountRuntime, BatchState};

#[test]
fn localized_difficulty_list_uses_chinese_labels() {
    assert_eq!(
        localized_difficulty_list(DIFFICULTY_ORDER),
        "简单、普通、困难、地狱"
    );
}

#[test]
fn pending_status_is_ignored_not_failed() {
    let mut summary = AccountRunSummary::default();
    let result = RoundResultSummary {
        status: "pending".to_string(),
        ..RoundResultSummary::default()
    };

    merge_round_into_summary(&mut summary, &result);

    assert_eq!(summary.won, 0);
    assert_eq!(summary.failed, 0);
}

#[test]
fn fixed_click_queue_uses_snapshot_tiles_in_desc_order() {
    let start = StartResponse {
        difficulty: "easy".to_string(),
        session_id: 42,
        slot_limit: 7,
        slots: vec![3],
        tiles: vec![
            Tile {
                id: 7,
                pattern: "A".to_string(),
                ..Tile::default()
            },
            Tile {
                id: 3,
                pattern: "B".to_string(),
                ..Tile::default()
            },
            Tile {
                id: 9,
                pattern: "C".to_string(),
                ..Tile::default()
            },
        ],
        ..StartResponse::default()
    };

    let snapshot = snapshot_from_start_response(&start);

    assert_eq!(collect_tile_ids(&snapshot.tiles), vec![7, 9]);
    assert_eq!(fixed_click_queue(&snapshot), vec![9, 7]);
}

#[test]
fn continued_round_uses_same_fixed_queue_rule() {
    let start = history_item_to_start_response(&HistoryItem {
        difficulty: "easy".to_string(),
        session_id: 24,
        slot_limit: 7,
        slots: vec![5],
        tiles: vec![
            Tile {
                id: 8,
                pattern: "A".to_string(),
                ..Tile::default()
            },
            Tile {
                id: 5,
                pattern: "B".to_string(),
                ..Tile::default()
            },
            Tile {
                id: 2,
                pattern: "C".to_string(),
                ..Tile::default()
            },
        ],
        ..HistoryItem::default()
    });

    let snapshot = snapshot_from_start_response(&start);

    assert_eq!(fixed_click_queue(&snapshot), vec![8, 2]);
}

#[test]
fn play_round_consumes_fixed_initial_queue_without_replanning() {
    let requested_tile_ids = Arc::new(Mutex::new(Vec::<i32>::new()));
    let requested_tile_ids_for_server = Arc::clone(&requested_tile_ids);
    let me_calls = Arc::new(AtomicUsize::new(0));
    let me_calls_for_server = Arc::clone(&me_calls);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/tile-api/me" => {
            me_calls_for_server.fetch_add(1, Ordering::SeqCst);
            ResponseSpec::json(
                200,
                r#"{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":1},"daily_plays_used":{"easy":0},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}"#,
            )
        }
        "/tile-api/step" => {
            let step: StepRequest = serde_json::from_str(&request.body).unwrap();
            requested_tile_ids_for_server
                .lock()
                .unwrap()
                .push(step.tile_id);
            let response = match step.tile_id {
                9 => {
                    r#"{"action":"click","move_count":1,"ok":true,"reward_amount":0.0,"session_id":42,"slot_limit":7,"slots":[],"status":"pending","tiles":[{"id":100,"pattern":"X"},{"id":101,"pattern":"Y"}],"total_tiles":2}"#
                }
                5 => {
                    r#"{"action":"click","move_count":2,"ok":true,"reward_amount":0.0,"session_id":42,"slot_limit":7,"slots":[],"status":"pending","tiles":[{"id":200,"pattern":"Z"}],"total_tiles":1}"#
                }
                2 => {
                    r#"{"action":"click","move_count":3,"ok":true,"reward_amount":0.0,"session_id":42,"slot_limit":7,"slots":[],"status":"pending","tiles":[{"id":300,"pattern":"Q"}],"total_tiles":1}"#
                }
                other => panic!("unexpected tile id {other}"),
            };
            ResponseSpec::json(200, response)
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });
    let temp = tempdir().unwrap();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: server.base_url().to_string(),
            accounts: vec![],
        },
        auth_cache_file: None,
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer test-token".to_string(),
    };
    let start = StartResponse {
        difficulty: "easy".to_string(),
        session_id: 42,
        slot_limit: 7,
        status: "pending".to_string(),
        tiles: vec![
            Tile {
                id: 2,
                pattern: "A".to_string(),
                ..Tile::default()
            },
            Tile {
                id: 9,
                pattern: "B".to_string(),
                ..Tile::default()
            },
            Tile {
                id: 5,
                pattern: "C".to_string(),
                ..Tile::default()
            },
        ],
        ..StartResponse::default()
    };

    let result = play_round(
        &cancel_flag,
        &state,
        &mut runtime,
        &ConfigResponse {
            min_interval_ms: 0,
            ..ConfigResponse::default()
        },
        &start,
        RoundPlayContext {
            continued: false,
            progress: RoundProgress {
                current: 1,
                total: 1,
            },
            remaining_after: 0,
        },
    )
    .unwrap();

    assert_eq!(*requested_tile_ids.lock().unwrap(), vec![9, 5, 2]);
    assert_eq!(me_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        result.error_message,
        "这一局的可点击步骤已经用完，但服务端仍显示未通关。"
    );
}

#[test]
fn play_round_retries_same_step_until_http_200() {
    let requested_tile_ids = Arc::new(Mutex::new(Vec::<i32>::new()));
    let requested_tile_ids_for_server = Arc::clone(&requested_tile_ids);
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_server = Arc::clone(&attempts);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/tile-api/me" => ResponseSpec::json(
            200,
            r#"{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":1},"daily_plays_used":{"easy":0},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}"#,
        ),
        "/tile-api/step" => {
            let step: StepRequest = serde_json::from_str(&request.body).unwrap();
            requested_tile_ids_for_server
                .lock()
                .unwrap()
                .push(step.tile_id);
            if attempts_for_server.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseSpec::json(503, r#"{"message":"try later"}"#)
            } else {
                ResponseSpec::json(
                    200,
                    r#"{"action":"click","balance":12.34,"grant_ref":"","history":[],"move_count":1,"ok":true,"powerups":{"remove":0,"shuffle":0,"undo":0},"removed":[9],"reward_amount":0.5,"schema_version":1,"server_now_ms":1777006767000,"session_id":42,"slot_limit":7,"slots":[],"started_at_ms":1777006766000,"status":"won","tiles":[],"total_tiles":0}"#,
                )
            }
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });
    let temp = tempdir().unwrap();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: server.base_url().to_string(),
            accounts: vec![],
        },
        auth_cache_file: None,
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer test-token".to_string(),
    };
    let start = StartResponse {
        difficulty: "easy".to_string(),
        session_id: 42,
        slot_limit: 7,
        status: "pending".to_string(),
        tiles: vec![Tile {
            id: 9,
            pattern: "B".to_string(),
            ..Tile::default()
        }],
        ..StartResponse::default()
    };

    let result = play_round(
        &cancel_flag,
        &state,
        &mut runtime,
        &ConfigResponse {
            min_interval_ms: 0,
            ..ConfigResponse::default()
        },
        &start,
        RoundPlayContext {
            continued: false,
            progress: RoundProgress {
                current: 1,
                total: 1,
            },
            remaining_after: 0,
        },
    )
    .unwrap();

    assert_eq!(*requested_tile_ids.lock().unwrap(), vec![9, 9]);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(result.status, "won");
}

#[test]
fn play_round_retries_step_transport_error_until_http_200() {
    let requested_tile_ids = Arc::new(Mutex::new(Vec::<i32>::new()));
    let requested_tile_ids_for_server = Arc::clone(&requested_tile_ids);
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_server = Arc::clone(&attempts);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/tile-api/me" => ResponseSpec::json(
            200,
            r#"{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":1},"daily_plays_used":{"easy":0},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}"#,
        ),
        "/tile-api/step" => {
            let step: StepRequest = serde_json::from_str(&request.body).unwrap();
            requested_tile_ids_for_server
                .lock()
                .unwrap()
                .push(step.tile_id);
            if attempts_for_server.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseSpec::drop_connection()
            } else {
                ResponseSpec::json(
                    200,
                    r#"{"action":"click","balance":12.34,"grant_ref":"","history":[],"move_count":1,"ok":true,"powerups":{"remove":0,"shuffle":0,"undo":0},"removed":[9],"reward_amount":0.5,"schema_version":1,"server_now_ms":1777006767000,"session_id":42,"slot_limit":7,"slots":[],"started_at_ms":1777006766000,"status":"won","tiles":[],"total_tiles":0}"#,
                )
            }
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });
    let temp = tempdir().unwrap();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: server.base_url().to_string(),
            accounts: vec![],
        },
        auth_cache_file: None,
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer test-token".to_string(),
    };
    let start = StartResponse {
        difficulty: "easy".to_string(),
        session_id: 42,
        slot_limit: 7,
        status: "pending".to_string(),
        tiles: vec![Tile {
            id: 9,
            pattern: "B".to_string(),
            ..Tile::default()
        }],
        ..StartResponse::default()
    };

    let result = play_round(
        &cancel_flag,
        &state,
        &mut runtime,
        &ConfigResponse {
            min_interval_ms: 0,
            ..ConfigResponse::default()
        },
        &start,
        RoundPlayContext {
            continued: false,
            progress: RoundProgress {
                current: 1,
                total: 1,
            },
            remaining_after: 0,
        },
    )
    .unwrap();

    assert_eq!(*requested_tile_ids.lock().unwrap(), vec![9, 9]);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(result.status, "won");
    assert!(result.error_message.is_empty());
}

#[test]
fn play_round_returns_thread_retry_error_after_repeated_step_transport_errors() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_server = Arc::clone(&attempts);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/tile-api/step" => {
            attempts_for_server.fetch_add(1, Ordering::SeqCst);
            ResponseSpec::drop_connection()
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });
    let temp = tempdir().unwrap();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: server.base_url().to_string(),
            accounts: vec![],
        },
        auth_cache_file: None,
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer test-token".to_string(),
    };
    let start = StartResponse {
        difficulty: "easy".to_string(),
        session_id: 42,
        slot_limit: 7,
        status: "pending".to_string(),
        tiles: vec![Tile {
            id: 9,
            pattern: "B".to_string(),
            ..Tile::default()
        }],
        ..StartResponse::default()
    };

    let error = play_round(
        &cancel_flag,
        &state,
        &mut runtime,
        &ConfigResponse {
            min_interval_ms: 0,
            ..ConfigResponse::default()
        },
        &start,
        RoundPlayContext {
            continued: false,
            progress: RoundProgress {
                current: 1,
                total: 1,
            },
            remaining_after: 0,
        },
    )
    .unwrap_err();

    assert_eq!(
        attempts.load(Ordering::SeqCst),
        crate::workflows::common::API_RETRY_MAX_ATTEMPTS
    );
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(error.to_string().contains("卡在第 1 步"));
}

#[test]
fn play_round_stops_when_step_status_is_lost() {
    let requested_tile_ids = Arc::new(Mutex::new(Vec::<i32>::new()));
    let requested_tile_ids_for_server = Arc::clone(&requested_tile_ids);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/tile-api/me" => ResponseSpec::json(
            200,
            r#"{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":0},"daily_plays_used":{"easy":1},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}"#,
        ),
        "/tile-api/step" => {
            let step: StepRequest = serde_json::from_str(&request.body).unwrap();
            requested_tile_ids_for_server
                .lock()
                .unwrap()
                .push(step.tile_id);
            ResponseSpec::json(
                200,
                r#"{"action":"click","balance":12.34,"move_count":1,"ok":true,"reward_amount":0.0,"session_id":42,"slot_limit":7,"slots":[9],"status":"lost","tiles":[{"id":5,"pattern":"B"}],"total_tiles":1}"#,
            )
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });
    let temp = tempdir().unwrap();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: server.base_url().to_string(),
            accounts: vec![],
        },
        auth_cache_file: None,
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer test-token".to_string(),
    };
    let start = StartResponse {
        difficulty: "easy".to_string(),
        session_id: 42,
        slot_limit: 7,
        status: "pending".to_string(),
        tiles: vec![
            Tile {
                id: 9,
                pattern: "A".to_string(),
                ..Tile::default()
            },
            Tile {
                id: 5,
                pattern: "B".to_string(),
                ..Tile::default()
            },
        ],
        ..StartResponse::default()
    };

    let result = play_round(
        &cancel_flag,
        &state,
        &mut runtime,
        &ConfigResponse {
            min_interval_ms: 0,
            ..ConfigResponse::default()
        },
        &start,
        RoundPlayContext {
            continued: false,
            progress: RoundProgress {
                current: 1,
                total: 1,
            },
            remaining_after: 0,
        },
    )
    .unwrap();

    assert_eq!(*requested_tile_ids.lock().unwrap(), vec![9]);
    assert_eq!(result.status, "lost");
    assert!(result.error_message.is_empty());
}

#[test]
fn play_round_reports_http_409_slot_full_as_failure() {
    let requested_tile_ids = Arc::new(Mutex::new(Vec::<i32>::new()));
    let requested_tile_ids_for_server = Arc::clone(&requested_tile_ids);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/tile-api/me" => ResponseSpec::json(
            200,
            r#"{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":1},"daily_plays_used":{"easy":0},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}"#,
        ),
        "/tile-api/step" => {
            let step: StepRequest = serde_json::from_str(&request.body).unwrap();
            requested_tile_ids_for_server
                .lock()
                .unwrap()
                .push(step.tile_id);
            ResponseSpec::json(409, r#"{"message":"slot full"}"#)
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });
    let temp = tempdir().unwrap();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig {
            base_url: server.base_url().to_string(),
            accounts: vec![],
        },
        auth_cache_file: None,
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer test-token".to_string(),
    };
    let start = StartResponse {
        difficulty: "easy".to_string(),
        session_id: 42,
        slot_limit: 7,
        status: "pending".to_string(),
        tiles: vec![Tile {
            id: 9,
            pattern: "B".to_string(),
            ..Tile::default()
        }],
        ..StartResponse::default()
    };

    let result = play_round(
        &cancel_flag,
        &state,
        &mut runtime,
        &ConfigResponse {
            min_interval_ms: 0,
            ..ConfigResponse::default()
        },
        &start,
        RoundPlayContext {
            continued: false,
            progress: RoundProgress {
                current: 1,
                total: 1,
            },
            remaining_after: 0,
        },
    )
    .unwrap();

    assert_eq!(*requested_tile_ids.lock().unwrap(), vec![9]);
    assert_eq!(result.status, "pending");
    assert!(
        result.error_message.contains("slot full") || result.error_message.contains("槽位已满")
    );
}

#[test]
fn ensure_authenticated_reuses_cached_token_across_restart() {
    let temp = tempdir().unwrap();
    let auth_path = temp.path().join("auth.json");
    let login_count = Arc::new(AtomicUsize::new(0));
    let login_count_for_server = Arc::clone(&login_count);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/api/v1/auth/login" => {
            login_count_for_server.fetch_add(1, Ordering::SeqCst);
            ResponseSpec::json(
                200,
                r#"{"code":0,"message":"ok","reason":"","data":{"access_token":"token-login","token_type":"Bearer","user":{"email":"demo@example.com"}}}"#,
            )
        }
        "/api/v1/auth/me" => {
            let auth = request.header("authorization");
            if auth == "Bearer token-login" {
                ResponseSpec::json(
                    200,
                    r#"{"code":0,"message":"ok","reason":"","data":{"email":"demo@example.com","balance":0,"status":"active"}}"#,
                )
            } else {
                ResponseSpec::json(401, r#"{"message":"unauthorized"}"#)
            }
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });

    let first = ApiClient::new(server.base_url());
    let (login_response, auth_token) = first.do_login("demo@example.com", "pw").unwrap();
    assert_eq!(auth_token, "Bearer token-login");
    let account = cache_from_login(&login_response, "demo@example.com", "pw", first.base_url());

    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig::default(),
        auth_cache_file: Some(auth_path),
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account,
        auth_token: String::new(),
    };

    ensure_authenticated(&state, &mut runtime).unwrap();

    assert_eq!(runtime.auth_token, "Bearer token-login");
    assert_eq!(login_count.load(Ordering::SeqCst), 1);
    let session = get_session(&runtime.account, runtime.api_client.base_url()).unwrap();
    assert_eq!(session.token_type, "Bearer");
    assert_eq!(session.access_token, "token-login");
}

#[test]
fn do_login_validates_token_with_auth_me() {
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/api/v1/auth/login" => ResponseSpec::json(
            200,
            r#"{"code":0,"message":"ok","reason":"","data":{"access_token":"token-login","token_type":"Bearer","user":{"email":"demo@example.com"}}}"#,
        ),
        "/api/v1/auth/me" => {
            let auth = request.header("authorization");
            if auth == "Bearer token-login" {
                ResponseSpec::json(
                    200,
                    r#"{"code":0,"message":"ok","reason":"","data":{"email":"demo@example.com","balance":0,"status":"active"}}"#,
                )
            } else {
                ResponseSpec::json(401, r#"{"message":"unauthorized"}"#)
            }
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });

    let client = ApiClient::new(server.base_url());
    let (_login_response, auth_token) = client.do_login("demo@example.com", "pw").unwrap();

    assert_eq!(auth_token, "Bearer token-login");
}

#[test]
fn do_login_allows_standard_auth_me_response() {
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/api/v1/auth/login" => ResponseSpec::json(
            200,
            r#"{"code":0,"message":"ok","reason":"","data":{"access_token":"token-login","token_type":"Bearer","user":{"email":"demo@example.com"}}}"#,
        ),
        "/api/v1/auth/me" => {
            let auth = request.header("authorization");
            if auth == "Bearer token-login" {
                ResponseSpec::json(
                    200,
                    r#"{"code":0,"message":"ok","reason":"","data":{"email":"demo@example.com","balance":0,"status":"active"}}"#,
                )
            } else {
                ResponseSpec::json(401, r#"{"message":"unauthorized"}"#)
            }
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });

    let client = ApiClient::new(server.base_url());
    let (_login_response, auth_token) = client.do_login("demo@example.com", "pw").unwrap();

    assert_eq!(auth_token, "Bearer token-login");
}

#[test]
fn ensure_authenticated_reuses_token_cache() {
    let temp = tempdir().unwrap();
    let auth_path = temp.path().join("auth.json");
    let login_count = Arc::new(AtomicUsize::new(0));
    let login_count_for_server = Arc::clone(&login_count);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/api/v1/auth/login" => {
            login_count_for_server.fetch_add(1, Ordering::SeqCst);
            ResponseSpec::json(
                200,
                r#"{"code":0,"message":"ok","reason":"","data":{"access_token":"fresh-token","token_type":"Bearer","user":{"email":"demo@example.com"}}}"#,
            )
        }
        "/api/v1/auth/me" => {
            let auth = request.header("authorization");
            if auth == "Bearer cached-token" {
                ResponseSpec::json(
                    200,
                    r#"{"code":0,"message":"ok","reason":"","data":{"email":"demo@example.com","balance":0,"status":"active"}}"#,
                )
            } else {
                ResponseSpec::json(401, r#"{"message":"unauthorized"}"#)
            }
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });

    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig::default(),
        auth_cache_file: Some(auth_path),
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            password: "pw".to_string(),
            token_type: "Bearer".to_string(),
            access_token: "cached-token".to_string(),
            ..AuthCache::default()
        },
        auth_token: String::new(),
    };

    ensure_authenticated(&state, &mut runtime).unwrap();

    assert_eq!(login_count.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.auth_token, "Bearer cached-token");
    let session = get_session(&runtime.account, runtime.api_client.base_url()).unwrap();
    assert_eq!(session.access_token, "cached-token");
}

#[test]
fn remaining_plays_uses_tile_me_remaining() {
    let server = TestServer::start(|request| match request.path.as_str() {
        "/tile-api/me" => ResponseSpec::json(
            200,
            r#"{"active_session":null,"authenticated":true,"daily_plays_remaining":{"easy":3,"normal":1},"daily_plays_used":{"easy":7,"normal":9},"server_now_ms":1777006766099,"user":{"balance":12.34,"email":"demo@example.com","id":1,"status":"active"}}"#,
        ),
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });
    let temp = tempdir().unwrap();
    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig::default(),
        auth_cache_file: Some(temp.path().join("auth.json")),
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer token".to_string(),
    };

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let remaining = remaining_plays(&cancel_flag, &state, &mut runtime, "easy").unwrap();

    assert_eq!(remaining, 3);
}

#[test]
fn snapshot_from_step_response_keeps_previous_state_when_fields_are_missing() {
    let previous = SessionSnapshot {
        difficulty: "easy".to_string(),
        session_id: 123,
        slot_limit: 7,
        powerups: Powerups {
            remove: 1,
            shuffle: 2,
            undo: 3,
        },
        status: "pending".to_string(),
        tiles: vec![Tile {
            id: 9,
            pattern: "A".to_string(),
            ..Tile::default()
        }],
        slot_tiles: vec![Tile {
            id: 5,
            pattern: "B".to_string(),
            ..Tile::default()
        }],
        move_count: 2,
    };
    let step: StepResponse = serde_json::from_str(
        r#"{"action":"click","balance":12.34,"move_count":3,"ok":true,"reward_amount":0.5,"status":"won"}"#,
    )
    .unwrap();

    let next = snapshot_from_step_response(&previous, &step);

    assert_eq!(next.status, "won");
    assert_eq!(next.session_id, 123);
    assert_eq!(next.slot_limit, 7);
    assert_eq!(next.powerups.remove, 1);
    assert_eq!(next.slot_tiles.len(), 1);
    assert_eq!(next.slot_tiles[0].id, 5);
    assert_eq!(next.tiles.len(), 1);
    assert_eq!(next.tiles[0].id, 9);
}

#[test]
fn snapshot_from_step_response_allows_explicit_empty_slots() {
    let previous = SessionSnapshot {
        difficulty: "easy".to_string(),
        session_id: 123,
        slot_limit: 7,
        powerups: Powerups::default(),
        status: "pending".to_string(),
        tiles: vec![Tile {
            id: 9,
            pattern: "A".to_string(),
            ..Tile::default()
        }],
        slot_tiles: vec![Tile {
            id: 5,
            pattern: "B".to_string(),
            ..Tile::default()
        }],
        move_count: 2,
    };
    let step: StepResponse = serde_json::from_str(
        r#"{"action":"click","move_count":3,"ok":true,"reward_amount":0.5,"status":"pending","slots":[],"tiles":[]}"#,
    )
    .unwrap();

    let next = snapshot_from_step_response(&previous, &step);

    assert!(next.slot_tiles.is_empty());
    assert!(next.tiles.is_empty());
}

#[test]
fn with_auth_retry_reauthenticates_after_unauthorized_step() {
    let temp = tempdir().unwrap();
    let auth_path = temp.path().join("auth.json");
    let login_count = Arc::new(AtomicUsize::new(0));
    let validate_count = Arc::new(AtomicUsize::new(0));
    let step_count = Arc::new(AtomicUsize::new(0));
    let login_count_for_server = Arc::clone(&login_count);
    let validate_count_for_server = Arc::clone(&validate_count);
    let step_count_for_server = Arc::clone(&step_count);
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/api/v1/auth/me" => {
            let count = validate_count_for_server.fetch_add(1, Ordering::SeqCst) + 1;
            let auth = request.header("authorization");
            let valid = count >= 2 && auth == "Bearer fresh-token-1";
            if valid {
                ResponseSpec::json(
                    200,
                    r#"{"code":0,"message":"ok","reason":"","data":{"email":"demo@example.com","balance":0,"status":"active"}}"#,
                )
            } else {
                ResponseSpec::json(401, r#"{"message":"unauthorized"}"#)
            }
        }
        "/api/v1/auth/login" => {
            let count = login_count_for_server.fetch_add(1, Ordering::SeqCst) + 1;
            ResponseSpec::json(
                200,
                &format!(
                    "{{\"code\":0,\"message\":\"ok\",\"reason\":\"\",\"data\":{{\"access_token\":\"fresh-token-{count}\",\"token_type\":\"Bearer\",\"user\":{{\"email\":\"demo@example.com\"}}}}}}"
                ),
            )
        }
        "/tile-api/step" => {
            if step_count_for_server.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseSpec::json(401, r#"{"message":"unauthorized"}"#)
            } else {
                ResponseSpec::json(
                    200,
                    r#"{"action":"click","balance":12.34,"grant_ref":"","history":[],"move_count":3,"ok":true,"powerups":{"remove":0,"shuffle":0,"undo":0},"removed":[1,2,3],"reward_amount":0.5,"schema_version":1,"server_now_ms":1777006767000,"session_id":123,"slot_limit":7,"slots":[],"started_at_ms":1777006766000,"status":"won","tiles":[],"total_tiles":0}"#,
                )
            }
        }
        _ => ResponseSpec::json(404, r#"{"message":"not found"}"#),
    });

    let state = Arc::new(Mutex::new(BatchState {
        config: AuthConfig::default(),
        auth_cache_file: Some(auth_path.clone()),
        result_log_dir: temp.path().join("log"),
        log: crate::ui::TaskLog::stdout(),
    }));
    let mut runtime = AccountRuntime {
        api_client: ApiClient::new(server.base_url()),
        account: AuthCache {
            email: "demo@example.com".to_string(),
            password: "pw".to_string(),
            token_type: "Bearer".to_string(),
            access_token: "stale-token".to_string(),
            ..AuthCache::default()
        },
        auth_token: "Bearer stale-token".to_string(),
    };

    let step = with_auth_retry(&state, &mut runtime, |client, auth_token| {
        client.step(
            auth_token,
            StepRequest {
                session_id: 123,
                action: "click".to_string(),
                tile_id: 1,
            },
        )
    })
    .unwrap();

    assert_eq!(step.status, "won");
    assert_eq!(login_count.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.auth_token, "Bearer fresh-token-1");
    let session = get_session(&runtime.account, runtime.api_client.base_url()).unwrap();
    assert_eq!(session.access_token, "fresh-token-1");
}

struct TestServer {
    base_url: String,
    shutdown: Option<std::sync::mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn start<F>(handler: F) -> Self
    where
        F: Fn(TestRequest) -> ResponseSpec + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let base_url = format!("http://{}", address);
        let handler = Arc::new(handler);
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let handler = Arc::clone(&handler);
                        handle_connection(&mut stream, handler);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base_url,
            shutdown: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Debug, Clone)]
struct TestRequest {
    path: String,
    headers: std::collections::HashMap<String, String>,
    body: String,
}

impl TestRequest {
    fn header(&self, name: &str) -> String {
        self.headers
            .get(&name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
struct ResponseSpec {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
    close_without_response: bool,
}

impl ResponseSpec {
    fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: body.to_string(),
            close_without_response: false,
        }
    }

    fn drop_connection() -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: String::new(),
            close_without_response: true,
        }
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    handler: Arc<dyn Fn(TestRequest) -> ResponseSpec + Send + Sync>,
) {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let header_len = header_end + 4;
                    let header_text = String::from_utf8_lossy(&buffer[..header_len]);
                    let content_length = header_text
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            if name.trim().eq_ignore_ascii_case("content-length") {
                                value.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    while buffer.len() < header_len + content_length {
                        match stream.read(&mut chunk) {
                            Ok(0) => break,
                            Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                            Err(_) => return,
                        }
                    }
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            Err(_) => return,
        }
    }
    let header_end = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .unwrap_or(buffer.len());
    let request_text = String::from_utf8_lossy(&buffer[..header_end]);
    let body = String::from_utf8_lossy(&buffer[header_end..]).to_string();
    let mut lines = request_text.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let _method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();
    let mut headers = std::collections::HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let response = handler(TestRequest {
        path,
        headers,
        body,
    });
    if response.close_without_response {
        return;
    }
    let status_text = match response.status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "OK",
    };
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        status_text,
        response.body.len()
    );
    for (name, value) in response.headers {
        head.push_str(&format!("{}: {}\r\n", name, value));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(response.body.as_bytes());
    let _ = stream.flush();
}
