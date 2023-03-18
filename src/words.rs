use std::path::Path;

use anyhow::Context;
use rand::seq::SliceRandom;
use rand::thread_rng;
use tokio::io::AsyncBufReadExt;
use tokio::{fs, io};
use tokio_stream::wrappers::LinesStream;
use tokio_stream::StreamExt;

#[derive(Debug, Clone)]
pub struct WordLists {
    adjectives: Vec<String>,
    nouns: Vec<String>,
}

impl WordLists {
    pub async fn load(
        adjectives_file: impl AsRef<Path>,
        nouns_file: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        Ok(WordLists {
            adjectives: read_lines(adjectives_file)
                .await
                .context("failed to read adjectives file")?,
            nouns: read_lines(nouns_file)
                .await
                .context("failed to read nouns file")?,
        })
    }
}

pub fn generate_key(words: &WordLists) -> String {
    let mut rng = thread_rng();
    let adj_a = words.adjectives.choose(&mut rng).unwrap();
    let adj_b = words.adjectives.choose(&mut rng).unwrap();
    let noun = words.nouns.choose(&mut rng).unwrap();
    format!("{adj_a}-{adj_b}-{noun}")
}

async fn read_lines(path: impl AsRef<Path>) -> io::Result<Vec<String>> {
    let file = fs::File::open(path).await?;
    let lines = LinesStream::new(io::BufReader::new(file).lines())
        .filter_map(Result::ok)
        .filter(|s| !s.is_empty())
        .collect()
        .await;
    Ok(lines)
}
