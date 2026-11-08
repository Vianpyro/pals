//! A command run in a gVisor container through the Docker CLI, argv only (ADR-0002, ADR-0018)

use std::{
    io,
    process::{ExitStatus, Stdio},
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy, sink},
    process::Command,
    time, try_join,
};

use crate::{Pack, Submission};

/// gVisor, as registered with Docker
const RUNTIME: &str = "runsc";
const MOUNT: &str = "/submission";

// Doesn't depend on the image's `/etc/passwd`
const SANDBOX_USER: u32 = 65534;

// Fixed limits
const MEMORY: u64 = 256 * 1024 * 1024; // 256 MiB
const CPUS: &str = "1";
const PIDS: &str = "64";

// Drop long outputs (>16 MiB)
pub(super) const MAX_OUTPUT: u64 = 16 * 1024 * 1024;

/// Codes `docker run` exists with when Docker fails
const DOCKER_CODES: [i32; 3] = [125, 126, 127];

/// `.State.StartedAt` of a container that never started (Go zero value for `time.Time`)
const NEVER_STARTED: &str = "0001-01-01T00:00:00Z";

#[derive(Debug)]
pub enum Outcome {
    Exited { status: ExitStatus, stdout: Vec<u8> },
    TimedOut,
}

pub async fn run_in_sandbox(
    name: &str,
    pack: &Pack,
    submission: &Submission,
    input: &[u8],
    timeout: Duration,
) -> io::Result<Outcome> {
    remove(name).await?; // Left by a judge killed mid-case: its name would conflict
    let mut child = container(name, pack, submission)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("stdin is not piped"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("stdout is not piped"))?;
    let finished = time::timeout(timeout, async {
        let feed = async move {
            match stdin.write_all(input).await {
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                result => result,
            }
        };
        let read = async {
            let mut kept = Vec::new();
            (&mut stdout)
                .take(MAX_OUTPUT)
                .read_to_end(&mut kept)
                .await?;
            copy(&mut stdout, &mut sink()).await?;
            Ok::<_, io::Error>(kept)
        };
        let ((), kept) = try_join!(feed, read)?;
        Ok::<_, io::Error>((child.wait().await?, kept))
    })
    .await;

    match finished {
        Ok(Ok((status, stdout))) => {
            let started = ensure_started(name, status).await;
            remove(name).await?;
            started.map(|()| Outcome::Exited { status, stdout })
        }
        Ok(Err(error)) => {
            remove(name).await?;
            Err(error)
        }
        Err(_) => {
            remove(name).await?;
            Ok(Outcome::TimedOut)
        }
    }
}

fn container(name: &str, pack: &Pack, submission: &Submission) -> Command {
    let memory = MEMORY.to_string();
    let mut docker = Command::new("docker");
    docker
        .args(["run", "--interactive"])
        .args(["--name", name, "--runtime", RUNTIME])
        .args(["--pull", "never"])
        .args(["--log-driver", "none", "--network", "none"]) // Docker would keep the program's output on the host and logs would be lost otherwise
        .arg("--read-only")
        .args(["--memory", &memory]) // Hard limit on RAM available to the sandbox
        .args(["--memory-swap", &memory]) // No additional swap beyond the RAM limit
        .args(["--cpus", CPUS]) // Limits CPU time so CPU-bound programs cannot monopolize the whole CPU
        .args(["--pids-limit", PIDS]) // Prevents fork/thread bombs
        .args(["--cap-drop", "ALL", "--security-opt", "no-new-privileges"])
        .arg("--user")
        .arg(format!("{SANDBOX_USER}:{SANDBOX_USER}"))
        .arg("--mount")
        .arg(format!(
            "type=bind,src={},dst={MOUNT},readonly",
            submission.dir().display()
        ))
        .args(["--workdir", MOUNT])
        .arg(&pack.image)
        .args(&pack.run);
    docker
}

/// Err when Docker exited with one of its own codes without ever starting the program
async fn ensure_started(name: &str, status: ExitStatus) -> io::Result<()> {
    if !status
        .code()
        .is_some_and(|code| DOCKER_CODES.contains(&code))
    {
        return Ok(());
    }

    let inspect = Command::new("docker")
        .args(["container", "inspect", "--format"])
        .arg("{{.State.StartedAt}}\t{{.State.Error}}")
        .arg(name)
        .stderr(Stdio::null())
        .output()
        .await?;
    if !inspect.status.success() {
        return Err(io::Error::other(format!(
            "{name}: docker exited with {status} before creating the container"
        )));
    }

    let state = String::from_utf8_lossy(&inspect.stdout);
    match state.trim_end_matches('\n').split_once('\t') {
        Some((NEVER_STARTED, error)) => Err(io::Error::other(format!("{name}: {error}"))),
        Some(_) => Ok(()),
        None => Err(io::Error::other(format!(
            "{name}: unexpected state {state}"
        ))),
    }
}

async fn remove(name: &str) -> io::Result<()> {
    Command::new("docker")
        .args(["rm", "--force", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_packs;
    use std::{assert_matches, path::Path};

    const START: Duration = Duration::from_mins(1);
    const DEADLINE: Duration = Duration::from_secs(5);

    fn test_container(name: &str) -> String {
        format!("judge-test-{name}")
    }

    // The deployed pack: its pinned image is the one the judge runs
    fn python_pack() -> Pack {
        let packs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/languages");
        let mut packs = read_packs(&packs).unwrap();
        packs.remove(&"python".parse().unwrap()).unwrap()
    }

    async fn sandboxed(
        name: &str,
        pack: &Pack,
        code: &str,
        input: &[u8],
        timeout: Duration,
    ) -> io::Result<Outcome> {
        let name = test_container(name);
        let submission = Submission::write(&name, &pack.source, code).unwrap();
        run_in_sandbox(&name, pack, &submission, input, timeout).await
    }

    async fn python(name: &str, code: &str, input: &[u8], timeout: Duration) -> Outcome {
        sandboxed(name, &python_pack(), code, input, timeout)
            .await
            .unwrap()
    }

    // A pack Docker cannot start: the code never runs
    async fn broken(name: &str, pack: Pack) -> io::Result<Outcome> {
        sandboxed(name, &pack, "print(1)", b"", START).await
    }

    async fn exists(name: &str) -> bool {
        Command::new("docker")
            .args(["container", "inspect"])
            .arg(test_container(name))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap()
            .success()
    }

    #[track_caller]
    fn assert_failed(outcome: Outcome) {
        assert_matches!(outcome, Outcome::Exited { status, .. } if !status.success());
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn command_runs_as_nobody() {
        let ids = format!("{SANDBOX_USER} {SANDBOX_USER}\n");
        assert_matches!(
            python("nobody", "import os; print(os.getuid(), os.getgid())", b"", START).await,
            Outcome::Exited { status, stdout } if status.success() && stdout == ids.as_bytes()
        );
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn network_is_unreachable() {
        let connect = "import socket; socket.create_connection(('1.1.1.1', 53), timeout=5)";
        assert_failed(python("network", connect, b"", START).await);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn filesystem_is_read_only() {
        let write = "open('/var/tmp/escape', 'w')";
        assert_failed(python("read-only", write, b"", START).await);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn submission_is_mounted_read_only() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let name = test_container("read-only-mount");
        let pack = python_pack();
        let submission = Submission::write(&name, &pack.source, "open('written', 'w')").unwrap();
        fs::set_permissions(submission.dir(), fs::Permissions::from_mode(0o777)).unwrap();
        let outcome = run_in_sandbox(&name, &pack, &submission, b"", START)
            .await
            .unwrap();
        assert_failed(outcome);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn memory_is_limited() {
        let flood = format!("b'A' * {}", 2 * MEMORY);
        assert_failed(python("memory", &flood, b"", START).await);
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn stdout_is_bounded() {
        let flood = format!("import sys; sys.stdout.write('A' * {})", 2 * MAX_OUTPUT);
        assert_matches!(
            python("flood", &flood, b"", START).await,
            Outcome::Exited { status, stdout } if status.success() && stdout.len() as u64 == MAX_OUTPUT
        );
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn endless_command_is_cleaned_up() {
        let outcome = python("endless", "while True: pass", b"", DEADLINE).await;
        assert_matches!(outcome, Outcome::TimedOut);
        assert!(!exists("endless").await, "the container is still there");
    }

    // Docker uses these codes for its own failures: the program may still exit with them
    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn docker_codes_from_the_program_are_its_own() {
        for code in [125, 126, 127] {
            let name = format!("exit-{code}");
            let exit = format!("import sys; sys.exit({code})");
            assert_matches!(
                python(&name, &exit, b"", START).await,
                Outcome::Exited { status, .. } if status.code() == Some(code)
            );
            assert!(!exists(&name).await, "the container is still there");
        }
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn missing_image_is_an_error() {
        let pack = Pack {
            image: "judge-test/missing:never".into(),
            ..python_pack()
        };
        assert!(broken("missing-image", pack).await.is_err());
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn missing_command_is_an_error() {
        let pack = Pack {
            run: vec!["missing-command".into()],
            ..python_pack()
        };
        assert!(broken("missing-command", pack).await.is_err());
        assert!(
            !exists("missing-command").await,
            "the container is still there"
        );
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn non_executable_command_is_an_error() {
        let pack = Pack {
            run: vec!["/etc/passwd".into()],
            ..python_pack()
        };
        assert!(broken("non-executable", pack).await.is_err());
        assert!(
            !exists("non-executable").await,
            "the container is still there"
        );
    }

    // Left by a judge killed mid-case: its name would conflict and Docker would exit with 125
    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn leftover_container_is_replaced() {
        remove(&test_container("leftover")).await.unwrap(); // left by a previous failed run
        let created = Command::new("docker")
            .args(["create", "--name"])
            .arg(test_container("leftover"))
            .args([&python_pack().image, "true"])
            .stdout(Stdio::null())
            .status()
            .await
            .unwrap();
        assert!(created.success());
        assert_matches!(
            python("leftover", "print(1)", b"", START).await,
            Outcome::Exited { status, stdout } if status.success() && stdout == b"1\n"
        );
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn input_reaches_stdin() {
        let sum = "import sys; print(sum(map(int, sys.stdin.read().split())))";
        assert_matches!(
            python("stdin", sum, b"3 5\n", START).await,
            Outcome::Exited { status, stdout } if status.success() && stdout == b"8\n"
        );
    }

    #[tokio::test]
    #[ignore = "needs Docker with runsc"]
    async fn unread_input_is_not_an_error() {
        let input = vec![b'A'; 2 * MAX_OUTPUT as usize];
        assert_matches!(
            python("unread", "pass", &input, START).await,
            Outcome::Exited { status, .. } if status.success()
        );
    }
}
