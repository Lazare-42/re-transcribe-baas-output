use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::env;
use std::path::Path;
use std::process;


use serde::Serialize;
use serde_json::Value;

#[derive(Debug)]
struct Transcript {
    speaker: String,
    audio_offset: f64,
    duration: f64,
}

#[derive(Debug, Serialize)]
struct Word {
    start: f64,
    end: f64,
    word: String,
}

#[derive(Debug, Serialize)]
struct Webhook {
    event: String,
    data: WebhookData,
}

#[derive(Debug, Serialize)]
struct WebhookData {
    bot_id: String,
    transcript: Vec<OutTranscript>,
    speakers: Vec<String>,
    mp4: String,
}

#[derive(Debug, Serialize)]
struct OutTranscript {
    speaker: String,
    offset: f64,
    words: Vec<Word>,
}

const OUTPUT_DIR: &str = "./transcription_output";

pub fn process_transcriptions(bot_id_file: &str, bot_misc_dir: &str) -> std::io::Result<()> {
    // Check if bot_id_file exists
    if !Path::new(bot_id_file).exists() {
        eprintln!("Error: Bot ID file '{}' does not exist", bot_id_file);
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Bot ID file not found"));
    }

    // Check if bot_misc_dir exists
    if !Path::new(bot_misc_dir).exists() {
        eprintln!("Error: Bot misc directory '{}' does not exist", bot_misc_dir);
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Bot misc directory not found"));
    }

    let bot_id_file = File::open(bot_id_file).map_err(|e| {
        eprintln!("Error opening bot ID file: {}", e);
        e
    })?;

    // Create output directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(OUTPUT_DIR) {
        eprintln!("Error creating output directory '{}': {}", OUTPUT_DIR, e);
        return Err(e);
    }

    let lines = BufReader::new(bot_id_file).lines();
    for line in lines {
        let bot_id = line?;
        println!("Processing bot_id: {}", bot_id);

        let json_file_path = format!("{}/{}.json", bot_misc_dir, bot_id);
        let runpod_file_path = format!("{}/{}.json.runpod", bot_misc_dir, bot_id);

        // Check if json file exists and is readable
        if let Err(e) = File::open(&json_file_path) {
            eprintln!("Error accessing JSON file '{}': {}", json_file_path, e);
            continue;
        }

        // Check if runpod file exists and is readable
        if let Err(e) = File::open(&runpod_file_path) {
            eprintln!("Error accessing Runpod file '{}': {}", runpod_file_path, e);
            continue;
        }

        let json_file = match read_file(&json_file_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file {}: {}", json_file_path, e);
                continue;
            }
        };

        let runpod_file = match read_file(&runpod_file_path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file {}: {}", runpod_file_path, e);
                continue;
            }
        };

        let raw_json: Value = match serde_json::from_str(&json_file) {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Error parsing JSON from {}: {}", json_file_path, e);
                continue;
            }
        };

        let raw_words_file: Value = match serde_json::from_str(&runpod_file) {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Error parsing JSON from {}: {}", runpod_file_path, e);
                continue;
            }
        };

        let mut raw_transcripts = Vec::new();
        find_objects_with_key(&raw_json, "transcripts", &mut raw_transcripts);

        let transcripts: Vec<Transcript> = raw_transcripts
            .iter()
            .map(|transcript| {
                let audio_offset: f64 = transcript.get("audio_offset").unwrap().as_f64().unwrap();
                let duration = transcript.get("duration").unwrap().as_f64().unwrap();
                let speaker = transcript
                    .get("transcripts")
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get("speaker")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();

                Transcript {
                    speaker,
                    audio_offset,
                    duration,
                }
            })
            .collect();
        // dbg!(transcripts);

        let mut raw_words = Vec::new();
        find_objects_with_key(&raw_words_file, "word", &mut raw_words);

        let mut words: Vec<Word> = raw_words
            .iter()
            .map(|word| {
                let start: f64 = word.get("start").unwrap().as_f64().unwrap();
                let end: f64 = word.get("end").unwrap().as_f64().unwrap();
                let word: String = word.get("word").unwrap().as_str().unwrap().to_owned();
                Word { start, end, word }
            })
            .collect();
        words.reverse();
        // dbg!(words);

        let mut speakers: HashSet<String> = HashSet::new();
        let mut out_transcripts: Vec<OutTranscript> = Vec::new();

        let mut word = words.pop();
        for transcript in transcripts {
            let mut out_trancript: Option<OutTranscript> = None;
            while let Some(word_ref) = &word {
                if word_ref.start >= transcript.audio_offset
                    && word_ref.start < transcript.audio_offset + transcript.duration
                {
                    match &mut out_trancript {
                        Some(out_transcript) => {
                            out_transcript.words.push(Word {
                                start: word_ref.start,
                                end: word_ref.end,
                                word: word_ref.word.clone(),
                            });
                        }
                        None => {
                            out_trancript = Some(OutTranscript {
                                speaker: transcript.speaker.clone(),
                                offset: transcript.audio_offset,
                                words: vec![Word {
                                    start: word_ref.start,
                                    end: word_ref.end,
                                    word: word_ref.word.clone(),
                                }],
                            })
                        }
                    }
                    word = words.pop();
                } else {
                    break;
                }
            }
            if let Some(out_transcript) = out_trancript {
                let _r = speakers.insert(transcript.speaker);
                out_transcripts.push(out_transcript)
            }
        }
        let speakers: Vec<String> = speakers.iter().map(|s| s.clone()).collect();

        let output = Webhook {
            event: "complete".to_owned(),
            data: WebhookData {
                bot_id: bot_id.clone(),
                mp4: raw_json
                    .get("assets")
                    .unwrap()
                    .get(0)
                    .unwrap()
                    .get("mp4_s3_path")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
                transcript: out_transcripts,
                speakers,
            },
        };
        // dbg!(&output);

        // Save output to new directory with .json extension
        let output_file_path = format!("{}/{}.json", OUTPUT_DIR, bot_id);
        let mut out = File::create(&output_file_path).map_err(|e| {
            eprintln!("Error creating output file '{}': {}", output_file_path, e);
            e
        })?;

        // Write to output file
        writeln!(out, "{}", serde_json::to_string_pretty(&output).unwrap())?;
        println!("Output saved to: {}", output_file_path);
    }
    Ok(())
}

fn find_objects_with_key<'a>(value: &'a Value, key: &str, results: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            if map.contains_key(key) {
                results.push(value);
            }
            for v in map.values() {
                find_objects_with_key(v, key, results);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                find_objects_with_key(v, key, results);
            }
        }
        _ => {}
    }
}

fn read_file(path: &str) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}
