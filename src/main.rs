mod docker;

extern crate dotenv;

use std::env;

use bollard::Docker;
use dotenv::dotenv;
use url::Url;

use serenity::async_trait;
use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{StandardFramework, CommandResult, CommandError};

#[group]
#[commands(ping, add)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

struct DockerClientData;

impl TypeMapKey for DockerClientData {
    type Value = Docker;
}

#[tokio::main]
async fn main() {
    dotenv().ok().expect("Couldn't load .env");

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("$"))
        .group(&GENERAL_GROUP);

    let token = env::var("TOKEN").expect("TOKEN not found in env");
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    #[cfg(unix)]
    let docker_client = Docker::connect_with_socket_defaults()
        .expect("Couldn't connect to docker daemon");

    client.data.write().await.insert::<DockerClientData>(docker_client);

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    return CommandResult::Ok(());
}

#[command]
async fn add(ctx: &Context, msg: &Message) -> CommandResult {
    let args: Vec<&str> = msg.content.split(" ").collect();

    if args.len() != 2 {
        msg.reply(ctx, "Bad arguments.").await?;
        return CommandResult::Err(CommandError::from("Bad arguments."));
    }

    let parsed = Url::parse(args[1]);

    if !parsed.is_ok()
        || (parsed.is_ok() && !parsed.clone().unwrap().scheme().starts_with("http"))
        || (parsed.is_ok() && parsed.clone().unwrap().path_segments().is_none()) {

        msg.reply(ctx, "Invalid url.").await?;
        return CommandResult::Err(CommandError::from("Invalid url."));
    }

    let url = parsed.unwrap();

    let data = ctx.data.read().await;
    let docker_client = data.get::<DockerClientData>().unwrap();

    let _file_path = "/tmp/".to_owned() + url.path_segments().unwrap().last().unwrap();
    let file_path = _file_path.as_str();

    docker::execute_command_for_container(
        "calibre-web",
        docker_client,
        Some(vec!["curl", "-o", file_path, url.as_str()])
    ).await;

    docker::execute_command_for_container(
        "calibre-web",
        docker_client,
        Some(vec!["/usr/bin/calibredb", "add", "--library", "/books", file_path])
    ).await;

    docker::execute_command_for_container(
        "calibre-web",
        docker_client,
        Some(vec!["rm", file_path])
    ).await;

    return CommandResult::Ok(());
}
