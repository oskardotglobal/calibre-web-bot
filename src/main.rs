mod docker;
mod errors;
mod util;

extern crate dotenv;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use std::env;

use bollard::Docker;
use dotenv::dotenv;
use url::Url;

use crate::util::{find_book, upload_to_haste};
use serenity::async_trait;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{CommandError, CommandResult, StandardFramework};
use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::utils::Colour;

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
    dotenv().ok().expect("Couldn't load .env file");
    pretty_env_logger::init();

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
    let docker_client =
        Docker::connect_with_socket_defaults().expect("Couldn't connect to docker daemon");

    client
        .data
        .write()
        .await
        .insert::<DockerClientData>(docker_client);

    if let Err(why) = client.start().await {
        error!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(&ctx.http, "Pong!").await?;

    return CommandResult::Ok(());
}

#[command]
async fn test(ctx: &Context, msg: &Message) -> CommandResult {
    find_book("Harry Potter".to_owned()).await?;

    return CommandResult::Ok(());
}

#[command]
async fn add(ctx: &Context, msg: &Message) -> CommandResult {
    let args: Vec<&str> = msg.content.split(" ").collect();

    if args.len() != 2 {
        msg.reply(&ctx.http, "Bad arguments.").await?;
        return CommandResult::Err(CommandError::from("Bad arguments."));
    }

    let parsed = Url::parse(args[1]);

    if !parsed.is_ok()
        || (parsed.is_ok() && !parsed.clone().unwrap().scheme().starts_with("http"))
        || (parsed.is_ok() && parsed.clone().unwrap().path_segments().is_none())
    {
        msg.reply(&ctx.http, "Invalid url.").await?;
        return CommandResult::Err(CommandError::from("Invalid url."));
    }

    let url = parsed.unwrap();

    let data = ctx.data.read().await;
    let docker_client = data.get::<DockerClientData>().unwrap();

    let _file_path = "/tmp/".to_owned() + url.path_segments().unwrap().last().unwrap();
    let file_path = _file_path.as_str();

    let mut log = "".to_owned();

    match docker::execute_command_for_container(
        "calibre-web",
        docker_client,
        Some(vec!["curl", "-o", file_path, url.as_str()]),
    )
    .await
    {
        Err(e) => error!("{}", e.to_string()),
        Ok(o) => log += &o,
    };

    match docker::execute_command_for_container(
        "calibre-web",
        docker_client,
        Some(vec![
            "/usr/bin/calibredb",
            "add",
            "--library",
            "/books",
            file_path,
        ]),
    )
    .await
    {
        Err(e) => error!("{}", e.to_string()),
        Ok(o) => log += &o,
    };

    match docker::execute_command_for_container(
        "calibre-web",
        docker_client,
        Some(vec!["rm", file_path]),
    )
    .await
    {
        Err(e) => error!("{}", e.to_string()),
        Ok(o) => log += &o,
    };

    let url = match upload_to_haste(log.to_owned()).await {
        Some(url) => format!("Command output: {}", url),
        _ => "Error whilst uploading or fetching command output".to_owned(),
    };

    msg.channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Added book")
                    .description(url.as_str())
                    .color(Colour::ORANGE)
            })
        })
        .await
        .expect("Couldn't send message");

    return CommandResult::Ok(());
}
