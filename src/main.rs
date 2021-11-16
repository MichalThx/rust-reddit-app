#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(non_upper_case_globals)]
#![allow(unused_must_use)]
#![allow(unused_mut)]
#![allow(unused_doc_comments)]

use std::fs;
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io::{Read, Seek, SeekFrom, Write};
use std::pin::Pin;
use std::ptr::null;
use std::time::SystemTime;
use std::{env, io};

use async_recursion::async_recursion;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use futures::future::BoxFuture;
use reqwest::header::USER_AGENT;
use reqwest::{header, Client, Request, Result, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Result as R, Value};
use tokio::{task, time};
use tokio_postgres::types::Type;
use tokio_postgres::{Error, NoTls, Row};
use toml::{Value as TomlValue, de::Error as TomlError};
use toml::value::{Array, Datetime};

static DEBUG : bool = true;

#[derive(Serialize, Deserialize)]
struct RefreshToken {
    token: String,
    unixTimestamp: NaiveDateTime,
}

#[derive(Deserialize)]
struct RefreshTokenAPI {
    access_token: String,
    token_type: String,
    expires_in: i32,
    scope: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    database: Database,
    api: Api,
    reddit: Reddit,
}

#[derive(Serialize, Deserialize, Clone)]
struct Database {
    host: String,
    user: String,
    password: String,
    dbname: String,
}
#[derive(Serialize, Deserialize, Clone)]
struct Api {
    user_agent: String,
    username: String,
    password: String,
    client_id: String,
    secret: String,
    token_file: String,
}
#[derive(Serialize, Deserialize, Clone)]
struct Reddit {
    subreddits: Array,
    endings: Array,
    max_comment_depth: i32,
}
#[tokio::main]
async fn main() -> Result<()>{

    let thread = task::spawn(async {
        loop {
            time::sleep(time::Duration::from_secs(900_u64)).await;
            get_data().await;
        }
    });

    thread.await;
    Ok(())
}

async fn get_data() -> Result<()> {

    let content : &String = &std::fs::read_to_string("configuration.toml").unwrap();
    let config: Config = toml::from_str(&content).unwrap();

    let (db_client, connection) = tokio_postgres::connect(
        &format!("host={} user={} password={} dbname={}",
                 &config.database.host, &config.database.user, &config.database.password, &config.database.dbname),
        NoTls,
    )
    .await
    .expect("Error from tokio_postgres");
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let token = refresh_token(&config).await;

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
    );
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_str(&config.api.user_agent).unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_str("application/json").unwrap(),
    );
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_str("application/json").unwrap(),
    );

    let http_client = Client::builder().default_headers(headers).build().unwrap();

    discover_posts(&db_client, &http_client, &config).await;
    if DEBUG {println!("Finished discovery");}
    download_data(&db_client, &http_client, &config).await;
    Ok(())
}

/// refresh_token()
///
/// Method responsible for initial check if the current Token is valid
/// If it is - the application starts normally
/// If it is not valid - the application requests new token and saves the Oauth
async fn refresh_token(config : &Config) -> String {
    let name = &config.api.token_file;
    let is_valid = is_valid_token(&config);

    if is_valid.0 {
        return is_valid.1;
    } else {
        match request_token(&config).await {
            Ok(tok) => update_token(&name, tok),
            Err(e) => {
                panic!("Error getting token");
            }
        }
    }
}

/// is_valid_token(&Config)
///
/// Checks if the token is "fresh" enough to be used
/// If it is not, returns false and requires a token refresh
fn is_valid_token(config : &Config) -> (bool, String) {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(&config.api.token_file)
        .unwrap();

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(&file);
    let current_time = Utc::now().naive_utc();
    let mut diff = 0;
    let mut refresh_token = String::new();
    for result in reader.deserialize() {
        let record: RefreshToken = result.unwrap();
        diff = current_time
            .signed_duration_since(record.unixTimestamp)
            .num_minutes();
        refresh_token = record.token;
    }
    (diff < 50, refresh_token)
}

/// update_token(name: &str, token: String)
///
/// Starts the motion of refreshing the token.
/// Asks the API for the new token and refreshes the
/// storage file with it.
fn update_token(name: &str, token: String) -> String {
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(name)
        .unwrap();
    let refreshed_token = RefreshToken {
        token: token.parse().unwrap(),
        unixTimestamp: Utc::now().naive_utc(),
    };
    let mut wtr = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(&file);
    wtr.serialize(refreshed_token);
    token
}

async fn request_token(config : &Config) -> Result<String> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_str(&config.api.user_agent).unwrap(),
    );
    if DEBUG {println!("{:?}", headers.get(header::AUTHORIZATION));}
    let form = [
        ("grant_type", "password"),
        ("username", &config.api.username),
        ("password", &config.api.password),
    ];
    let client = Client::builder().default_headers(headers).build().unwrap();
    let response = client
        .post("https://api.reddit.com/api/v1/access_token")
        .basic_auth(&config.api.client_id, Some(&config.api.secret))
        .form(&form)
        .send()
        .await?;
    let refresh_token_api = response.json::<RefreshTokenAPI>().await?;

    // The question mark means - This function returns the value or the error
    // If the error happens the return would be an Err() not Ok()
    Ok(refresh_token_api.access_token)
}

async fn download_data(db_client: &tokio_postgres::Client, http_client: &Client,
                       config: &Config) -> Result<()> {
    let mut now: chrono::DateTime<Utc> = chrono::DateTime::from(chrono::Utc::now());

    let posts_to_check = db_client
        .query(
            "SELECT id, link, stage FROM updates WHERE update_time < $1",
            &[&now],
        )
        .await
        .expect("Error getting update_time from updates");

    for post in posts_to_check {
        let link: String = post.get("link");
        let id: i32 = post.get("id");
        let stage: i16 = post.get("stage");
        now = chrono::DateTime::from(chrono::Utc::now());
        match stage {
            0 => now = now + Duration::hours(1),
            1 => now = now + Duration::hours(3),
            2 => now = now + Duration::hours(5),
            3 => now = now + Duration::hours(8),
            4 => now = now + Duration::hours(16),
            5 => now = now + Duration::hours(24),
            6 => now = now + Duration::days(900),
            other => panic!("Error checking stage"),
        }
        if DEBUG {println!("Downloading post {}", id);}
        insert_data_to_db(&link, &db_client, &http_client, &(stage + 1), &config).await;

        db_client
            .query(
                "UPDATE updates SET stage = $1, update_time = $2 WHERE id = $3",
                &[&(stage + 1), &now, &id],
            )
            .await
            .expect("Error updating update_time in updates");
    }
    Ok(())
}

async fn insert_data_to_db(url: &str, db_client: &tokio_postgres::Client, http_client: &Client,
    stage: &i16, config: &Config) -> Result<()> {

    let result = http_client.get(url).send().await?.text().await?;
    let v: Value = serde_json::from_str(&*result).unwrap();

    // Inserting a post
    let data = &v[0]["data"]["children"][0]["data"];
    let author = data["author"].to_string().replace("\"", "");
    let post_id = data["id"].to_string().replace("\"", "");
    let title = data["title"].to_string().replace("\"", "");
    let selftext = data["selftext"].to_string().replace("\"", "");
    let timestamp = &data["created_utc"].as_f64().unwrap();
    let utc: chrono::DateTime<Utc> =
        chrono::DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(*timestamp as i64, 0), Utc);
    let mut preview = String::new();
    if data["preview"] != serde_json::Value::Null {
        preview = data["preview"]["images"][0]["source"]["url"].to_string();
    }
    let ratio = data["upvote_ratio"].as_f64().unwrap();
    let ratio_name = format!("ratio_{}", stage.to_string());
    let score = data["score"].as_f64().unwrap() as i16;
    let score_name = format!("score_{}", stage.to_string());

    let subreddit_size: i32 = data["subreddit_subscribers"].as_f64().unwrap() as i32;
    let subreddit = data["subreddit"].to_string();
    let insert = db_client
        .query(&format!("INSERT INTO posts (post_id, title, selftext, author, utc, preview, ratio_1, score_1, subreddit, subreddit_size) \
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (post_id) DO UPDATE SET {} = $7, {} = $8;", ratio_name, score_name),
               &[&post_id, &title, &selftext, &author, &utc, &preview, &ratio, &score, &subreddit, &subreddit_size])
        .await.expect("Error inserting to post");

    // Inserting comments
    add_comments_to_db(&v[1], stage, 0_i32, &db_client, "", &config).await;

    Ok(())
}

#[async_recursion]
async fn add_comments_to_db(response: &Value, stage: &i16, depth: i32, db_client: &tokio_postgres::Client,
    base_id: &str, config : &Config) -> () {
    let mut iterator = 0;
    let mut data = &response["data"]["children"][iterator]["data"];

    while data["body"] != serde_json::Value::Null {
        let mut body = data["body"].to_string();
        body = body.replace("\"", "");
        let author = data["author"].to_string().replace("\"", "");
        let comment_id = format!("{}{}", base_id, data["id"].to_string().replace("\"", ""));
        let timestamp = data["created_utc"].as_f64().unwrap();
        let utc: chrono::DateTime<Utc> = chrono::DateTime::<Utc>::from_utc(
            NaiveDateTime::from_timestamp(timestamp as i64, 0),
            Utc,
        );

        let controversiality = data["controversiality"].is_null();
        let score = data["score"].as_f64().unwrap() as i16;
        let post_id = data["parent_id"].to_string();
        let controversiality_name = format!("controversiality_{}", stage.to_string());
        let score_name = format!("score_{}", stage.to_string());
        let insert = db_client
            .query(&format!("INSERT INTO comments (comment_id, body, author, utc, {cont}, {score}, post_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (comment_id) DO UPDATE SET {cont} = $5, {score} = $6;",
                            cont = controversiality_name, score = score_name),
                   &[&comment_id, &body, &author, &utc, &controversiality, &score, &post_id])
            .await.expect("Error inserting to comments");

        if &depth <= &config.reddit.max_comment_depth {
            let mut children_data = &data["replies"];
            add_comments_to_db(children_data, stage, depth + 1, db_client,
                               &format!("{}_", &comment_id), &config).await;
        }
        iterator += 1;
        data = &response["data"]["children"][iterator]["data"];
    }
}

async fn discover_posts(db_client: &tokio_postgres::Client, http_client: &Client, config: &Config) -> Result<()> {
    let urls = &config.reddit.subreddits;
    let endings = &config.reddit.endings;

    for x in urls {
        for y in endings {
            let url_temp = format!("https://oauth.reddit.com{}{}", clean_data(x.to_string()), clean_data(y.to_string()));
            let mut result = http_client.get(&url_temp).send().await?.text().await?;
            let v: Value = serde_json::from_str(&*result).unwrap();
            let mut iterator = 0;
            let mut data = &v["data"]["children"][iterator]["data"];

            while data != &serde_json::Value::Null {
                let id = clean_data(data["id"].to_string());
                let url_to_post = format!(
                    "https://oauth.reddit.com{}?raw_json=1&sort=best&api_type=json",
                    clean_data(data["permalink"].to_string())
                );
                let post_id = clean_data(data["id"].to_string());
                let timestamp = data["created_utc"].as_f64().unwrap();
                let utc: chrono::DateTime<Utc> = chrono::DateTime::<Utc>::from_utc(
                    NaiveDateTime::from_timestamp(timestamp as i64, 0),
                    Utc,
                );
                let init: chrono::DateTime<Utc> = chrono::DateTime::from(chrono::Utc::now());
                let now: chrono::DateTime<Utc> = chrono::DateTime::from(chrono::Utc::now());

                let insert = db_client
                    .query("INSERT INTO updates (post_id, utc, init, update_time, stage, link) \
                    VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (post_id) DO NOTHING;",
                           &[&post_id, &utc, &init, &now, &0_i16, &url_to_post])
                    .await.expect("Error inserting to comments");
                iterator += 1;
                data = &v["data"]["children"][iterator]["data"];
            }
        }
    }
    Ok(())
}

fn clean_data(str: String) -> String {
    let mut w = str;
    w = w.replace("\"", "");
    w = w.replace("\n", " ");
    w = w.replace("\"", "");
    w = w.replace("\\", "");

    return w;
}