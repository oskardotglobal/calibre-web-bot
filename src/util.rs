use crate::errors::Error;
use crate::errors::FindBookError;
use reqwest::Response;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serenity::futures::SinkExt;
use std::time::Instant;
use tokio::sync::Mutex;
use url::Url;

const HASTEBIN_URL: &str = "https://paste.developerden.org";

#[derive(Deserialize)]
struct HastebinJsonResponse {
    key: Option<String>,
}

pub(crate) async fn upload_to_haste(s: String) -> Option<String> {
    let client = reqwest::Client::new();

    match client
        .post(HASTEBIN_URL.to_owned() + "/documents")
        .body(s)
        .send()
        .await
        .ok()?
        .json::<HastebinJsonResponse>()
        .await
    {
        Ok(HastebinJsonResponse { key, .. }) => match key {
            Some(key) => Some(HASTEBIN_URL.to_owned() + "/" + key.as_str()),
            None => None,
        },
        _ => None,
    }
}

static LAST_REQUEST_MUTEX: Mutex<Option<Instant>> = Mutex::new(None);
static REQUEST_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

// Do a request for the given URL, with a minimum time between requests
// to avoid overloading the server.
async fn do_throttled_request(url: &str) -> Option<Response> {
    let mut last_request_mutex = LAST_REQUEST_MUTEX.lock().await;
    let last_request = last_request_mutex.take();
    let now = Instant::now();

    if let Some(last_request) = last_request {
        let duration = now.duration_since(last_request);

        if duration < REQUEST_DELAY {
            std::thread::sleep(REQUEST_DELAY - duration);
        }
    }

    let response = reqwest::get(url).await;
    last_request_mutex.replace(now);

    Some(response.unwrap())
}

pub(crate) struct Book {
    title: String,
    author: String,
    isbn: String,
    link: String,
    description: Option<String>,
    download_links: Vec<String>,
    cover_url: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct DbBook {
    id: String,
    filename: String,
    download_urls: Vec<Vec<String>>,
    top_box: DbBookTopBox,
}

#[derive(Deserialize, Serialize)]
struct DbBookTopBox {
    title: String,
    author: String,
    top_row: String,
    cover_url: Option<String>,
    description: Option<String>,
}

pub(crate) async fn find_book(query: String) -> Result<Vec<Book>, FindBookError> {
    let input = format!(
        "https://annas-archive.org/search?q={}",
        url_escape::encode_fragment(&query)
    );

    let url = match Url::parse(input.as_str()) {
        Ok(url) => url.to_string(),
        Err(e) => return Err(FindBookError::ParseError(e)),
    };

    info!("Searching for book: {}", url);

    let body = match do_throttled_request(url.as_str()).await {
        Some(res) => match res.text().await {
            Ok(text) => text,
            Err(_) => {
                return Err(FindBookError::Error(Error::RequestFailed {
                    url: url.clone(),
                }))
            }
        },
        None => {
            return Err(FindBookError::Error(Error::RequestFailed {
                url: url.clone(),
            }))
        }
    };

    let document = Html::parse_document(body.as_str());

    /*
    /search has the following site structure:
    <body>
        <main>
            <!-- 2 random divs, although the amount might change from time to time -->
            <form action="/search" method="get" role="search"></form>

            <!-- this is the important part -->
            <!-- it contains the search results -->
            <div>
                <!-- for every search result there's a div like this -->
                <div>
                    <!-- we need this link -->
                    <a href="...">
                        <!-- Description of the book -->
                    </a>
                </div>

                <!-- if there's no results, this div contains a single <span> -->
            </div>
        </main>
    </body>
    */

    let selector = match Selector::parse("main > div > div > a") {
        Ok(selector) => selector,

        Err(_) => {
            return match Selector::parse("main > div > div > span") {
                Ok(_) => Err(FindBookError::Error(Error::NoResults { query })),
                Err(_) => Err(FindBookError::Error(Error::ParseError { url })),
            }
        }
    };

    let mut i = 0;
    let mut results = Vec::<Book>::new();

    for result in document.select(&selector) {
        i += 1;

        if i == 3 {
            break;
        }

        let md5 = match result.value().attr("href") {
            Some(href) => href.replace("/md5/", ""),
            None => return Err(FindBookError::Error(Error::ParseError { url })),
        };

        let book = match do_throttled_request(
            format!("https://anna-archive.org/db/aarecords/md5:{}.json", md5).as_str(),
        )
        .await
        {
            Some(res) => match res.json::<DbBook>().await {
                Ok(book) => book,
                Err(_) => return Err(FindBookError::Error(Error::RequestFailed { url })),
            },
            None => return Err(FindBookError::Error(Error::RequestFailed { url })),
        };

        info!("{}", serde_json::to_string_pretty(&book).unwrap());

        let _ = results.send(Book {
            title: String::new(),
            author: String::new(),
            isbn: String::new(),
            link: format!("https://anna-archive.org/md5/{}", md5),
            description: None,
            download_links: vec![],
            cover_url: None,
        });
    }

    if i == 0 {
        return Err(FindBookError::Error(Error::NoResults { query }));
    }

    Ok(results)
}
