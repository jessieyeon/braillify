#[cfg(feature = "cli")]
use std::env;

#[cfg(feature = "cli")]
use anyhow::Result;
#[cfg(feature = "cli")]
use braillify::cli::run_cli;

#[cfg(feature = "cli")]
fn main() -> Result<()> {
    run_cli(env::args().collect())
}

#[cfg(all(test, feature = "cli"))]
mod tests {
    use std::io::Write;
    use std::sync::OnceLock;

    use assert_cmd::assert::OutputAssertExt;
    use predicates::prelude::*;

    // 빌드를 한 번만 수행하고 재사용
    static BUILT_BINARY: OnceLock<escargot::CargoRun> = OnceLock::new();

    fn get_built_binary() -> &'static escargot::CargoRun {
        BUILT_BINARY.get_or_init(|| {
            // 재시도 로직: 첫 번째 테스트에서 빌드가 실패할 수 있으므로 재시도
            let mut last_error = None;
            for attempt in 1..=3 {
                match escargot::CargoBuild::new()
                    .bin("braillify")
                    .current_release()
                    .current_target()
                    .run()
                {
                    Ok(built) => return built,
                    Err(e) => {
                        last_error = Some(e);
                        if attempt < 3 {
                            // 재시도 전에 짧은 대기 시간
                            std::thread::sleep(std::time::Duration::from_millis(
                                200 * attempt as u64,
                            ));
                        }
                    }
                }
            }
            panic!(
                "Failed to build braillify binary for testing after 3 attempts. \
                Last error: {:?}. \
                This may happen on the first test run. \
                Try running 'cargo build --bin braillify' manually first.",
                last_error
            );
        })
    }

    // assert_cmd를 사용한 통합 테스트들
    #[test]
    fn test_braillify_integration_single_word() {
        let mut cmd = get_built_binary().command();
        cmd.arg("안녕");
        let assert = cmd
            .assert()
            .success()
            .stdout(predicate::str::is_empty().not());

        // 점자 유니코드가 포함되어 있는지 확인
        let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
        assert!(
            stdout
                .chars()
                .any(|c| c as u32 >= 0x2800 && c as u32 <= 0x28FF)
        );
    }

    #[test]
    fn test_braillify_integration_english() {
        let mut cmd = get_built_binary().command();
        cmd.arg("hello");
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }

    #[test]
    fn test_braillify_integration_mixed() {
        let mut cmd = get_built_binary().command();
        cmd.arg("안녕 hello");
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }

    #[test]
    fn test_braillify_integration_numbers() {
        let mut cmd = get_built_binary().command();
        cmd.arg("123");
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }

    #[test]
    fn test_braillify_pipe_input() {
        let mut cmd = get_built_binary().command();
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all("안녕\n".as_bytes()).unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
    }

    #[test]
    fn test_braillify_help() {
        let mut cmd = get_built_binary().command();
        cmd.arg("--help");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("한국어 점자 변환 CLI"));
    }

    #[test]
    fn test_braillify_version() {
        let mut cmd = get_built_binary().command();
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("braillify"));
    }

    #[test]
    fn test_braillify_no_args() {
        let mut cmd = get_built_binary().command();
        // 인자 없이 실행하면 REPL 모드로 진입
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        {
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all("안녕\n".as_bytes()).unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("braillify REPL"));
    }

    #[test]
    fn test_braillify_empty_input() {
        let mut cmd = get_built_binary().command();
        cmd.arg("");
        cmd.assert().success().stdout(predicate::str::is_empty());
    }

    #[test]
    fn test_braillify_long_text() {
        let long_text = "안녕하세요 ".repeat(100);
        let mut cmd = get_built_binary().command();
        cmd.arg(&long_text);
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }

    #[test]
    fn test_braillify_special_characters() {
        let mut cmd = get_built_binary().command();
        cmd.arg("!@#$%^&*()");
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }

    #[test]
    fn test_braillify_korean_sentences() {
        let mut cmd = get_built_binary().command();
        cmd.arg("안녕하세요. 오늘 날씨가 좋네요.");
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }

    #[test]
    fn test_braillify_multiple_spaces() {
        let mut cmd = get_built_binary().command();
        cmd.arg("안녕    하세요");
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }

    #[test]
    fn test_braillify_newlines() {
        let mut cmd = get_built_binary().command();
        cmd.arg("안녕\n하세요");
        cmd.assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }
}
