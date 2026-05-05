use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use crate::model::AuthConfig;
use crate::storage::upsert_account;
use crate::ui;

pub const TASK_TITLE: &str = "全自动运行所有无次数限制的白嫖玩法";
pub const DONE_MESSAGE: &str = "全自动运行所有无次数限制的白嫖玩法已停止。";

pub(crate) type UnlimitedFeatureRunner = Arc<
    dyn Fn(AuthConfig, &Path, &ui::CancelFlag, &ui::TaskLog) -> io::Result<AuthConfig>
        + Send
        + Sync,
>;
pub(crate) type SaveMergedConfig = Box<dyn Fn(AuthConfig) -> io::Result<()> + Send>;

#[derive(Clone)]
pub(crate) struct UnlimitedFreeFeature {
    pub(crate) title: &'static str,
    pub(crate) run: UnlimitedFeatureRunner,
}

pub(crate) struct UnlimitedFreeFeatureRunners {
    pub(crate) features: Vec<UnlimitedFreeFeature>,
    pub(crate) save_merged_config: SaveMergedConfig,
}

pub(crate) fn execute_all_unlimited_free_features(
    original_config: AuthConfig,
    auth_cache_file: impl AsRef<Path>,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
    runners: UnlimitedFreeFeatureRunners,
) -> io::Result<AuthConfig> {
    let features = runners.features;
    if features.is_empty() {
        return Ok(original_config);
    }

    log.line_fmt(format_args!(
        "开始{}，本次会处理 {} 个账号；无次数限制的白嫖玩法会持续运行，按 ESC 停止。",
        TASK_TITLE,
        original_config.accounts.len()
    ));

    let auth_cache_file = auth_cache_file.as_ref().to_path_buf();
    let (result_tx, result_rx) = mpsc::channel::<io::Result<AuthConfig>>();
    let mut handles = Vec::with_capacity(features.len());
    for feature in features {
        ui::check_cancel(cancel_flag)?;
        let feature_config = original_config.clone();
        let feature_auth_path = auth_cache_file.clone();
        let feature_cancel = Arc::clone(cancel_flag);
        let feature_log = log.prefixed(format!("【{}】", feature.title));
        let result_tx = result_tx.clone();
        handles.push(std::thread::spawn(move || {
            let result = run_unlimited_feature(
                feature,
                feature_config,
                feature_auth_path,
                &feature_cancel,
                &feature_log,
            );
            let _ = result_tx.send(result);
        }));
    }
    drop(result_tx);

    let mut merged_config = original_config.clone();
    let mut first_error = None;
    for _ in 0..handles.len() {
        match result_rx.recv() {
            Ok(Ok(feature_config)) => {
                merged_config = merge_config_accounts(merged_config, feature_config);
            }
            Ok(Err(error)) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            Err(_) => {
                if first_error.is_none() {
                    first_error = Some(io::Error::other(
                        "全自动无次数限制白嫖任务提前结束，请查看前面的项目日志了解原因。",
                    ));
                }
                break;
            }
        }
    }

    for handle in handles {
        if handle.join().is_err() && first_error.is_none() {
            first_error = Some(io::Error::other(
                "全自动无次数限制白嫖任务异常退出，请查看前面的项目日志了解原因。",
            ));
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    (runners.save_merged_config)(merged_config.clone())?;
    Ok(merged_config)
}

fn run_unlimited_feature(
    feature: UnlimitedFreeFeature,
    config: AuthConfig,
    auth_cache_file: PathBuf,
    cancel_flag: &ui::CancelFlag,
    log: &ui::TaskLog,
) -> io::Result<AuthConfig> {
    log.line("开始运行。");
    let result = (feature.run)(config, &auth_cache_file, cancel_flag, log);
    match &result {
        Ok(_) => log.line("运行结束。"),
        Err(error) if error.kind() == io::ErrorKind::Interrupted => log.line("已停止。"),
        Err(error) => log.line_fmt(format_args!("运行失败：{}", error)),
    }
    result
}

fn merge_config_accounts(mut base: AuthConfig, update: AuthConfig) -> AuthConfig {
    for account in update.accounts {
        base = upsert_account(base, account);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AuthCache;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn sample_config(emails: &[&str]) -> AuthConfig {
        AuthConfig {
            base_url: "http://example.com".to_string(),
            accounts: emails
                .iter()
                .map(|email| AuthCache {
                    email: (*email).to_string(),
                    ..AuthCache::default()
                })
                .collect(),
        }
    }

    #[test]
    fn execute_all_unlimited_features_uses_runner_registry_and_saves_once() {
        let config = sample_config(&["alpha@example.com"]);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let saved_configs = Arc::new(Mutex::new(Vec::<AuthConfig>::new()));

        let result = execute_all_unlimited_free_features(
            config,
            Path::new("auth.json"),
            &cancel_flag,
            &ui::TaskLog::stdout(),
            UnlimitedFreeFeatureRunners {
                features: vec![UnlimitedFreeFeature {
                    title: "自动箭头逃离",
                    run: Arc::new({
                        let events = Arc::clone(&events);
                        move |mut config, auth_path, _cancel_flag, _log| {
                            events
                                .lock()
                                .unwrap()
                                .push(format!("arrow_out:{}", auth_path.display()));
                            config.accounts[0].access_token = "after-arrow-out".to_string();
                            Ok(config)
                        }
                    }),
                }],
                save_merged_config: Box::new({
                    let saved_configs = Arc::clone(&saved_configs);
                    move |merged_config| {
                        saved_configs.lock().unwrap().push(merged_config);
                        Ok(())
                    }
                }),
            },
        )
        .unwrap();

        assert_eq!(result.accounts[0].access_token, "after-arrow-out");
        assert_eq!(events.lock().unwrap().as_slice(), ["arrow_out:auth.json"]);
        let saved_configs = saved_configs.lock().unwrap();
        assert_eq!(saved_configs.len(), 1);
        assert_eq!(saved_configs[0], result);
    }

    #[test]
    fn execute_all_unlimited_features_propagates_cancel() {
        let config = sample_config(&["alpha@example.com"]);
        let cancel_flag = Arc::new(AtomicBool::new(true));
        let save_called = Arc::new(AtomicBool::new(false));

        let error = execute_all_unlimited_free_features(
            config,
            Path::new("auth.json"),
            &cancel_flag,
            &ui::TaskLog::stdout(),
            UnlimitedFreeFeatureRunners {
                features: vec![UnlimitedFreeFeature {
                    title: "自动箭头逃离",
                    run: Arc::new(|config, _auth_path, _cancel_flag, _log| Ok(config)),
                }],
                save_merged_config: Box::new({
                    let save_called = Arc::clone(&save_called);
                    move |_merged_config| {
                        save_called.store(true, Ordering::SeqCst);
                        Ok(())
                    }
                }),
            },
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert!(!save_called.load(Ordering::SeqCst));
    }
}
