//! WordPiece tokenizer for BERT-family models.

use std::collections::HashMap;

const CLS_ID: u32 = 101;
const SEP_ID: u32 = 102;
const UNK_ID: u32 = 100;

/// A WordPiece tokenizer compatible with BERT/MiniLM vocabulary.
pub struct WordPieceTokenizer {
    vocab: HashMap<String, u32>,
    max_seq_len: usize,
}

/// Tokenized input ready for the model.
pub struct TokenizedInput {
    /// Token IDs (vocabulary indices).
    pub input_ids: Vec<u32>,
    /// Attention mask (1 for real tokens, 0 for padding).
    pub attention_mask: Vec<u32>,
    /// Token type IDs (0 for single-sentence input).
    pub token_type_ids: Vec<u32>,
}

impl WordPieceTokenizer {
    /// Build a tokenizer from a vocab.txt file (one token per line).
    pub fn from_vocab(vocab_text: &str) -> Self {
        let vocab: HashMap<String, u32> = vocab_text
            .lines()
            .enumerate()
            .map(|(i, line)| (line.to_string(), i as u32))
            .collect();

        Self {
            vocab,
            max_seq_len: 256,
        }
    }

    /// Tokenize a text string.
    pub fn tokenize(&self, text: &str) -> TokenizedInput {
        let lower = text.to_lowercase();
        let words = basic_split(&lower);

        let mut tokens = vec![CLS_ID];

        for word in &words {
            self.wordpiece_tokenize(word, &mut tokens);
            if tokens.len() >= self.max_seq_len - 1 {
                tokens.truncate(self.max_seq_len - 1);
                break;
            }
        }

        tokens.push(SEP_ID);

        let len = tokens.len();
        let attention_mask = vec![1u32; len];
        let token_type_ids = vec![0u32; len];

        TokenizedInput {
            input_ids: tokens,
            attention_mask,
            token_type_ids,
        }
    }

    fn wordpiece_tokenize(&self, word: &str, tokens: &mut Vec<u32>) {
        if word.is_empty() {
            return;
        }

        // Try the whole word first
        if let Some(&id) = self.vocab.get(word) {
            tokens.push(id);
            return;
        }

        // WordPiece subword tokenization
        let chars: Vec<char> = word.chars().collect();
        let mut start = 0;
        let mut is_first = true;

        while start < chars.len() {
            let mut end = chars.len();
            let mut found = false;

            while start < end {
                let substr: String = if is_first {
                    chars[start..end].iter().collect()
                } else {
                    format!("##{}", chars[start..end].iter().collect::<String>())
                };

                if let Some(&id) = self.vocab.get(&substr) {
                    tokens.push(id);
                    found = true;
                    start = end;
                    is_first = false;
                    break;
                }

                end -= 1;
            }

            if !found {
                tokens.push(UNK_ID);
                start += 1;
                is_first = false;
            }
        }
    }
}

/// Basic tokenization: split on whitespace and punctuation.
fn basic_split(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else if is_punctuation(ch) {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(ch.to_string());
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn is_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '!' | '"'
            | '#'
            | '$'
            | '%'
            | '&'
            | '\''
            | '('
            | ')'
            | '*'
            | '+'
            | ','
            | '-'
            | '.'
            | '/'
            | ':'
            | ';'
            | '<'
            | '='
            | '>'
            | '?'
            | '@'
            | '['
            | '\\'
            | ']'
            | '^'
            | '_'
            | '`'
            | '{'
            | '|'
            | '}'
            | '~'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vocab() -> String {
        // Minimal vocab for testing
        let mut lines = vec!["[PAD]"; 101]; // 0..100 = pad/unused
        lines[0] = "[PAD]";
        lines[100] = "[UNK]";
        lines.push("[CLS]"); // 101
        lines.push("[SEP]"); // 102
        lines.push("hello"); // 103
        lines.push("world"); // 104
        lines.push("##ing"); // 105
        lines.push("test"); // 106
        lines.join("\n")
    }

    #[test]
    fn test_basic_tokenization() {
        let vocab = test_vocab();
        let tok = WordPieceTokenizer::from_vocab(&vocab);
        let result = tok.tokenize("hello world");
        // [CLS]=101, hello=103, world=104, [SEP]=102
        assert_eq!(result.input_ids[0], CLS_ID);
        assert_eq!(result.input_ids[1], 103); // hello
        assert_eq!(result.input_ids[2], 104); // world
        assert_eq!(*result.input_ids.last().unwrap(), SEP_ID);
        assert_eq!(result.attention_mask.len(), result.input_ids.len());
        assert!(result.attention_mask.iter().all(|&v| v == 1));
    }

    #[test]
    fn test_unknown_word() {
        let vocab = test_vocab();
        let tok = WordPieceTokenizer::from_vocab(&vocab);
        let result = tok.tokenize("xyz");
        // [CLS], [UNK], [SEP]
        assert_eq!(result.input_ids[1], UNK_ID);
    }

    #[test]
    fn test_basic_split_punctuation() {
        let tokens = basic_split("hello, world!");
        assert_eq!(tokens, vec!["hello", ",", "world", "!"]);
    }

    #[test]
    fn test_empty_input() {
        let vocab = test_vocab();
        let tok = WordPieceTokenizer::from_vocab(&vocab);
        let result = tok.tokenize("");
        // [CLS], [SEP]
        assert_eq!(result.input_ids, vec![CLS_ID, SEP_ID]);
    }
}
