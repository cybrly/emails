# Emails Scraper

`emails` is a Rust-based command-line tool for scraping emails from websites. It takes a URL as input, searches the website for email addresses, and prints the results to the console. The tool offers several features, including multi-threading, recursion depth, timeout handling, and strict domain matching.

## Features

- Automatically append `http://` or `https://` if the protocol is missing.
- Recursively scrape emails from the provided URL.
- Print emails matching the URL domain in green; others are printed in white.
- Configurable recursion depth and number of threads.
- Timeout handling for long-running scrapes.
- Optional strict mode to only print emails matching the provided domain.
- Progress tracking and real-time output of results.

## Usage

### Basic Usage

./emails <URL>

### Options

- `-d, --depth <DEPTH>`: Set the depth of recursion. Default is 2.
- `-t, --threads <THREADS>`: Set the number of threads to use. Default is 4.
- `--timeout <SECONDS>`: Set the timeout for the scrape. Default is 60 seconds.
- `--strict`: Only print emails that match the domain of the provided URL.

### Examples

Scrape emails from a website:

./emails https://example.com

Scrape emails with a recursion depth of 3 and 8 threads:

./emails -d 3 -t 8 https://example.com

Scrape emails in strict mode:

./emails --strict https://example.com

## Building from Source

To build the project from source, make sure you have Rust installed, then run:

cargo build --release

The executable will be available in `target/release/emails`.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
