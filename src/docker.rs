use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecResults};
use serenity::futures::StreamExt;
use crate::errors;
use crate::errors::ExecCommandForContainerError;

pub(crate) async fn execute_command_for_container(container_name: &str, docker_client: &Docker, cmd: Option<Vec<&str>>) -> Result<String, impl std::error::Error> {
    let id = match docker_client
        .inspect_container(container_name, None)
        .await
        .map_err(ExecCommandForContainerError::BollardError)
    {
        Ok(inspect) => {
            match inspect.id {
                Some(id) => id,
                None => {
                    let name = container_name.clone().to_owned();
                    return Err(ExecCommandForContainerError::Error(errors::Error::DockerContainerNotFound { container_name: name }));
                }
            }
        }
        Err(e) => return Err(e),
    };

    let exec = match docker_client
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
        .map_err(ExecCommandForContainerError::BollardError)
    {
        Ok(exec) => exec.id,
        Err(e) => return Err(e)
    };

    match docker_client.start_exec(&exec, None)
        .await
        .expect("Couldn't start execution")
    {
        StartExecResults::Attached { mut output, .. } => {
            let mut full_output = "".to_owned();

            while let Some(Ok(msg)) = output.next().await {
                full_output += msg.to_string().as_str();
            }

            info!("{}", full_output);
            return Ok(full_output.to_owned());
        }
        StartExecResults::Detached => panic!("Couldn't attach to container")
    };
}