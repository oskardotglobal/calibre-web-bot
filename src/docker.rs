use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecResults};
use serenity::futures::StreamExt;

pub async fn execute_command_for_container(container_name: &str, docker_client: &Docker, cmd: Option<Vec<&str>>) {
    let id = docker_client
        .inspect_container(container_name, None)
        .await
        .expect("Couldn't inspect container")
        .id
        .expect("Couldn't get container id");

    let exec = docker_client
        .create_exec(
            &id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd,
                ..Default::default()
            },
        )
        .await
        .expect("Couldn't execute command")
        .id;

    if let Ok(StartExecResults::Attached { mut output, .. }) = docker_client.start_exec(&exec, None).await {
        while let Some(Ok(msg)) = output.next().await {
            print!("{}", msg);
        }
    } else {
        unreachable!();
    }
}