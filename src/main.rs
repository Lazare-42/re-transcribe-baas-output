use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;
use dotenv::dotenv;
use std::env;
mod process_transcriptions;


const API_URL: &'static str = "https://api.runpod.ai/v2/oq0i26ut0lom1h";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load the .env file
    dotenv().ok();

    // Get the RUNPOD_API_KEY from the environment
    let runpod_api_key = env::var("RUNPOD_API_KEY").expect("RUNPOD_API_KEY must be set");

    let client = Client::new();
    let dir = "./files";
    read_dir(dir, &client, &runpod_api_key).await?;

    // After read_dir completes, call process_transcriptions
    let bot_id_file = "bot_ids.txt";
    let bot_misc_dir = dir;
    process_transcriptions::process_transcriptions(bot_id_file, bot_misc_dir)?;

    Ok(())
}

#[derive(Deserialize, Serialize, Debug)]
struct RunPodResult {
    detected_language: String,
    word_timestamps: Vec<RunPodWordTimestamp>,
}

#[derive(Deserialize, Serialize, Debug)]
struct RunPodWordTimestamp {
    start: f64,
    end: f64,
    word: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct ApiResponse {
    id: Option<String>,
    status: String,
    output: Option<RunPodResult>,
}

async fn runpod(url: &str, request_client: &Client, runpod_api_key: &str) -> Result<RunPodResult, Box<dyn Error>> {
    let payload = json!({
        "input": {
            "audio": url,
            "model": "large-v3",
            "transcription": "plain_text",
            "translate": false,
            "temperature": 0,
            "best_of": 5,
            "beam_size": 5,
            "patience": 1,
            "suppress_tokens": "-1",
            "condition_on_previous_text": false,
            "temperature_increment_on_fallback": 0.2,
            "compression_ratio_threshold": 2.4,
            "logprob_threshold": -1,
            "no_speech_threshold": 0.6,
            "word_timestamps": true,
            "language": "pt",
        },
        "enable_vad": false,
    });

    let response = request_client
        .post(format!("{}/run", API_URL))
        .header("Content-Type", "application/json")
        .bearer_auth(runpod_api_key)
        .json(&payload)
        .send()
        .await?;

    if response.status().is_success() {
        let mut api_response: ApiResponse = response.json().await?;

        println!("ID: {:?}", api_response.id);
        println!("Status: {}", api_response.status);

        while api_response.status != "COMPLETED" && api_response.status != "FAILED" {
            sleep(Duration::from_secs(5)).await;

            let response = request_client
                .get(format!("{}/status/{}", API_URL, api_response.id.unwrap()))
                .header("Content-Type", "application/json")
                .bearer_auth(runpod_api_key)
                .send()
                .await?;
            api_response = response.json().await?;

            println!("ID: {:?}", api_response.id);
            println!("Status: {}", api_response.status);
        }
        dbg!(&api_response);
        match api_response.status.as_str() {
            "COMPLETED" => return Ok(api_response.output.unwrap()),
            "FAILED" => {
                unimplemented!()
            }
            _ => unreachable!(),
        }
    } else {
        println!("Erreur lors de la requête : {:?}", response.status());
        unimplemented!()
    }
}

async fn read_dir(dir: &str, request_client: &Client, runpod_api_key: &str) -> Result<(), Box<dyn Error>> {
    let path = Path::new(dir);
    let output_dir = Path::new("./files");

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let filename = entry.file_name();
            let path = entry.path();

            if path.is_file() {
                let file = File::open(&path)?;
                let lines = io::BufReader::new(file).lines();
                for (_idx, line) in lines.enumerate() {
                    if let Ok(l) = line {
                        if l.contains("mp4_s3_path") {
                            if let Some(s) = l.split(": ").nth(1) {
                                let s = s.trim_matches(&['"', ','][..]);
                                dbg!(s);

                                let output_file = output_dir.join(format!(
                                    "{}.runpod",
                                    filename.to_str().unwrap()
                                ));
                                let mut out = File::create(output_file)?;
                                let output = runpod(s, request_client, runpod_api_key).await?;
                                writeln!(out, "{}", serde_json::to_string(&output).unwrap())?
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("Invalid dirent");
    }
    Ok(())
}
