#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Docker::inspect_container didn't return id for container {container_name}, does the container exist?")]
    DockerContainerNotFound { container_name: String },

    #[error("crate::util::find_book couldn't process the request to {url}")]
    RequestFailed { url: String },

    #[error("crate::util::find_book couldn't parse the URL {url}")]
    InvalidUrl { url: String },

    #[error("crate::util::find_book couldn't find any books matching the query {query}")]
    NoResults { query: String },

    #[error("crate::util::find_book couldn't parse the page at {url}")]
    ParseError { url: String },
}

#[derive(Debug, thiserror::Error)]
#[error("Error returned by crate::docker::execute_command_for_container")]
pub(crate) enum ExecCommandForContainerError {
    Error(#[source] Error),
    BollardError(#[source] bollard::errors::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("Error returned by crate::util::find_book")]
pub(crate) enum FindBookError {
    Error(#[source] Error),
    ParseError(#[source] url::ParseError),
}
