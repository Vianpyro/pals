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
        Ok(Ok((status, stdout))) => Ok(Outcome::Exited { status, stdout }),
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
        .args(["run", "--rm", "--interactive"])
        .args(["--name", name, "--runtime", RUNTIME])
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
    use std::assert_matches;

    const START: Duration = Duration::from_mins(1);
    const DEADLINE: Duration = Duration::from_secs(5);

    fn test_container(name: &str) -> String {
        format!("judge-test-{name}")
    }

    fn python_pack() -> Pack {
        Pack {
            image: "python:3.15-alpine".into(),
            source: "main.py".into(),
            run: vec!["python3".into(), "main.py".into()],
        }
    }

    async fn python(name: &str, code: &str, input: &[u8], timeout: Duration) -> Outcome {
        let name = test_container(name);
        let pack = python_pack();
        let submission = Submission::write(&name, &pack.source, code).unwrap();
        run_in_sandbox(&name, &pack, &submission, input, timeout)
            .await
            .unwrap()
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
    async fn endless_command_is_stopped_and_removed() {
        let outcome = python("endless", "while True: pass", b"", DEADLINE).await;
        assert_matches!(outcome, Outcome::TimedOut);
        let inspect = Command::new("docker")
            .args(["container", "inspect"])
            .arg(test_container("endless"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap();
        assert!(!inspect.success(), "the container is still there");
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
