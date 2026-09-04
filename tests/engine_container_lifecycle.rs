//! An engine container a run abandoned is found and removed by the next run.
//!
//! The exit slot drops the container on every ordinary exit; a terminating
//! signal runs no exit code at all. So every engine container carries a label
//! and a start stamp, and a start sweeps any stamped more than two hours ago.
//! Age is what keeps a concurrent binary's container safe.

#[path = "support/containers.rs"]
mod containers;
#[path = "support/exit_slot.rs"]
mod exit_slot;

use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::SyncRunner,
};

fn docker(args: &[&str]) -> std::process::Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("run docker")
}

fn container_exists(id: &str) -> bool {
    docker(&["inspect", "--format", "{{.Id}}", id])
        .status
        .success()
}

/// The anonymous volumes the image's `VOLUME` declaration gave this container.
fn volumes_of(id: &str) -> Vec<String> {
    let out = docker(&[
        "inspect",
        "--format",
        "{{range .Mounts}}{{if eq .Type \"volume\"}}{{.Name}} {{end}}{{end}}",
        id,
    ]);
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn volume_exists(name: &str) -> bool {
    docker(&["volume", "inspect", name]).status.success()
}

/// Test cleanup: `-v`, since `docker rm` alone strands the anonymous volume.
fn remove(ids: &[&str]) {
    let mut args = vec!["rm", "-f", "-v"];
    args.extend_from_slice(ids);
    let _ = docker(&args);
}

fn create(started: &str) -> String {
    let out = docker(&[
        "create",
        "--label",
        &format!("{}=postgres", containers::CONTAINER_LABEL),
        "--label",
        &format!("{}={started}", containers::STARTED_LABEL),
        "postgres:16.15-alpine",
    ]);
    assert!(
        out.status.success(),
        "docker create: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn sweep_removes_abandoned_containers_and_keeps_live_ones() {
    let abandoned = create("0");
    let live = create(&containers::now_secs().to_string());
    let abandoned_volumes = volumes_of(&abandoned);
    assert!(
        !abandoned_volumes.is_empty(),
        "the image declares a volume; none was created"
    );

    containers::sweep_abandoned();

    let live_survived = container_exists(&live);
    let abandoned_survived = container_exists(&abandoned);
    let stranded: Vec<&String> = abandoned_volumes
        .iter()
        .filter(|v| volume_exists(v))
        .collect();
    remove(&[&live, &abandoned]);
    assert!(
        stranded.is_empty(),
        "sweep left the swept container's volumes behind: {stranded:?}"
    );
    assert!(
        !abandoned_survived,
        "a container stamped at the epoch was not swept"
    );
    assert!(
        live_survived,
        "a container stamped just now was swept: a sibling run would lose its server"
    );
}

static PROBE: exit_slot::ExitSlot<testcontainers::Container<GenericImage>> =
    exit_slot::ExitSlot::new();

const PROBE_WAIT_ENV: &str = "ENGINE_PROBE_WAIT";

/// Runs in a child process: starts a labelled container the way the engine
/// harnesses do, prints its ID and volumes, then either returns, so the
/// binary exits normally, or waits to be killed.
#[test]
#[ignore = "child probe for the lifecycle tests"]
fn probe_starts_labelled_container() {
    let id = PROBE.with(
        || {
            GenericImage::new("postgres", "16.15-alpine")
                .with_exposed_port(5432.tcp())
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ))
                .with_env_var("POSTGRES_PASSWORD", "postgres")
                .with_labels(containers::labels("postgres"))
                .start()
                .expect("start postgres 16.15-alpine")
        },
        |container| container.id().to_owned(),
    );
    println!("container-id={id}");
    println!("volumes={}", volumes_of(&id).join(","));
    // Piped stdout is block-buffered: without this the parent waits on the
    // ID line while the probe sleeps.
    let _ = std::io::stdout().flush();
    if std::env::var_os(PROBE_WAIT_ENV).is_some() {
        thread::sleep(Duration::from_secs(120));
    }
}

/// Removes a container and named volumes when dropped, so a failed assertion
/// between "ID known" and "explicit cleanup" cannot strand what the suite
/// exists to prevent. `Drop` is infallible: results are ignored.
struct Cleanup {
    container: String,
    volumes: Vec<String>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", "-v", &self.container])
            .output();
        for v in &self.volumes {
            let _ = Command::new("docker")
                .args(["volume", "rm", "-f", v])
                .output();
        }
    }
}

#[test]
fn a_normal_exit_removes_the_container_and_its_volumes() {
    let output = Command::new(std::env::current_exe().expect("own path"))
        .args([
            "--ignored",
            "--exact",
            "probe_starts_labelled_container",
            "--nocapture",
        ])
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "probe: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let id = stdout
        .lines()
        .find_map(|l| l.strip_prefix("container-id="))
        .expect("container id");
    let volumes: Vec<String> = stdout
        .lines()
        .find_map(|l| l.strip_prefix("volumes="))
        .expect("volume list")
        .split(',')
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect();
    let cleanup = Cleanup {
        container: id.to_owned(),
        volumes,
    };
    assert!(
        !cleanup.volumes.is_empty(),
        "the image declares a volume; the probe saw none"
    );
    assert!(
        !container_exists(id),
        "container {id} survived a normal exit"
    );
    let stranded: Vec<&String> = cleanup
        .volumes
        .iter()
        .filter(|v| volume_exists(v))
        .collect();
    assert!(
        stranded.is_empty(),
        "volumes survived a normal exit: {stranded:?}"
    );
}

#[test]
fn a_killed_run_leaves_a_container_the_sweep_can_identify() {
    let mut child = Command::new(std::env::current_exe().expect("own path"))
        .args([
            "--ignored",
            "--exact",
            "probe_starts_labelled_container",
            "--nocapture",
        ])
        .env(PROBE_WAIT_ENV, "1")
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn the probe");
    let stdout = child.stdout.take().expect("piped stdout");
    let id = BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .find_map(|line| line.strip_prefix("container-id=").map(str::to_owned))
        .expect("the probe printed its container id");
    let _cleanup = Cleanup {
        container: id.clone(),
        volumes: Vec::new(),
    };
    child.kill().expect("SIGKILL the probe");
    let _ = child.wait();

    let survived = container_exists(&id);
    let labels = docker(&[
        "inspect",
        "--format",
        &format!(
            "{{{{index .Config.Labels \"{}\"}}}} {{{{index .Config.Labels \"{}\"}}}}",
            containers::CONTAINER_LABEL,
            containers::STARTED_LABEL
        ),
        &id,
    ]);
    assert!(
        survived,
        "a SIGKILLed run should leave its container; nothing ran to remove it"
    );
    let labels = String::from_utf8_lossy(&labels.stdout);
    let (role, stamp) = labels.trim().split_once(' ').expect("two labels");
    assert_eq!(role, "postgres");
    let stamp: u64 = stamp.parse().expect("a numeric start stamp");
    assert!(
        containers::now_secs() - stamp < 600,
        "stamp {stamp} is not recent"
    );
}
