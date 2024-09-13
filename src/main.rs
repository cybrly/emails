use clap::{Arg, ArgAction, Command};
use colored::*;
use regex::Regex;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// List of common TLDs for validation
const COMMON_TLDS: &[&str] = &[
    "com", "org", "net", "edu", "gov", "mil", "int", "co", "io", "me", "biz",
    "info", "us", "uk", "ca", "de", "jp", "fr", "au", "ru", "ch", "it", "nl",
    "se", "no", "es", "mil", "gov", "edu", "tv", "ly",
];

fn main() {
    // Parse command-line arguments
    let matches = Command::new("emails")
        .version("1.0")
        .author("Chris Neuwirth")
        .about("Searches a website for email addresses.")
        .arg(
            Arg::new("URL")
                .help("The URL to scrape")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("depth")
                .short('d')
                .long("depth")
                .help("Depth of recursion")
                .default_value("2")
                .value_name("DEPTH")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("threads")
                .short('t')
                .long("threads")
                .help("Number of threads to use")
                .default_value("4")
                .value_name("THREADS")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("timeout")
                .long("timeout")
                .help("Timeout in seconds if no results are found")
                .default_value("60")
                .value_name("SECONDS")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("strict")
                .long("strict")
                .help("Only print emails that match the domain provided")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    // Get command-line argument values
    let input_url = matches.get_one::<String>("URL").unwrap();
    let depth = matches
        .get_one::<String>("depth")
        .unwrap()
        .parse::<usize>()
        .expect("Depth must be a number");
    let num_threads = matches
        .get_one::<String>("threads")
        .unwrap()
        .parse::<usize>()
        .expect("Threads must be a number");
    let timeout = matches
        .get_one::<String>("timeout")
        .unwrap()
        .parse::<u64>()
        .expect("Timeout must be a number");
    let strict_mode = matches.get_flag("strict");

    // Prepend http:// if missing
    let url = if input_url.starts_with("http://") || input_url.starts_with("https://") {
        input_url.to_string()
    } else {
        format!("http://{}", input_url)
    };

    println!("Starting email scraping on: {}", url);

    // Shared data structures
    let emails_found = Arc::new(Mutex::new(HashSet::new()));
    let urls_to_visit = Arc::new(Mutex::new(VecDeque::new()));
    let visited_urls = Arc::new(Mutex::new(HashSet::new()));
    urls_to_visit
        .lock()
        .unwrap()
        .push_back((url.clone(), 0));
    visited_urls.lock().unwrap().insert(url.clone());

    // HTTP client
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    // Domain extraction for email matching
    let domain = get_domain(&url);

    let start_time = Instant::now();
    let mut handles = vec![];

    // Spawn threads
    for _ in 0..num_threads {
        let emails_found = Arc::clone(&emails_found);
        let urls_to_visit = Arc::clone(&urls_to_visit);
        let visited_urls = Arc::clone(&visited_urls);
        let client = client.clone();
        let domain = domain.clone();
        let start_time = start_time.clone();
        let strict_mode = strict_mode;

        let handle = thread::spawn(move || loop {
            // Check for timeout
            if start_time.elapsed() > Duration::from_secs(timeout) {
                break;
            }

            let (current_url, current_depth) = {
                let mut urls = urls_to_visit.lock().unwrap();
                if let Some((url, depth)) = urls.pop_front() {
                    (url, depth)
                } else {
                    break;
                }
            };

            if current_depth > depth {
                continue;
            }

            // Fetch the page content
            match client.get(&current_url).send() {
                Ok(resp) => {
                    if let Ok(text) = resp.text() {
                        // Extract emails
                        let emails = extract_emails(&text);
                        let mut emails_set = emails_found.lock().unwrap();
                        for email in emails {
                            let email = email.trim().trim_start_matches(|c| !char::is_alphanumeric(c));
                            let email_lower = email.to_lowercase();

                            // Decide whether to decode
                            let should_decode = is_likely_rot13_encoded(&email_lower);

                            let final_email = if should_decode {
                                let decoded_email = rot13_decode(&email_lower);
                                if is_valid_email(&decoded_email) {
                                    decoded_email
                                } else {
                                    continue; // Skip invalid emails
                                }
                            } else {
                                if is_valid_email(&email_lower) {
                                    email_lower
                                } else {
                                    continue; // Skip invalid emails
                                }
                            };

                            // Skip if it's an asset filename
                            if is_asset_filename(&final_email) {
                                continue;
                            }

                            if emails_set.insert(final_email.clone()) {
                                let email_matches_domain = final_email
                                    .to_lowercase()
                                    .ends_with(&domain.to_lowercase());

                                if strict_mode {
                                    if email_matches_domain {
                                        println!("{}", final_email.green());
                                    }
                                } else {
                                    if email_matches_domain {
                                        println!("{}", final_email.green());
                                    } else {
                                        println!("{}", final_email.white());
                                    }
                                }
                            }
                        }

                        // Extract links and add to queue
                        if current_depth < depth {
                            let links = extract_links(&text, &current_url);
                            let mut urls = urls_to_visit.lock().unwrap();
                            let mut visited = visited_urls.lock().unwrap();
                            for link in links {
                                if !visited.contains(&link) {
                                    urls.push_back((link.clone(), current_depth + 1));
                                    visited.insert(link);
                                }
                            }
                        }
                    }
                }
                Err(_err) => {
                    // Suppress error messages
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to finish
    for handle in handles {
        handle.join().unwrap();
    }

    println!(
        "Finished scraping. Found {} emails.",
        emails_found.lock().unwrap().len()
    );
}

// Function to extract emails using regex
fn extract_emails(text: &str) -> Vec<String> {
    let re = Regex::new(
        r"(?i)([a-z0-9._%+-]+@[a-z0-9-]+(?:\.[a-z0-9-]+)*\.[a-z]{2,})",
    )
    .unwrap();
    re.find_iter(text)
        .map(|mat| mat.as_str().to_string())
        .collect()
}

// Function to check if an email is likely ROT13 encoded
fn is_likely_rot13_encoded(email: &str) -> bool {
    // Simple heuristic: check if the domain ends with a known TLD after decoding
    let decoded_email = rot13_decode(email);
    let domain = decoded_email.split('@').nth(1).unwrap_or("");
    let tld = domain.split('.').last().unwrap_or("");
    COMMON_TLDS.contains(&tld)
}

// Function to decode ROT13 encoded emails
fn rot13_decode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + 13) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + 13) % 26) + b'A') as char,
            _ => c,
        })
        .collect()
}

// Function to check if the email is likely an asset filename
fn is_asset_filename(email: &str) -> bool {
    let asset_extensions = [
        ".png", ".jpg", ".jpeg", ".gif", ".svg", ".css", ".js", ".ico", ".pdf", ".zip", ".rar",
        ".exe",
    ];
    asset_extensions.iter().any(|ext| email.contains(ext))
}

// Function to validate the email format
fn is_valid_email(email: &str) -> bool {
    // Use a comprehensive regex for validation
    let email_regex = Regex::new(
        r"(?i)^[a-z0-9._%+-]+@[a-z0-9-]+(?:\.[a-z0-9-]+)*\.([a-z]{2,})$",
    )
    .unwrap();

    if let Some(caps) = email_regex.captures(email) {
        let tld = &caps[1].to_lowercase();
        COMMON_TLDS.contains(&tld.as_str())
    } else {
        false
    }
}

// Function to extract links from the HTML content
fn extract_links(html: &str, base_url: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").unwrap();
    let mut links = Vec::new();

    for element in document.select(&selector) {
        if let Some(href) = element.value().attr("href") {
            // Skip mailto, tel, javascript, and other non-http(s) links
            if href.starts_with("#")
                || href.starts_with("mailto:")
                || href.starts_with("tel:")
                || href.starts_with("javascript:")
                || href.starts_with("data:")
            {
                continue;
            }

            // Resolve relative URLs
            let full_url = match url::Url::parse(href) {
                Ok(url) => {
                    // Only accept http and https URLs
                    if url.scheme() == "http" || url.scheme() == "https" {
                        url.to_string()
                    } else {
                        continue;
                    }
                }
                Err(_) => match url::Url::parse(base_url) {
                    Ok(base) => match base.join(href) {
                        Ok(joined) => joined.to_string(),
                        Err(_) => continue,
                    },
                    Err(_) => continue,
                },
            };

            links.push(full_url);
        }
    }

    links
}

// Function to extract the domain from the URL
fn get_domain(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed_url) => {
            if let Some(host) = parsed_url.host_str() {
                host.to_string()
            } else {
                "".to_string()
            }
        }
        Err(_) => "".to_string(),
    }
}
